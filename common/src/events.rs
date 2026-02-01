use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{
    bullets::{BeamTarget, RoomTarget},
    ship::SystemId,
};

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AdjustPower {
    /// Whether to request power from or return power to the reactor.
    pub dir: PowerDir,
    /// Which system power is being adjusted for. The server handles adjusting power in useful
    /// increments -- for example, a single `AdjustPower` event targeting shields will increase
    /// power to shields by two.
    pub system: SystemId,
}

impl AdjustPower {
    pub fn request(system: SystemId) -> Self {
        Self {
            dir: PowerDir::Request,
            system,
        }
    }

    pub fn remove(system: SystemId) -> Self {
        Self {
            dir: PowerDir::Remove,
            system,
        }
    }
}

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WeaponPower {
    /// Whether to request power from or return power to the reactor.
    pub dir: PowerDir,
    /// Which weapon should be powered or depowered.
    pub weapon_index: usize,
}

#[derive(Message, MapEntities, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetProjectileWeaponTarget {
    pub weapon_index: usize,
    #[entities]
    pub target: Option<RoomTarget>,
}

#[derive(Message, MapEntities, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetBeamWeaponTarget {
    pub weapon_index: usize,
    #[entities]
    pub target: Option<BeamTarget>,
}

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MoveWeapon {
    pub weapon_index: usize,
    pub target_index: usize,
}

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetCrewGoal {
    pub crew: usize,
    pub room: usize,
}

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetAutofire(pub bool);

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SetDoorsOpen {
    Single { door: usize, open: bool },
    All { open: bool },
}

#[derive(Message, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum CrewStations {
    Save,
    Return,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PowerDir {
    Request,
    Remove,
}
