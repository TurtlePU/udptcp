use std::{
    io::{self, Read},
    net::{ToSocketAddrs, UdpSocket},
};

use anyhow::Result;
use itertools::Itertools;
use rand::Rng;

pub fn start_client(address: impl ToSocketAddrs) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
    let client = Client(socket);
    let seq = rand::thread_rng().gen_range(0..1000);
    let mut num = client.start_connection(seq)?;
    for chunk in &io::stdin().bytes().chunks(1024) {
        let chunk = chunk.collect::<Result<Vec<_>, _>>()?;
        num = client.send_chunk(num, &chunk[..])?;
    }
    client.end_connection()
}

struct Numbers {
    _sequence: u32,
    _acknowledgment: u32,
}

struct Client(UdpSocket);

impl Client {
    fn start_connection(&self, _seq: u32) -> Result<Numbers> {
        todo!()
    }

    fn send_chunk(&self, _num: Numbers, _chunk: &[u8]) -> Result<Numbers> {
        todo!()
    }

    fn end_connection(&self) -> Result<()> {
        todo!()
    }
}
