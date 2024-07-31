pub const PACKET_SIZE: usize = 9;

pub mod tcp {
    use log::{error, info, trace};
use packet_tools::{IndexedPacket, TimedQueue};
use tokio::{self, io::AsyncWriteExt, net::{TcpListener, TcpStream, ToSocketAddrs}, time::sleep};
use std::{sync::{atomic::AtomicBool, Arc, Mutex}, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct TcpSyncServer {
    pub listener: Arc<TcpListener>,
    slot_duration: Duration,
    slots_stored: usize,
    streams: Arc<Mutex<Vec<Arc<TcpStream>>>>,
    listen_for_connections: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
}

impl TcpSyncServer {
    pub async fn new<A>(addr: A, slot_duration: Duration, slots_stored: usize) -> Result<Self> 
    where A: ToSocketAddrs
    {
        let listener = TcpListener::bind(addr).await?;
        Ok(
            Self {
                listener: Arc::new(listener),
                slot_duration,
                slots_stored,
                streams: Arc::new(Mutex::new(vec![])),
                listen_for_connections: Arc::new(AtomicBool::new(true)),
                is_running: Arc::new(AtomicBool::new(false)),
            }
        )
    }

    pub fn listen_for_connections(&self) {
        let accept_connections = self.listen_for_connections.clone();
        let listener = self.listener.clone();
        let streams = self.streams.clone();
        
        // listening for connections
        tokio::spawn(async move {
            info!("listening for new connections on {:?}", listener.local_addr().unwrap());
            let mut new_streams = Vec::new();
            while accept_connections.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((mut stream, _)) = result {
                            info!("accepted connection: {:?}", stream.peer_addr().unwrap());
                            stream.write(&[new_streams.len() as u8]).await.unwrap();
                            new_streams.push(Arc::new(stream));
                        }
                    },
                    _ = sleep(Duration::from_millis(100)) => {
                        continue
                    }
                }
            }
            
            *streams.lock().unwrap() = new_streams;
            info!("stop listening for new connections");
        });
    }

    pub fn run<const PACKET_SIZE: usize>(&self) {
        self.stop_listening_for_new_connections();
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        let packet_queue = Arc::new(Mutex::new(TimedQueue::<IndexedPacket<[u8; PACKET_SIZE], PACKET_SIZE>>::new(self.slot_duration)));
        
        {
            info!("start listening to incoming packets");
            // listening tasks
            let streams = self.streams.lock().unwrap();
            for (id, stream) in streams.iter().enumerate() {
                let is_running = self.is_running.clone();
                let stream = stream.clone();
                let queue = packet_queue.clone();
                tokio::spawn(async move {
                    loop {
                        if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                            info!("closing connection with {:?}", stream.peer_addr().unwrap());
                            break;
                        }
                        stream.readable().await.unwrap();
                        let mut packet = [0; PACKET_SIZE];
                        if let Ok(num) = stream.try_read(&mut packet)
                        {
                            trace!("received {num} bytes from {:?}", stream.peer_addr().unwrap());
                            let packet = IndexedPacket::new(id as u8, packet);
                            queue.lock().unwrap().push(packet);
                        }
                    }
                });
            }
        }

        {
            info!("start broadcasting");
            // broadcasting task
            let streams: Vec<_> = self.streams.lock().unwrap().iter()
                .map(|s| Arc::clone(s))
                .collect();
            let is_running = self.is_running.clone();
            let slots_stored = self.slots_stored;
            let slot_duration = self.slot_duration;
            tokio::spawn(async move {
                let mut cycle = 1;
                loop {
                    if !is_running.load(std::sync::atomic::Ordering::Relaxed) {
                        info!("stop broadcasting");
                        return;
                    }

                    if cycle % slots_stored == 0 {
                        let data = packet_queue.lock().unwrap().take(slots_stored);
                        let bytes = packet_tools::serialize_packets(&data);
                        
                        for stream in streams.iter() {
                            stream.writable().await.unwrap();
                            if let Err(e) = stream.try_write(&bytes) {
                                error!("{e} occured while sending to {:?}", stream.peer_addr());
                            }
                            else {
                                trace!("sending: {data:?} to {:?}", stream.peer_addr());
                            }
                        }
                    }
                    cycle += 1;
                    std::thread::sleep(slot_duration);
                    //sleep(slot_duration).await;
                }
            });
        }
    }

    pub fn stop_listening_for_new_connections(&self) {
        self.listen_for_connections.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn stop(&self) {
        self.stop_listening_for_new_connections();
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
}