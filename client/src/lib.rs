pub mod egui_common;
pub mod select;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    renet::{
        transport::{ClientAuthentication, NetcodeClientTransport},
        ConnectionConfig, RenetClient,
    },
    RenetChannelsExt, RepliconRenetClientPlugin,
};
use common::{protocol_plugin, PROTOCOL_ID};
use std::{
    net::{Ipv6Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

pub fn client_plugin(app: &mut App) {
    app.add_plugins((
        RepliconPlugins.build().disable::<ServerPlugin>(),
        RepliconRenetClientPlugin,
        protocol_plugin,
    ));
    app.add_systems(Startup, connect_to_server);
}

fn connect_to_server(channels: Res<RepliconChannels>, mut commands: Commands) {
    let server_channels_config = channels.get_server_configs();
    let client_channels_config = channels.get_client_configs();
    commands.insert_resource(RenetClient::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..default()
    }));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;
    let server_addr = SocketAddr::new(
        // Ipv6Addr::new(0x2a01, 0x4ff, 0x1f0, 0x9230, 0x0, 0x0, 0x0, 0x1).into(),
        Ipv6Addr::LOCALHOST.into(),
        5000,
    );
    let socket = UdpSocket::bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))).unwrap();
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
    commands.insert_resource(transport);
}
