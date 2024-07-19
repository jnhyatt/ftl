pub mod events;
pub mod intel;
pub mod lobby;
pub mod nav;
pub mod projectiles;
pub mod ship;
pub mod weapon;

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
use lobby::{PlayerReady, ReadyState};
use nav::CrewNavStatus;
use projectiles::{FiredFrom, NeedsDodgeTest, RoomTarget, Traversal, WeaponDamage};
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
    app.replicate::<Traversal>();
    app.replicate::<WeaponDamage>();
    app.replicate::<NeedsDodgeTest>();
    app.replicate_mapped::<RoomTarget>();
    app.replicate_mapped::<FiredFrom>();
    app.replicate::<Dead>();

    // Player inputs
    app.add_client_event::<AdjustPower>(ChannelKind::Ordered);
    app.add_client_event::<WeaponPower>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetProjectileWeaponTarget>(ChannelKind::Ordered);
    app.add_client_event::<MoveWeapon>(ChannelKind::Ordered);
    app.add_client_event::<SetCrewGoal>(ChannelKind::Ordered);
    app.add_client_event::<SetAutofire>(ChannelKind::Ordered);
}

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

// TODO Change this to also check piloting and manning crew skills
pub fn compute_dodge_chance(engine_power: usize) -> usize {
    engine_power * 5
}
