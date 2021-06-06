use std::{
    borrow::Cow, collections::HashMap, convert::TryFrom, net::SocketAddr,
    sync::Arc,
};

use anyhow::{anyhow, Result};
use rand::{thread_rng, Rng};
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use crate::{
    packet::{Ack, Flags, Packet, PacketExtra, PseudoPacket, Seq},
    socket::PacketSocket,
};

#[tokio::main]
pub async fn start_server(address: impl ToSocketAddrs) -> Result<()> {
    let socket = Arc::new(PacketSocket(UdpSocket::bind(address).await?));
    println!("Started on {:?}", socket.0.local_addr()?);
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(event_listener(tx.clone(), rx, socket.clone()));
    loop {
        tx.send(Event::from(socket.recv_from().await?))?;
    }
}

#[derive(Debug)]
enum Event {
    Receive(SocketAddr, Packet),
    Close(SocketAddr),
}

type Letter = (Packet, SocketAddr);

impl From<Letter> for Event {
    fn from((packet, address): Letter) -> Self {
        Self::Receive(address, packet)
    }
}

type Connections = HashMap<SocketAddr, ConnectionHandles>;
type Socket = Arc<PacketSocket<UdpSocket>>;

async fn event_listener(
    tx: UnboundedSender<Event>,
    mut rx: UnboundedReceiver<Event>,
    socket: Socket,
) -> Result<()> {
    let mut connections = Connections::default();
    let on_new_connection = |address| {
        let (ttx, rx) = mpsc::unbounded_channel();
        Connection::new(tx.clone(), rx, socket.clone(), address)
            .unwrap()
            .handles(ttx)
    };
    while let Some(event) = rx.recv().await {
        match event {
            Event::Receive(address, packet) => connections
                .entry(address)
                .or_insert_with_key(|&address| on_new_connection(address))
                .send(packet)?,
            Event::Close(address) => {
                match connections.remove(&address).unwrap().join().await {
                    Ok(()) => println!("Connection on {:?} closed", address),
                    Err(err) => {
                        println!("Connection on {:?} errored: {}", address, err)
                    }
                }
            }
        }
    }
    Ok(())
}

struct ConnectionHandles(JoinHandle<Result<()>>, UnboundedSender<Packet>);

impl ConnectionHandles {
    fn send(&mut self, packet: Packet) -> Result<()> {
        Ok(self.1.send(packet)?)
    }

    async fn join(self) -> Result<()> {
        Ok(self.0.await??)
    }
}

struct Connection {
    emitter: UnboundedSender<Event>,
    source: Source,
    socket: ConnSocket,
    header: Header,
}

impl Connection {
    fn new(
        emitter: UnboundedSender<Event>,
        source: UnboundedReceiver<Packet>,
        socket: Socket,
        address: SocketAddr,
    ) -> Result<Self> {
        Ok(Self {
            emitter,
            source: Source(source),
            header: Header::from_udp(&socket, address)?,
            socket: ConnSocket(socket, address),
        })
    }

    fn handles(mut self, sender: UnboundedSender<Packet>) -> ConnectionHandles {
        ConnectionHandles(tokio::spawn(async move {
            let result = self.task().await;
            self.close().unwrap();
            result
        }), sender)
    }

    async fn task(&mut self) -> Result<()> {
        let (seq, mut ack) = self.start_connection().await?;
        while let Some((new_ack, data)) = self.receive_chunk(seq, ack).await? {
            ack = new_ack;
            self.report(format!("{:?}", String::from_utf8(data)));
        }
        self.report("received fin");
        self.terminate_connection(seq, ack).await
    }

    async fn start_connection(&mut self) -> Result<(Seq, Ack)> {
        let packet = self.source.receive().await;
        let packet = packet.ok_or(anyhow!("Broken packet"))?;
        let ack = packet.syn().ok_or(anyhow!("Incorrect packet"))?;
        let seq = Seq(thread_rng().gen_range(0..1000));
        let new_ack = ack + 1;
        loop {
            self.socket.send(self.header.syn_ack(seq, new_ack)).await?;
            let new_seq = seq + 1;
            if let Some(packet) = self.source.receive().await {
                if let Some(new_ack_too) = packet.ack(new_seq) {
                    if new_ack.0 == new_ack_too.0 {
                        break Ok((new_seq, new_ack));
                    }
                }
            }
        }
    }

    async fn receive_chunk(
        &mut self,
        seq: Seq,
        ack: Ack,
    ) -> Result<Option<(Ack, Vec<u8>)>> {
        loop {
            self.socket.send(self.header.ack(seq, ack)).await?;
            if let Some(packet) = self.source.receive().await {
                let seq = packet.seq();
                if seq.0 == ack.0 {
                    break Ok(if packet.fin() {
                        None
                    } else {
                        let data = packet.data();
                        let new_ack = ack + u32::try_from(data.len())?;
                        Some((new_ack, Vec::from(data)))
                    });
                }
            }
        }
    }

    async fn terminate_connection(&mut self, seq: Seq, ack: Ack) -> Result<()> {
        loop {
            self.socket.send(self.header.fin_ack(seq, ack + 1)).await?;
            if let Some(packet) = self.source.receive().await {
                if let Some(new_ack) = packet.ack(seq + 1) {
                    if new_ack.0 == ack.0 + 1 {
                        break Ok(());
                    }
                }
            }
        }
    }

    fn close(&self) -> Result<()> {
        Ok(self.emitter.send(Event::Close(self.header.dest))?)
    }

    fn report(&self, message: impl Into<Cow<'static, str>>) {
        let message: Cow<str> = message.into();
        println!("[{:?}]: {}", self.header.dest, &*message);
    }
}

struct Source(UnboundedReceiver<Packet>);

impl Source {
    async fn receive(&mut self) -> Option<Packet> {
        let packet = self.0.recv().await.unwrap();
        if packet.check_sum() {
            Some(packet)
        } else {
            None
        }
    }
}

struct ConnSocket(Socket, SocketAddr);

impl ConnSocket {
    async fn send(&self, packet: Packet) -> Result<()> {
        self.0.send_to(packet, self.1).await
    }
}

struct Header {
    source: SocketAddr,
    dest: SocketAddr,
}

impl Header {
    fn from_udp(socket: &Socket, address: SocketAddr) -> Result<Self> {
        Ok(Self {
            source: socket.0.local_addr()?,
            dest: address,
        })
    }

    fn syn_ack(&self, seq: Seq, ack: Ack) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                ack,
                flags: Flags::default().flip_syn().flip_ack(),
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

    fn fin_ack(&self, seq: Seq, ack: Ack) -> Packet {
        Packet::from(PseudoPacket {
            source: self.source,
            dest: self.dest,
            seq,
            extra: PacketExtra {
                ack,
                flags: Flags::default().flip_fin().flip_ack(),
                ..Default::default()
            },
        })
    }
}
