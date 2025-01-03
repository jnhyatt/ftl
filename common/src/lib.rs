pub mod bullets;
pub mod events;
pub mod intel;
pub mod lobby;
pub mod nav;
pub mod ship;
pub mod util;
pub mod weapon;

mod replicate_resource;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bullets::{BeamTarget, FiredFrom, NeedsDodgeTest, Progress, RoomTarget, WeaponDamage};
use events::{
    AdjustPower, CrewStations, MoveWeapon, SetAutofire, SetBeamWeaponTarget, SetCrewGoal,
    SetDoorsOpen, SetProjectileWeaponTarget, WeaponPower,
};
use intel::{
    CrewIntel, CrewNavIntel, CrewVisionIntel, InteriorIntel, SelfIntel, ShipIntel, SystemsIntel,
    WeaponChargeIntel,
};
use lobby::{PlayerReady, ReadyState};
use nav::{Cell, CrewNavStatus};
use replicate_resource::ReplicateResExt;
use serde::{Deserialize, Serialize};
use ship::{Dead, Room};

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
    app.replicate::<Progress>();
    app.replicate::<WeaponDamage>();
    app.replicate::<NeedsDodgeTest>();
    app.replicate_mapped::<RoomTarget>();
    app.replicate_mapped::<BeamTarget>();
    app.replicate_mapped::<FiredFrom>();
    app.replicate::<Dead>();

    // Player inputs
    app.add_client_event::<AdjustPower>(ChannelKind::Ordered);
    app.add_client_event::<WeaponPower>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetProjectileWeaponTarget>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetBeamWeaponTarget>(ChannelKind::Ordered);
    app.add_client_event::<MoveWeapon>(ChannelKind::Ordered);
    app.add_client_event::<SetCrewGoal>(ChannelKind::Ordered);
    app.add_client_event::<SetAutofire>(ChannelKind::Ordered);
    app.add_client_event::<SetDoorsOpen>(ChannelKind::Ordered);
    app.add_client_event::<CrewStations>(ChannelKind::Ordered);
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
