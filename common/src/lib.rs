pub mod events;
pub mod intel;
pub mod pathing;
pub mod projectiles;

mod replicate_resource;
mod util;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use events::{
    AdjustPower, MoveWeapon, SetAutofire, SetCrewGoal, SetProjectileWeaponTarget, WeaponPower,
};
use intel::{
    CrewIntel, CrewNavIntel, CrewVisionIntel, InteriorIntel, SelfIntel, ShipIntel, SystemsIntel,
    WeaponChargeIntel,
};
use pathing::{Cell, CrewNavStatus};
use projectiles::{FiredFrom, NeedsDodgeTest, RoomTarget, Traversal, WeaponDamage};
use replicate_resource::ReplicateResExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::Deref, time::Duration};
use strum_macros::EnumIter;

pub const PROTOCOL_ID: u64 = 1;

pub fn protocol_plugin(app: &mut App) {
    // Ready state communication
    app.replicate_resource::<ReadyState>();
    app.add_client_event::<PlayerReady>(ChannelKind::Ordered);

    // Make sure intel makes it all the way to clients
    app.replicate_mapped::<SelfIntel>();
    app.replicate_mapped::<ShipIntel>();
    app.replicate::<CrewVisionIntel>();
    app.replicate::<InteriorIntel>();
    app.replicate::<WeaponChargeIntel>();
    app.replicate::<SystemsIntel>();

    // Miscellaneous
    app.replicate::<Traversal>();
    app.replicate::<WeaponDamage>();
    app.replicate::<NeedsDodgeTest>();
    app.replicate_mapped::<RoomTarget>();
    app.replicate_mapped::<FiredFrom>();
    app.replicate::<Name>();
    app.replicate::<Dead>();

    // Player inputs
    app.add_client_event::<AdjustPower>(ChannelKind::Ordered);
    app.add_client_event::<WeaponPower>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetProjectileWeaponTarget>(ChannelKind::Ordered);
    app.add_client_event::<MoveWeapon>(ChannelKind::Ordered);
    app.add_client_event::<SetCrewGoal>(ChannelKind::Ordered);
    app.add_client_event::<SetAutofire>(ChannelKind::Ordered);
}

#[derive(Serialize, Deserialize, EnumIter, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemId {
    Shields,
    Weapons,
    Engines,
}

impl std::fmt::Display for SystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shields => write!(f, "shields"),
            Self::Weapons => write!(f, "weapons"),
            Self::Engines => write!(f, "engines"),
        }
    }
}

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub enum ReadyState {
    AwaitingClients { ready_clients: HashSet<ClientId> },
    Starting { countdown: Duration },
}

impl Default for ReadyState {
    fn default() -> Self {
        Self::AwaitingClients {
            ready_clients: default(),
        }
    }
}

#[derive(Event, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PlayerReady;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Room {
    pub cells: Vec<Cell>,
}

impl Room {
    fn has_cell(&self, cell: Cell) -> bool {
        self.cells.iter().any(|x| *x == cell)
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Default)]
pub struct Dead;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Crew {
    pub name: String,
    pub nav_status: CrewNavStatus,
    pub health: f32,
    pub max_health: f32,
}

impl Crew {
    pub fn is_in_room(&self, room: &Room) -> bool {
        room.has_cell(self.nav_status.current_cell())
    }

    pub fn intel(&self) -> CrewIntel {
        CrewIntel {
            name: self.name.clone(),
            nav_status: match &self.nav_status {
                CrewNavStatus::At(cell) => CrewNavIntel::At(*cell),
                CrewNavStatus::Navigating(nav) => CrewNavIntel::Navigating(nav.current_location),
            },
            health: self.health,
            max_health: self.max_health,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Weapon(pub usize);

impl Deref for Weapon {
    type Target = WeaponType;

    fn deref(&self) -> &Self::Target {
        &WEAPONS[self.0]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponType {
    pub name: &'static str,
    pub damage: usize,
    pub power: usize,
    pub charge_time: f32,
    pub shot_speed: f32,
    pub volley_size: usize,
    pub shield_pierce: usize,
    pub uses_missile: bool,
    pub can_target_self: bool,
}

const WEAPONS: [WeaponType; 3] = [
    WeaponType {
        name: "Heavy Laser",
        damage: 2,
        power: 1,
        charge_time: 9.0,
        shot_speed: 0.35,
        volley_size: 1,
        shield_pierce: 0,
        uses_missile: false,
        can_target_self: false,
    },
    WeaponType {
        name: "Hermes Missiles",
        damage: 3,
        power: 3,
        charge_time: 14.0,
        shot_speed: 0.6,
        volley_size: 1,
        shield_pierce: 5,
        uses_missile: true,
        can_target_self: false,
    },
    WeaponType {
        name: "Burst Laser Mk I",
        damage: 1,
        power: 2,
        charge_time: 11.0,
        shot_speed: 0.6,
        volley_size: 2,
        shield_pierce: 0,
        uses_missile: false,
        can_target_self: false,
    },
];

// TODO Change this to also check piloting and manning crew skills
pub fn compute_dodge_chance(engine_power: usize) -> usize {
    engine_power * 5
}
