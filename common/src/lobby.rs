use std::time::Duration;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Client-to-server event indicating that the player is ready.
#[derive(Message, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PlayerReady;

/// Component added to client entities by the server and replicated to clients indicating that the
/// client is ready.
#[derive(Component, Serialize, Deserialize, Default)]
pub struct Ready;

/// Resource replicated to clients indicating the current ready state of the game -- either still
/// waiting for players to ready up or the game is starting with a countdown.
#[derive(Resource, Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub enum ReadyState {
    #[default]
    AwaitingClients,
    Starting {
        countdown: Duration,
    },
}
