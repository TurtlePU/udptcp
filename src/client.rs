use std::{
    convert::TryFrom,
    io::{self, Read},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
};

use anyhow::Result;
use itertools::Itertools;
use rand::Rng;

use crate::{
    packet::{Ack, Flags, Packet, PacketExtra, PseudoPacket, Seq},
    socket::{PacketSocket, CHUNK_SIZE},
};

pub fn start_client(address: impl ToSocketAddrs) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
    let client = Client::from_udp(socket)?;
    let seq = rand::thread_rng().gen_range(0..1000);
    let mut seq = client.start_connection(Seq(seq))?;
    for chunk in &io::stdin().bytes().chunks(CHUNK_SIZE) {
        let chunk = chunk.collect::<Result<Vec<_>, _>>()?;
        seq = client.send_chunk(seq, chunk)?;
    }
    client.end_connection(seq)
}

struct Client {
    header: Header,
    socket: Socket,
}

type Socket = PacketSocket<UdpSocket>;

impl Client {
    fn from_udp(socket: UdpSocket) -> Result<Self> {
        Ok(Self {
            header: Header::from_udp(&socket)?,
            socket: PacketSocket(socket),
        })
    }

    fn start_connection(&self, seq: Seq) -> Result<Seq> {
        let new_seq = seq + 1;
        let ack = loop {
            self.socket.send(self.header.syn(seq))?;
            if let Some(packet) = self.socket.recv()? {
                if let Some(ack) = packet.syn_ack(new_seq) {
                    break ack;
                }
            }
        };
        let new_seq = seq + 1;
        self.socket.send(self.header.ack(new_seq, ack + 1))?;
        Ok(new_seq)
    }

    fn send_chunk(&self, seq: Seq, chunk: Vec<u8>) -> Result<Seq> {
        loop {
            self.socket.send(self.header.data(seq, &chunk))?;
            let expected_ack = seq + u32::try_from(chunk.len())?;
            if let Some(packet) = self.socket.recv()? {
                if packet.check_ack(expected_ack) {
                    break Ok(expected_ack);
                }
            }
        }
    }

    fn end_connection(&self, seq: Seq) -> Result<()> {
        let ack = loop {
            self.socket.send(self.header.fin(seq))?;
            if let Some(packet) = self.socket.recv()? {
                if let Some(ack) = packet.fin_ack(seq + 1) {
                    break ack;
                }
            }
        };
        self.socket.send(self.header.ack(seq + 1, ack + 1))
    }
}

struct Header {
    source: SocketAddr,
    dest: SocketAddr,
}

impl Header {
    fn from_udp(socket: &UdpSocket) -> Result<Self> {
        Ok(Self {
            source: socket.local_addr()?,
            dest: socket.peer_addr()?,
        })
    }

    fn syn(&self, seq: Seq) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                flags: Flags::default().flip_syn(),
                ..Default::default()
            },
        })
    }

    fn ack(&self, seq: Seq, ack: Ack) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                ack,
                flags: Flags::default().flip_ack(),
                ..Default::default()
            },
        })
    }

    fn fin(&self, seq: Seq) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                flags: Flags::default().flip_fin(),
                ..Default::default()
            },
        })
    }

    fn data(&self, seq: Seq, data: &[u8]) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                data: data.into(),
                ..Default::default()
            },
        })
    }
}
