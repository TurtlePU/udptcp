mod server;
mod client;
mod socket;
mod packet;

use anyhow::Result;
use clap::clap_app;

use crate::{client::start_client, server::start_server};

fn main() -> Result<()> {
    let matches = clap_app!(tcpudp =>
        (version: "1.0")
        (author: "Pavel Sokolov <sokolov.p64@gmail.com>")
        (about: "TCP over UDP, server and client")
        (@arg SERVER: -s --server conflicts_with[CLIENT] "Start as server")
        (@arg CLIENT: -c --client "Start as client (default)")
        (@arg HOST: -H --host <HOST> "Hostname")
        (@arg PORT: -p --port <PORT> "Port")
    )
    .get_matches();

    let address = format!(
        "{}:{}",
        matches.value_of("HOST").unwrap(),
        matches.value_of("PORT").unwrap()
    );

    if matches.is_present("SERVER") {
        start_server(address)
    } else {
        start_client(address)
    }
}
