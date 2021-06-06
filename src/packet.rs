use std::{convert::TryInto, net::SocketAddr, ops::Add};

use anyhow::{anyhow, Result};

use crate::socket::MAX_PACKET_SIZE;

#[derive(Debug)]
pub struct Packet {
    source: Port,
    dest: Port,
    seq: Seq,
    ack: Ack,
    // NOTE: data offset actually uses 4 bits
    data_offset: u8,
    flags: Flags,
    window_size: WindowSize,
    checksum: u16,
    urgent: u16,
    // TODO: support options
    data: Vec<u8>,
}

impl Packet {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Packet> {
        let mut bytes = bytes.into_iter();
        let source = Port(read_u16(&mut bytes)?);
        let dest = Port(read_u16(&mut bytes)?);
        let seq = Seq(read_u32(&mut bytes)?);
        let ack = Ack(read_u32(&mut bytes)?);
        let (data_offset, flags) = from_u16(read_u16(&mut bytes)?);
        Ok(Packet {
            source, dest, seq, ack, data_offset, flags,
            window_size: WindowSize(read_u16(&mut bytes)?),
            checksum: read_u16(&mut bytes)?,
            urgent: read_u16(&mut bytes)?,
            // TODO: read options
            data: bytes.collect(),
        })
    }

    pub fn into_bytes(self) -> Vec<u8> {
        vec![
            Vec::from(self.source.0.to_be_bytes()),
            self.dest.0.to_be_bytes().into(),
            self.seq.0.to_be_bytes().into(),
            self.ack.0.to_be_bytes().into(),
            be_bytes(self.data_offset, self.flags),
            self.window_size.0.to_be_bytes().into(),
            self.checksum.to_be_bytes().into(),
            self.urgent.to_be_bytes().into(),
            // TODO: options' bytes
            self.data
        ].concat()
    }

    pub fn check_sum(&self) -> bool {
        // TODO: use checksum
        true
    }

    pub fn seq(&self) -> Ack {
        Ack(self.seq.0)
    }

    pub fn fin(&self) -> bool {
        self.flags.is_fin()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// returns sequence number of a packet
    pub fn syn(self) -> Option<Ack> {
        let Packet { seq, flags, .. } = self;
        if flags.is_syn() {
            Some(Ack(seq.0))
        } else {
            None
        }
    }

    /// returns sequence number of a packet
    pub fn ack(self, expected_ack: Seq) -> Option<Ack> {
        let Packet { seq, ack, flags, .. } = self;
        if flags.is_ack() && ack == expected_ack {
            Some(Ack(seq.0))
        } else {
            None
        }
    }

    /// returns sequence number of a packet
    pub fn syn_ack(self, expected_ack: Seq) -> Option<Ack> {
        let Packet { seq, ack, flags, .. } = self;
        if flags.is_ack() && flags.is_syn() && ack == expected_ack {
            Some(Ack(seq.0))
        } else {
            None
        }
    }

    /// returns sequence number of a packet
    pub fn fin_ack(self, expected_ack: Seq) -> Option<Ack> {
        let Packet { seq, ack, flags, .. } = self;
        if flags.is_ack() && flags.is_fin() && ack == expected_ack {
            Some(Ack(seq.0))
        } else {
            None
        }
    }

    pub fn check_ack(self, expected_ack: Seq) -> bool {
        let Packet { ack, flags, .. } = self;
        flags.is_ack() && ack == expected_ack
    }
}

const OFFSET_OFFSET: usize = 12;

fn be_bytes(offset: u8, flags: Flags) -> Vec<u8> {
    ((u16::from(offset) << OFFSET_OFFSET) | flags.0).to_be_bytes().into()
}

fn from_u16(value: u16) -> (u8, Flags) {
    let offset = value >> OFFSET_OFFSET;
    (offset.try_into().unwrap(), Flags(value & !(offset << OFFSET_OFFSET)))
}

fn read_u16(iter: &mut impl Iterator<Item = u8>) -> Result<u16> {
    let hi = iter.next().ok_or(anyhow!("Not enough bytes"))?;
    let lo = iter.next().ok_or(anyhow!("Not enough bytes"))?;
    Ok((u16::from(hi) << 8) | u16::from(lo))
}

fn read_u32(iter: &mut impl Iterator<Item = u8>) -> Result<u32> {
    let hi = read_u16(iter)?;
    let lo = read_u16(iter)?;
    Ok((u32::from(hi) << 16) | u32::from(lo))
}

pub struct PseudoPacket {
    pub source: SocketAddr,
    pub dest: SocketAddr,
    pub seq: Seq,
    pub extra: PacketExtra,
}

impl From<PseudoPacket> for Packet {
    fn from(packet: PseudoPacket) -> Self {
        let data_offset = packet.data_offset();
        let checksum = packet.checksum();
        Packet {
            source: packet.source.port().into(),
            dest: packet.dest.port().into(),
            seq: packet.seq,
            ack: packet.extra.ack,
            data_offset,
            flags: packet.extra.flags,
            window_size: packet.extra.window_size,
            checksum,
            urgent: packet.extra.urgent,
            data: packet.extra.data,
        }
    }
}

impl PseudoPacket {
    pub fn data_offset(&self) -> u8 {
        // TODO: use data offset (options support)
        0
    }

    pub fn checksum(&self) -> u16 {
        // TODO: use checksum
        0
    }
}

#[derive(Default)]
pub struct PacketExtra {
    pub ack: Ack,
    pub flags: Flags,
    pub window_size: WindowSize,
    pub urgent: u16,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Port(pub u16);

impl From<u16> for Port {
    fn from(port: u16) -> Self {
        Self(port)
    }
}

#[derive(Debug, Default)]
pub struct Flags(u16);

impl Flags {
    // pub ecn_nonce: bool,
    // pub cong_win_reduced: bool,
    // pub ecn_echo: bool,
    // pub urgent: bool,

    pub fn flip_ack(self) -> Self {
        Self(self.0 ^ 16)
    }

    pub fn is_ack(&self) -> bool {
        self.0 & 16 != 0
    }

    // pub push: bool,
    // pub reset: bool,

    pub fn flip_syn(self) -> Self {
        Self(self.0 ^ 2)
    }

    pub fn is_syn(&self) -> bool {
        self.0 & 2 != 0
    }

    pub fn flip_fin(self) -> Self {
        Self(self.0 ^ 1)
    }

    pub fn is_fin(&self) -> bool {
        self.0 & 1 != 0
    }
}

#[derive(Debug)]
pub struct WindowSize(u16);

impl Default for WindowSize {
    fn default() -> Self {
        Self(MAX_PACKET_SIZE.try_into().unwrap())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Seq(pub u32);

impl Add<u32> for Seq {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Ack(pub u32);

impl Add<u32> for Ack {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl PartialEq<Seq> for Ack {
    fn eq(&self, other: &Seq) -> bool {
        self.0 == other.0
    }
}
