use clap::Parser;
use std::net::{Ipv4Addr, UdpSocket};

#[derive(Parser)]
enum Cli {
    Client,
    Server,
}

fn main() {
    match Cli::parse() {
        Cli::Client => {
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 5001)).unwrap();
            socket.connect((Ipv4Addr::LOCALHOST, 5000)).unwrap();
            socket.send(&[65, 66, 67, 68]).unwrap();
        }
        Cli::Server => loop {
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 5000)).unwrap();
            let mut buf = [0; 1024];
            let (count, _) = socket.recv_from(&mut buf).unwrap();
            println!("{}", std::str::from_utf8(&buf[..count]).unwrap());
        },
    }
}
