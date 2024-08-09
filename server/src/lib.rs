pub mod lobby {
    use tokio::net::TcpStream;

    pub struct Player {
        pub id: u8,
        pub name: String,
        pub stream: TcpStream,
    }

    impl Player {
        pub fn new(id: u8, name: String, stream: TcpStream) -> Self {
            Self { id, name, stream }
        }
    }

    pub type Lobby = Vec<Player>;
}

pub mod server {
    use log::{error, info, trace, warn};
    use map_editor::map::Map as GameMap;
    use packet_tools::{
        client_packets::ClientPacket, server_packets::ServerPacket, IndexedPacket, TimedQueue,
        UnsizedPacketRead, UnsizedPacketWrite,
    };
    use std::{
        sync::{atomic::AtomicBool, Arc, Mutex}, time::Duration
    };
    use tokio::{
        self,
        net::{TcpListener, TcpStream, ToSocketAddrs},
        task::JoinHandle,
        time::sleep,
    };

    use crate::lobby::{Lobby, Player};

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    pub struct LobbyServer {
        lobby_task: JoinHandle<Lobby>,
        accept_players: Arc<AtomicBool>,
    }

    impl LobbyServer {
        pub async fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
            let listener = TcpListener::bind(addr).await?;
            let accept_players = Arc::new(AtomicBool::new(true));

            let running = accept_players.clone();
            let lobby_task: JoinHandle<Lobby> = tokio::spawn(async move {
                info!(
                    "Listening for new connections on {:?}",
                    listener.local_addr().unwrap()
                );
                let mut connections = vec![];

                while running.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::select! {
                        socket = listener.accept() => {
                            let Ok((mut socket, _)) = socket else { continue; };

                            let id = connections.len() as u8;
                            let connection_task = tokio::spawn(async move {
                                let name_packet: ClientPacket =
                                    socket.read_packet().await.expect("Corrupted packet");
                                let ClientPacket::SetName(name) = name_packet else {
                                    return Err("Wrong auth packet");
                                };
                                socket
                                    .write_packet(&ServerPacket::SetId(id))
                                    .await
                                    .expect("Unable to write id");
                                socket
                                    .write_packet(&ServerPacket::SetMap("default".to_string()))
                                    .await
                                    .expect("Unable to write map");

                                info!("{name} joined the game from: {}", socket.peer_addr().unwrap());
                                Ok(Player::new(id, name, socket))
                            });
                            
                            connections.push(connection_task);
                        },
                        _ = sleep(Duration::from_millis(100)) => {
                            continue
                        }

                    }
                }
                info!("Stop listening for new connections");

                let mut players = vec![];
                for task in connections.into_iter() {
                    if let Ok(player) = task.await.unwrap() {
                        players.push(player);
                    }
                }
                players
            });

            Ok(Self {
                lobby_task,
                accept_players,
            })
        }

        pub async fn get_lobby(self) -> Lobby {
            self.accept_players
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.lobby_task.await.unwrap()
        }
    }

    pub struct GameServer {
        players: Vec<Arc<Player>>,
        slot_duration: Duration,
        slots_stored: usize,
        listen_tasks: Vec<Option<JoinHandle<()>>>,
        send_task: Option<JoinHandle<()>>,
        running: Arc<AtomicBool>,
    }

    impl GameServer {
        pub async fn new(lobby: Lobby, slot_duration: Duration, slots_stored: usize) -> Self {
            let players: Vec<_> = lobby.into_iter().map(|player| Arc::new(player)).collect();
            Self {
                players,
                slot_duration,
                slots_stored,
                listen_tasks: vec![],
                send_task: None,
                running: Arc::new(AtomicBool::new(false)),
            }
        }

        pub async fn run<const PACKET_SIZE: usize>(&mut self) {
            self.running.store(true, std::sync::atomic::Ordering::Relaxed);
            for player in self.players.iter_mut() {
                // player is borrowed only once therefore this line won't panic
                let player = Arc::get_mut(player).unwrap();
                let _ = player.stream.write_packet(&ServerPacket::StartGame).await;
            }

            let packet_queue = Arc::new(Mutex::new(TimedQueue::<
                IndexedPacket<[u8; PACKET_SIZE], PACKET_SIZE>,
            >::new(self.slot_duration)));
            {
                let mut listen_tasks = Vec::new();
                info!("Start listening to incoming packets");
                // listening tasks
                for player in self.players.iter() {
                    let running = self.running.clone();
                    let player = player.clone();
                    let queue = packet_queue.clone();
                    let listen_task = tokio::spawn(async move {
                        while running.load(std::sync::atomic::Ordering::Relaxed) {
                            let _ = player.stream.readable().await;
                            let mut packet = [0; PACKET_SIZE];
                            match player.stream.try_read(&mut packet) {
                                Ok(0) => {
                                    warn!(
                                        "Player {} ({}) seems to have disconnected. Closing connection",
                                        player.name,
                                        player.stream.peer_addr().unwrap()
                                    );
                                    break;
                                }
                                Ok(n) => {
                                    trace!(
                                        "Received {n} bytes from {:?}",
                                        player.stream.peer_addr().unwrap()
                                    );
                                    let packet = IndexedPacket::new(player.id as u8, packet);
                                    queue.lock().unwrap().push(packet);
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    continue
                                }
                                Err(e) => {
                                    warn!(
                                        "{e} occured with {}. Closing connection",
                                        player.stream.peer_addr().unwrap()
                                    );
                                    break;
                                }
                            }
                        }
                    });
                    listen_tasks.push(Some(listen_task));
                }
                self.listen_tasks = listen_tasks;
            }

            {
                info!("Start broadcasting");
                // broadcasting task
                let running = self.running.clone();
                let players = self.players.clone();
                let slots_stored = self.slots_stored;
                let slot_duration = self.slot_duration;
                let broadcast_task = tokio::spawn(async move {
                    while running.load(std::sync::atomic::Ordering::Relaxed) {
                        let data = packet_queue.lock().unwrap().take(slots_stored);
                        let bytes = packet_tools::serialize_queue(&data);

                        for player in players.iter() {
                            'try_send: loop {
                                let _ = player.stream.writable().await;
                                match player.stream.try_write(&bytes) {
                                    Ok(_) => {
                                        trace!(
                                            "Sending: {data:?} to {:?}",
                                            player.stream.peer_addr()
                                        );
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

        pub fn stop(&mut self) {
            self.running.store(false, std::sync::atomic::Ordering::Relaxed);
            self.listen_tasks.iter_mut().for_each(|task| {
                task.take().map(|c| c.abort());
            });

            self.send_task.take().map(|c| c.abort());
            info!("Server stopped")
        }
    }

    impl Drop for GameServer {
        fn drop(&mut self) {
            self.stop();
        }
    }
}


