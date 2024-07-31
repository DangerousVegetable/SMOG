use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub trait Packet<const SIZE: usize>: Clone + Copy + Send + Sync + 'static + std::fmt::Debug {
    fn to_bytes(&self) -> [u8; SIZE];
    fn from_bytes(value: &[u8; SIZE]) -> Self;
}

impl<const SIZE: usize> Packet<SIZE> for [u8; SIZE] {
    fn from_bytes(value: &[u8; SIZE]) -> Self {
        value.clone()
    }

    fn to_bytes(&self) -> [u8; SIZE] {
        self.clone()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IndexedPacket<P: Packet<SIZE>, const SIZE: usize> {
    pub id: u8,
    pub contents: P,
}

impl<P: Packet<SIZE>, const SIZE: usize> IndexedPacket<P, SIZE> {
    pub fn new(id: u8, contents: P) -> Self {
        Self { id, contents }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.id];
        bytes.extend(self.contents.to_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            id: bytes[0],
            contents: P::from_bytes(bytes[1..].try_into().unwrap())
        }
    }
}

pub fn serialize_packets<P: Packet<SIZE>, const SIZE: usize>(
    packets: &Vec<Vec<IndexedPacket<P, SIZE>>>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for packets in packets.iter() {
        bytes.push(packets.len() as u8);
        bytes.extend(packets.iter().map(|p| p.to_bytes()).flatten());
    }
    bytes
}

pub fn deserialize_packets<P: Packet<SIZE>, const SIZE: usize>(
    bytes: &[u8],
) -> Result<Vec<Vec<IndexedPacket<P, SIZE>>>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let mut ind = 0;
    while ind < bytes.len() {
        let len = bytes[ind] as usize;
        ind += 1;
        let mut packets = Vec::new();
        for packet_bytes in bytes[ind..].chunks(SIZE+1).take(len) {
            packets.push(IndexedPacket::from_bytes(packet_bytes));
        }
        result.push(packets);

        ind += (SIZE+1) * len;
    }
    Ok(result)
}

pub struct TimedQueue<P> {
    pub queue: VecDeque<Vec<P>>,
    delta: Duration, // time delay between Packets in queue
    time: Instant,   // time stamp of the first Packets in queue
}

impl<P> TimedQueue<P>
where
    P: Clone + Copy,
{
    pub fn new(delta: Duration) -> Self {
        Self {
            queue: [vec![]].into(),
            delta,
            time: Instant::now(),
        }
    }

    pub fn push(&mut self, element: P) {
        if self.queue.is_empty() {
            self.queue.push_back(vec![]);
        }

        let now = Instant::now();
        let ind = ((now - self.time).as_nanos() / self.delta.as_nanos()) as usize;
        let empty_size = usize::max(ind, self.queue.len() - 1) - (self.queue.len() - 1);
        let mut empty_array: VecDeque<Vec<P>> = vec![vec![]; empty_size].into();
        self.queue.append(&mut empty_array);
        let Some(last) = self.queue.back_mut() else {
            return;
        };
        last.push(element);
    }

    pub fn take(&mut self, num: usize) -> Vec<Vec<P>> {
        self.time = Instant::now();
        let mut head: Vec<_> = self
            .queue
            .drain(0..usize::min(num, self.queue.len()))
            .collect();
        head.append(&mut vec![vec![]; num - head.len()]);
        head
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    type Result = std::result::Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn timed_queue_test() {
        let dur = Duration::from_millis(1);
        let mut q = TimedQueue::<usize>::new(dur);
        q.push(1);
        q.push(2);
        sleep(dur);

        q.push(3);
        q.push(4);
        q.push(5);
        sleep(dur * 2);

        q.push(6);

        let v: Vec<Vec<usize>> = q.take(6);
        assert_eq!(
            vec![vec![1, 2], vec![3, 4, 5], vec![], vec![6], vec![], vec![]],
            v
        );
    }
}
