use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::Result;
use rand::Rng;
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    sync::{Mutex, RwLock},
};

#[tokio::main]
pub async fn start_server(address: impl ToSocketAddrs) -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(address).await?);
    let conns = Arc::new(RwLock::new(Connections::default()));
    loop {
        let (packet, addr) = next_packet(&socket).await?;
        let socket = Arc::clone(&socket);
        let conns = Arc::clone(&conns);
        tokio::spawn(async move {
            let conn = {
                let conns = conns.read().await;
                conns.find_addressant(addr)
            };
            let conn = match conn {
                Some(conn) => conn,
                None => {
                    let mut conns = conns.write().await;
                    let ack = rand::thread_rng().gen_range(0..1000);
                    conns.create_connection(addr, ack)
                }
            };
            let mut conn = conn.lock().await;
            let verdict = conn.respond_to(&socket, packet).await.unwrap();
            if let Verdict::_Close = verdict {
                conns.write().await.remove_connection(addr);
            }
        });
    }
}

struct Packet;

async fn next_packet(_socket: &UdpSocket) -> Result<(Packet, SocketAddr)> {
    todo!()
}

struct Connection {
    _ack: u32,
    // ...
}

enum Verdict {
    _Keep,
    _Close,
}

impl Connection {
    async fn respond_to(&mut self, _socket: &UdpSocket, _packet: Packet) -> Result<Verdict> {
        todo!()
    }
}

type SyncConnection = Arc<Mutex<Connection>>;

#[derive(Default)]
struct Connections(HashMap<SocketAddr, SyncConnection>);

impl Connections {
    fn find_addressant(&self, address: SocketAddr) -> Option<SyncConnection> {
        self.0.get(&address).cloned()
    }

    fn create_connection(&mut self, address: SocketAddr, ack: u32) -> SyncConnection {
        let conn = Arc::new(Mutex::new(Connection { _ack: ack }));
        self.0.insert(address, Arc::clone(&conn));
        conn
    }

    fn remove_connection(&mut self, address: SocketAddr) {
        self.0.remove(&address);
    }
}
