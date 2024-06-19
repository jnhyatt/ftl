use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

use clap::Parser;
use renet::{
    transport::{
        ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
        ServerConfig,
    },
    ConnectionConfig, RenetClient, RenetServer, ServerEvent,
};

#[derive(Parser)]
enum Cli {
    Client,
    Server,
}

fn main() {
    match Cli::parse() {
        Cli::Client => {
            let mut client = RenetClient::new(ConnectionConfig::default());
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let mut transport = NetcodeClientTransport::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap(),
                ClientAuthentication::Unsecure {
                    protocol_id: 1,
                    client_id: 2,
                    server_addr: ([127, 0, 0, 1], 5000).into(),
                    user_data: None,
                },
                socket,
            )
            .unwrap();
            loop {
                let dt = Duration::from_secs(2);
                client.update(dt);
                transport.update(dt, &mut client).unwrap();
                if client.is_connected() {
                    while let Some(message) = client.receive_message(0) {
                        println!("Received {message:?}");
                    }
                }
                transport.send_packets(&mut client).unwrap();
                std::thread::sleep(dt);
            }
        }
        Cli::Server => {
            let mut server = RenetServer::new(ConnectionConfig::default());
            let server_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 5000);
            let socket = UdpSocket::bind(server_addr).unwrap();
            let mut transport = NetcodeServerTransport::new(
                ServerConfig {
                    current_time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap(),
                    max_clients: 2,
                    protocol_id: 1,
                    public_addresses: vec![server_addr],
                    authentication: ServerAuthentication::Unsecure,
                },
                socket,
            )
            .unwrap();
            loop {
                let dt = Duration::from_secs(1);
                server.update(dt);
                transport.update(dt, &mut server).unwrap();
                while let Some(event) = server.get_event() {
                    match event {
                        ServerEvent::ClientConnected { client_id } => println!("connec{client_id}"),
                        ServerEvent::ClientDisconnected { client_id, .. } => print!("d{client_id}"),
                    }
                }
                server.broadcast_message(0, "blah blah");
                transport.send_packets(&mut server);
                std::thread::sleep(dt);
            }
        }
    }
}
