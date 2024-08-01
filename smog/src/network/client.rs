use std::{sync::Arc};

use tokio::{ io::AsyncReadExt, net::{TcpStream, ToSocketAddrs}, runtime::Runtime, task::JoinHandle};

use crossbeam_channel::{unbounded, Receiver, Sender};

use packet_tools::{IndexedPacket, Packet};
pub struct TcpClient<P, const SIZE: usize> 
where P: Packet<SIZE>
{
    pub id: u8,
    runtime: Runtime,
    stream: Arc<TcpStream>,
    send_channel: Sender<P>,
    send_task: JoinHandle<()>,
    receive_channel: Receiver<Vec<IndexedPacket<P, SIZE>>>,
    receive_task: JoinHandle<()>,
    stop_channel: Sender<()>,
}

impl<P, const SIZE: usize> TcpClient<P, SIZE> 
where P: Packet<SIZE> + std::fmt::Debug
{
    pub fn new<A>(addr: A) -> Self
    where A: ToSocketAddrs
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Could not build tokio runtime");

        let (stream, id) = rt.block_on(async {
            let stream = TcpStream::connect(addr).await.unwrap();
            let mut buf = [0; 1];
            stream.readable().await.unwrap();
            stream.try_read(&mut buf).unwrap();
            let id = buf[0];
            (stream, id)
        });
        let stream = Arc::new(stream);
        let (stop_channel, stop_reader) = unbounded();
        // send task
        let stop_sending = stop_reader.clone();
        let (send_channel, r_channel) = unbounded::<P>();
        let send_stream = Arc::clone(&stream);
        let send_task = rt.spawn(async move {
            loop {
                if !stop_sending.is_empty() {break}
                match r_channel.try_recv() {
                    Ok(packet) => {
                        send_stream.writable().await.unwrap();
                        send_stream.try_write(&packet.to_bytes()).unwrap(); // TODO: error handling
                    }
                    Err(e) => ()
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
                if !stop_listening.is_empty() {break}

                receive_stream.readable().await.unwrap();
                match receive_stream.try_read(&mut buf[buf_start..]) {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        let (packets, res_len) = packet_tools::deserialize_packets(&mut buf[..buf_start+n]);
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

        Self {
            id,
            runtime: rt,
            stream,
            send_channel,
            send_task,
            receive_channel,
            receive_task,
            stop_channel
        }
    }

    pub fn stop(&self) {
        let _ = self.stop_channel.send(());
        self.send_task.abort();
        self.receive_task.abort();
    }

    pub fn get_packets(&self, limit: usize) -> Vec<Vec<IndexedPacket<P, SIZE>>> {
        let mut v = Vec::new();
        for _ in 0..limit {
            if let Ok(packets) = self.receive_channel.try_recv() {
                v.push(packets);
            }
            else {
                break
            }
        }
        v
    }

    pub fn send_packet(&self, packet: P) {
        self.send_channel.send(packet).unwrap();
    }

    pub fn send_packets(&self, packets: &[P]) {
        for &packet in packets {
            self.send_packet(packet);
        }
    }

    pub fn is_finished(&self) -> bool {
        self.send_task.is_finished() && self.receive_task.is_finished()
    }
} 

impl<P, const SIZE: usize> Drop for TcpClient<P, SIZE> 
where P: Packet<SIZE>
{
    fn drop(&mut self) {
        self.stop();
    }
}