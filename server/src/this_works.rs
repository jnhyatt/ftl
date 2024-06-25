use clap::Parser;
use std::net::{Ipv6Addr, UdpSocket};

#[derive(Parser)]
enum Cli {
    Client,
    Server,
}

fn main() {
    match Cli::parse() {
        Cli::Client => {
            let socket = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).unwrap();
            socket
                .connect((
                    Ipv6Addr::new(0x2a01, 0x4ff, 0x1f0, 0x9230, 0x0, 0x0, 0x0, 0x1),
                    5000,
                ))
                .unwrap();
            socket.send(&[65, 66, 67, 68]).unwrap();
        }
        Cli::Server => loop {
            let socket = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 5000)).unwrap();
            let mut buf = [0; 1024];
            let (count, _) = socket.recv_from(&mut buf).unwrap();
            println!("{}", std::str::from_utf8(&buf[..count]).unwrap());
        },
    }
}
