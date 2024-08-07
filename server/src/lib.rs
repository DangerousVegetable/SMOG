pub const PACKET_SIZE: usize = 9;

pub mod tcp {
    use log::{error, info, trace, warn};
    use packet_tools::{IndexedPacket, TimedQueue};
    use std::{
        sync::{atomic::AtomicBool, Arc, Mutex},
        time::Duration,
    };
    use tokio::{
        self,
        io::AsyncWriteExt,
        net::{TcpListener, TcpStream, ToSocketAddrs},
        task::JoinHandle,
        time::sleep,
    };

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    pub struct TcpSyncServer {
        pub listener: Arc<TcpListener>,
        slot_duration: Duration,
        slots_stored: usize,
        connection_task: Option<JoinHandle<()>>,
        listen_tasks: Option<Vec<JoinHandle<()>>>,
        send_task: Option<JoinHandle<()>>,
        streams: Arc<Mutex<Vec<Arc<TcpStream>>>>,
        accept_connections: Arc<AtomicBool>,
        is_running: Arc<AtomicBool>,
    }

    impl TcpSyncServer {
        pub async fn new<A>(addr: A, slot_duration: Duration, slots_stored: usize) -> Result<Self>
        where
            A: ToSocketAddrs,
        {
            let listener = TcpListener::bind(addr).await?;
            Ok(Self {
                listener: Arc::new(listener),
                slot_duration,
                slots_stored,
                connection_task: None,
                listen_tasks: None,
                send_task: None,
                streams: Arc::new(Mutex::new(vec![])),
                accept_connections: Arc::new(AtomicBool::new(true)),
                is_running: Arc::new(AtomicBool::new(false)),
            })
        }

        pub fn accept_connections(&mut self) {
            let accept_connections = self.accept_connections.clone();
            let listener = self.listener.clone();
            let streams = self.streams.clone();

            // listening for connections
            let connection_task = tokio::spawn(async move {
                info!(
                    "Listening for new connections on {:?}",
                    listener.local_addr().unwrap()
                );
                while accept_connections.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::select! {
                        result = listener.accept() => {
                            if let Ok((mut stream, _)) = result {
                                info!("Accepted connection: {:?}", stream.peer_addr().unwrap());
                                let id = streams.lock().unwrap().len() as u8;
                                stream.write(&[id]).await.unwrap();
                                streams.lock().unwrap().push(Arc::new(stream));
                            }
                        },
                        _ = sleep(Duration::from_millis(100)) => {
                            continue
                        }
                    }
                }
                info!("Stop listening for new connections");
            });

            self.connection_task = Some(connection_task);
        }

        pub fn run<const PACKET_SIZE: usize>(&mut self) {
            self.decline_connections();
            self.is_running
                .store(true, std::sync::atomic::Ordering::Relaxed);

            let packet_queue = Arc::new(Mutex::new(TimedQueue::<
                IndexedPacket<[u8; PACKET_SIZE], PACKET_SIZE>,
            >::new(self.slot_duration)));

            {
                let mut listen_tasks = Vec::new();
                info!("Start listening to incoming packets");
                // listening tasks
                let streams = self.streams.lock().unwrap();
                for (id, stream) in streams.iter().enumerate() {
                    let is_running = self.is_running.clone();
                    let stream = stream.clone();
                    let queue = packet_queue.clone();
                    let listen_task = tokio::spawn(async move {
                        loop {
                            if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                                info!("Closing connection with {:?}", stream.peer_addr().unwrap());
                                break;
                            }
                            let _ = stream.readable().await;
                            let mut packet = [0; PACKET_SIZE];
                            match stream.try_read(&mut packet) {
                                Ok(0) => {
                                    warn!("Client {} seems to have disconnected. Closing connection", stream.peer_addr().unwrap());
                                    break
                                }
                                Ok(n) => {
                                    trace!(
                                        "Received {n} bytes from {:?}",
                                        stream.peer_addr().unwrap()
                                    );
                                    let packet = IndexedPacket::new(id as u8, packet);
                                    queue.lock().unwrap().push(packet);
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                                Err(e) => {
                                    warn!("{e} occured with {}. Closing connection", stream.peer_addr().unwrap());
                                    break
                                }
                            }
                        }
                    });
                    listen_tasks.push(listen_task);
                }
                self.listen_tasks = Some(listen_tasks);
            }

            {
                info!("Start broadcasting");
                // broadcasting task
                let streams: Vec<_> = self
                    .streams
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|s| Arc::clone(s))
                    .collect();
                let is_running = self.is_running.clone();
                let slots_stored = self.slots_stored;
                let slot_duration = self.slot_duration;
                let broadcast_task = tokio::spawn(async move {
                    loop {
                        if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                            info!("Stop broadcasting");
                            return;
                        }

                        let data = packet_queue.lock().unwrap().take(slots_stored);
                        let bytes = packet_tools::serialize_packets(&data);

                        for stream in streams.iter() {
                            'try_send: loop {
                                let _ = stream.writable().await;
                                match stream.try_write(&bytes) {
                                    Ok(_) => {
                                        trace!("Sending: {data:?} to {:?}", stream.peer_addr());
                                        break 'try_send;
                                    }
                                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        continue;
                                    }
                                    _ => break 'try_send,
                                }
                            }
                        }
                        std::thread::sleep(slot_duration * slots_stored as u32);
                    }
                });
                self.send_task = Some(broadcast_task);
            }
        }

        pub fn decline_connections(&mut self) {
            self.accept_connections
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.connection_task.take().map(|c| c.abort());
        }

        pub fn stop(&mut self) {
            self.decline_connections();
            self.is_running
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.listen_tasks.take().map(|tasks| {
                tasks.into_iter().for_each(|t| {
                    t.abort();
                })
            });
            self.send_task.take().map(|c| c.abort());
        }
    }
    
    impl Drop for TcpSyncServer {
        fn drop(&mut self) {
            self.stop();
        }
    }
}

