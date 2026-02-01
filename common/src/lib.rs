pub mod bullets;
pub mod events;
pub mod intel;
pub mod lobby;
pub mod nav;
mod replicate_resource;
pub mod ship;
pub mod util;
pub mod weapon;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bullets::{BeamTarget, FiredFrom, NeedsDodgeTest, Progress, RoomTarget, WeaponDamage};
use events::{
    AdjustPower, CrewStations, MoveWeapon, SetAutofire, SetBeamWeaponTarget, SetCrewGoal,
    SetDoorsOpen, SetProjectileWeaponTarget, WeaponPower,
};
use intel::{
    CrewIntel, CrewNavIntel, InteriorIntel, SelfIntel, ShipIntel, SystemsIntel, WeaponChargeIntel,
};
use lobby::PlayerReady;
use nav::{Cell, CrewNavStatus};
use serde::{Deserialize, Serialize};
use ship::{Dead, Room};

use crate::{
    lobby::{Ready, ReadyState},
    replicate_resource::ReplicateResExt as _,
};

pub const PROTOCOL_ID: u64 = 1;

pub fn protocol_plugin(app: &mut App) {
    // Ready state communication
    app.add_client_message::<PlayerReady>(Channel::Ordered);

    // Make sure intel makes it all the way to clients
    app.replicate::<SelfIntel>();
    app.replicate::<ShipIntel>();
    app.replicate::<InteriorIntel>();
    app.replicate::<WeaponChargeIntel>();
    app.replicate::<SystemsIntel>();

    // Miscellaneous
    app.replicate::<Progress>();
    app.replicate::<WeaponDamage>();
    app.replicate::<NeedsDodgeTest>();
    app.replicate::<RoomTarget>();
    app.replicate::<BeamTarget>();
    app.replicate::<FiredFrom>();
    app.replicate::<Dead>();
    app.replicate::<Ready>();
    app.replicate_resource::<ReadyState>();

    // Player inputs
    app.add_client_message::<AdjustPower>(Channel::Ordered);
    app.add_client_message::<WeaponPower>(Channel::Ordered);
    app.add_mapped_client_message::<SetProjectileWeaponTarget>(Channel::Ordered);
    app.add_mapped_client_message::<SetBeamWeaponTarget>(Channel::Ordered);
    app.add_client_message::<MoveWeapon>(Channel::Ordered);
    app.add_client_message::<SetCrewGoal>(Channel::Ordered);
    app.add_client_message::<SetAutofire>(Channel::Ordered);
    app.add_client_message::<SetDoorsOpen>(Channel::Ordered);
    app.add_client_message::<CrewStations>(Channel::Ordered);
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug)]
pub struct DoorState {
    pub open: bool,
    /// How much longer this door will be broken in seconds. When a boarder breaks the door, this
    /// timer gets set to some positive amount, and ticks downward every frame. If this value is
    /// zero, this door can't be operated normally.
    pub broken_timer: f32,
}

impl DoorState {
    pub fn broken(&self) -> bool {
        self.broken_timer > 0.0
    }

    pub fn is_open(&self) -> bool {
        self.open || self.broken()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Crew {
    pub race: usize,
    pub name: String,
    pub nav_status: CrewNavStatus,
    /// Current health in `[0, max_health]`.
    pub health: f32,
    /// Typical crew have 100 max health. That's why it goes up to 100 instead of from 0 to 1: if
    /// health was measured as a percentage of max health, a `[0, 1]` range would make more sense.
    pub task: CrewTask,
    pub station: Option<Cell>,
}

impl Crew {
    pub fn is_in_room(&self, room: &Room) -> bool {
        room.has_cell(self.nav_status.current_cell())
    }

    pub fn intel(&self) -> CrewIntel {
        CrewIntel {
            race: self.race,
            name: self.name.clone(),
            nav_status: match &self.nav_status {
                CrewNavStatus::At(cell) => CrewNavIntel::At(*cell),
                CrewNavStatus::Navigating(nav) => CrewNavIntel::Navigating(nav.current_location),
            },
            health: self.health,
        }
    }
}

/// Use this as a sort of cache to avoid having to constantly recompute crew actions for simple
/// things like repairing rooms. Without this, we could easily end up in a situation where we want
/// to advance a system's repair status but need to check enemy presence, fires, hull breaches, etc.
/// for the room. In addition to being a lot of friggin repeated work, it also throws lots of
/// responsibilities onto unrelated systems. Instead, we should compute a crew's current task based
/// on all those many factors, then simply access that task in all the other systems.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CrewTask {
    Idle,
    RepairSystem,
}

// TODO Change this to also check piloting and manning crew skills
pub fn compute_dodge_chance(engine_power: usize) -> usize {
    engine_power * 5
}

pub struct Race {
    pub name: &'static str,
    pub max_health: f32,
}

pub const RACES: [Race; 1] = [Race {
    name: "Human",
    max_health: 100.0,
}];
