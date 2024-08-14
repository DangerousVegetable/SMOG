use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use common::RELATIVE_MAPS_PATH;
use map_editor::map::MapLoader;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, ToSocketAddrs},
    runtime::Runtime,
    task::JoinHandle,
};

use crossbeam_channel::{unbounded, Receiver, Sender};

use packet_tools::{
    client_packets::ClientPacket, server_packets::ServerPacket, IndexedPacket, Packet,
    UnsizedPacketRead, UnsizedPacketWrite,
};

use crate::network::error::ClientError;

pub struct LobbyInfo {
    pub id: u8,
    pub map: String,
    pub players: Vec<(u8, String)>,
}

pub struct GameClient<P, const SIZE: usize>
where
    P: Packet<SIZE>,
{
    pub name: String,
    pub lobby: LobbyInfo,
    runtime: Runtime,
    lobby_channel: Receiver<ServerPacket>,
    lobby_task: Option<JoinHandle<Result<(LobbyInfo, TcpStream)>>>,
    send_channel: Option<Sender<P>>,
    send_task: Option<JoinHandle<()>>,
    receive_channel: Option<Receiver<Vec<IndexedPacket<P, SIZE>>>>,
    receive_task: Option<JoinHandle<()>>,
    stop_channel: Option<Sender<()>>,
}

impl<P, const SIZE: usize> GameClient<P, SIZE>
where
    P: Packet<SIZE> + std::fmt::Debug,
{
    pub fn new<A>(addr: A, name: String) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let (id, name, stream) = rt.block_on(async {
            let mut stream = TcpStream::connect(addr).await?;
            stream
                .write_packet(&ClientPacket::SetName(name.clone()))
                .await?;
            let ServerPacket::SetId(id) = stream.read_packet().await? else {
                return Result::Err(ClientError::AuthenticationError)?;
            };

            anyhow::Ok((id, name, stream))
        })?;

        let mut lobby_stream = stream;
        let (send_lobby, receive_lobby) = unbounded();
        let lobby_task = rt.spawn(async move {
            let mut id = id;
            let mut map = String::new();
            let mut players = Vec::new();
            loop {
                let packet = lobby_stream.read_packet().await?;
                match packet {
                    ServerPacket::StartGame => {
                        let lobby = LobbyInfo { id, map, players };
                        return anyhow::Ok((lobby, lobby_stream));
                    }
                    ServerPacket::SetId(new_id) => id = new_id,
                    ServerPacket::SetMap(new_map) => {
                        map = new_map;
                        if !MapLoader::map_exists(&map, common::RELATIVE_MAPS_PATH) {
                            lobby_stream
                                .write_packet(&ClientPacket::RequestMap)
                                .await?
                        } else {
                            lobby_stream.write_packet(&ClientPacket::Ok).await?;
                        }
                    }
                    ServerPacket::SetPlayers(new_players) => players = new_players,
                    ServerPacket::CreateFile { name, contents } => {
                        let mut file_path = PathBuf::from(RELATIVE_MAPS_PATH);
                        file_path.push(&map);
                        tokio::fs::create_dir_all(&file_path).await?;
                        file_path.push(name);

                        tokio::fs::File::create(&file_path).await?
                            .write_all(&contents).await?
                    }
                    _ => send_lobby.send(packet)?,
                }
            }
        });

        Ok(Self {
            name,
            lobby: LobbyInfo {
                id,
                map: "default".to_string(),
                players: vec![],
            },
            runtime: rt,
            lobby_channel: receive_lobby,
            lobby_task: Some(lobby_task),
            send_channel: None,
            send_task: None,
            receive_channel: None,
            receive_task: None,
            stop_channel: None,
        })
    }

    pub fn get_lobby_packets(&self) -> Vec<ServerPacket> {
        let mut packets = vec![];
        while let Ok(packet) = self.lobby_channel.try_recv() {
            packets.push(packet);
        }
        packets
    }

    pub fn game_started(&self) -> bool {
        self.lobby_task
            .as_ref()
            .map_or(true, |task| task.is_finished())
    }

    pub fn run(&mut self) -> Result<()> {
        let rt = &self.runtime;
        let (lobby, stream) = self
            .runtime
            .block_on(async { self.lobby_task.take().ok_or(ClientError::NoConnectionToServer)?.await? })?;
        let stream = Arc::new(stream);
        let (stop_channel, stop_reader) = unbounded();

        // send task
        let stop_sending = stop_reader.clone();
        let (send_channel, r_channel) = unbounded::<P>();
        let send_stream = Arc::clone(&stream);
        let send_task = rt.spawn(async move {
            loop {
                if !stop_sending.is_empty() {
                    break;
                }
                match r_channel.try_recv() {
                    Ok(packet) => {
                        send_stream.writable().await.unwrap();
                        send_stream.try_write(&packet.to_bytes()).unwrap(); // TODO: error handling
                    }
                    Err(e) => (),
                }
            }
        });
        // listen task
        let stop_listening = stop_reader.clone();
        let (s_channel, receive_channel) = unbounded::<Vec<IndexedPacket<P, SIZE>>>();
        let receive_stream = Arc::clone(&stream);
        let receive_task = rt.spawn(async move {
            let mut buf_start = 0;
            let mut buf = Vec::from([0; 4096]);
            loop {
                if !stop_listening.is_empty() {
                    break;
                }

                receive_stream.readable().await.unwrap();
                match receive_stream.try_read(&mut buf[buf_start..]) {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        let (packets, res_len) =
                            packet_tools::deserialize_queue(&mut buf[..buf_start + n]);
                        buf_start = res_len;
                        if buf_start > buf.len() / 2 {
                            buf.extend((0..buf.len()).into_iter().map(|_| 0));
                        }

                        for p in packets {
                            s_channel.send(p).unwrap();
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        break;
                    }
                }
            }
        });

        self.lobby = lobby;
        self.send_channel = Some(send_channel);
        self.send_task = Some(send_task);
        self.receive_channel = Some(receive_channel);
        self.receive_task = Some(receive_task);
        self.stop_channel = Some(stop_channel);

        anyhow::Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_channel.take().map(|channel| channel.send(()));
        self.send_task.take().map(|task| task.abort());
        self.receive_task.take().map(|task| task.abort());
    }

    pub fn get_packets(&self, limit: usize) -> Vec<Vec<IndexedPacket<P, SIZE>>> {
        let Some(channel) = self.receive_channel.as_ref() else {
            return vec![];
        };
        let mut v = Vec::new();
        for _ in 0..limit {
            if let Ok(packets) = channel.try_recv() {
                v.push(packets);
            } else {
                break;
            }
        }
        v
    }

    pub fn send_packet(&self, packet: P) {
        self.send_channel
            .as_ref()
            .map(|channel| channel.send(packet).unwrap());
    }

    pub fn send_packets(&self, packets: &[P]) {
        for &packet in packets {
            self.send_packet(packet);
        }
    }

    pub fn is_finished(&self) -> bool {
        self.send_task
            .as_ref()
            .map_or(true, |task| task.is_finished())
            && self
                .receive_task
                .as_ref()
                .map_or(true, |task| task.is_finished())
    }
}

impl<P, const SIZE: usize> Drop for GameClient<P, SIZE>
where
    P: Packet<SIZE>,
{
    fn drop(&mut self) {
        self.stop();
    }
}
