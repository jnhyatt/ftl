use std::{collections::HashSet, time::Duration};

use bevy::{ecs::event::Event, prelude::Resource};
use bevy_replicon::core::ClientId;
use serde::{Deserialize, Serialize};

#[derive(Event, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PlayerReady;

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub enum ReadyState {
    AwaitingClients { ready_clients: HashSet<ClientId> },
    Starting { countdown: Duration },
}

impl Default for ReadyState {
    fn default() -> Self {
        Self::AwaitingClients {
            ready_clients: HashSet::default(),
        }
    }
}
