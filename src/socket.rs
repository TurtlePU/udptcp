use std::net::SocketAddr;

use anyhow::Result;

use crate::packet::Packet;

pub const MAX_PACKET_SIZE: usize = 2048;
pub const CHUNK_SIZE: usize = 1024;

pub struct PacketSocket<T>(pub T);

impl PacketSocket<std::net::UdpSocket> {
    pub fn send(&self, packet: Packet) -> Result<()> {
        println!("sending packet {:?}", packet);
        let packet = packet.into_bytes();
        assert!(packet.len() == self.0.send(&packet)?);
        println!("sent packet");
        Ok(())
    }

    pub fn recv(&self) -> Result<Option<Packet>> {
        println!("receiving packet");
        let mut buffer = Vec::new();
        buffer.resize(MAX_PACKET_SIZE, 0);
        let packet_size = self.0.recv(&mut buffer)?;
        buffer.truncate(packet_size);
        println!("received buffer of length {}", packet_size);
        let packet = Packet::from_bytes(buffer)?;
        println!("received packet {:?}", packet);
        Ok(if packet.check_sum() {
            Some(packet)
        } else {
            None
        })
    }
}

impl PacketSocket<tokio::net::UdpSocket> {
    pub async fn send_to(
        &self,
        packet: Packet,
        address: SocketAddr,
    ) -> Result<()> {
        println!("{:?}: sending packet {:?}", address, packet);
        let packet = packet.into_bytes();
        assert!(packet.len() == self.0.send_to(&packet, address).await?);
        println!("{:?}: sent packet", address);
        Ok(())
    }

    pub async fn recv_from(&self) -> Result<(Packet, SocketAddr)> {
        println!("receiving packet");
        let mut buffer = Vec::new();
        buffer.resize(MAX_PACKET_SIZE, 0);
        let (packet_size, address) = self.0.recv_from(&mut buffer).await?;
        buffer.truncate(packet_size);
        let packet = Packet::from_bytes(buffer)?;
        println!("{:?}: received packet {:?}", address, packet);
        Ok((packet, address))
    }
}
