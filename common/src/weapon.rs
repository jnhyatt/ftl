use crate::bullets::{BeamTarget, RoomTarget};
use bevy::ecs::entity::MapEntities;
use serde::{Deserialize, Serialize};

/// This represents an "physical" weapon. It is non-clonable because new instances must be produced
/// from a store or event, for example. They can be in a store inventory, ship storage or a
/// hardpoint.
pub enum Weapon {
    Projectile(ProjectileWeapon),
    Beam(BeamWeapon),
}

impl Weapon {
    pub fn new(id: WeaponId) -> Weapon {
        match id {
            WeaponId::Projectile(id) => Weapon::Projectile(ProjectileWeapon(id.0)),
            WeaponId::Beam(id) => Weapon::Beam(BeamWeapon(id.0)),
        }
    }

    pub fn id(&self) -> WeaponId {
        match self {
            Weapon::Projectile(weapon) => WeaponId::Projectile(weapon.id()),
            Weapon::Beam(weapon) => WeaponId::Beam(weapon.id()),
        }
    }
}

/// This represents the same thing as [`Weapon`], but fires projectiles. Honestly, the "projectile
/// vs beam" split needs to happen, but this might be the wrong place for it.
///
/// The wrapped `usize` is the index into the [`PROJECTILE_WEAPONS`] array.
pub struct ProjectileWeapon(usize);

impl Into<WeaponId> for ProjectileWeaponId {
    fn into(self) -> WeaponId {
        WeaponId::Projectile(self)
    }
}

impl ProjectileWeapon {
    pub fn id(&self) -> ProjectileWeaponId {
        ProjectileWeaponId(self.0)
    }
}

impl std::fmt::Debug for ProjectileWeapon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt(f)
    }
}

pub struct BeamWeapon(usize);

impl Into<WeaponId> for BeamWeaponId {
    fn into(self) -> WeaponId {
        WeaponId::Beam(self)
    }
}

impl BeamWeapon {
    pub fn id(&self) -> BeamWeaponId {
        BeamWeaponId(self.0)
    }
}

impl std::fmt::Debug for BeamWeapon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt(f)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectileStats {
    pub common: CommonStats,
    pub shot_speed: f32,
    pub volley_size: usize,
    pub shield_pierce: usize,
    pub uses_missile: bool,
    pub can_target_self: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BeamStats {
    pub common: CommonStats,
    pub speed: f32,
    pub length: f32,
}

pub trait Weaponlike: std::fmt::Debug {
    type Target: Copy + std::fmt::Debug;
    type Stats: Copy + std::fmt::Debug;
    type Id: Copy + std::fmt::Debug + Into<WeaponId>;

    fn id(&self) -> Self::Id;
}

impl Weaponlike for ProjectileWeapon {
    type Target = RoomTarget;
    type Stats = ProjectileStats;
    type Id = ProjectileWeaponId;

    fn id(&self) -> Self::Id {
        ProjectileWeaponId(self.0)
    }
}

impl Weaponlike for BeamWeapon {
    type Target = BeamTarget;
    type Stats = BeamStats;
    type Id = BeamWeaponId;

    fn id(&self) -> Self::Id {
        BeamWeaponId(self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommonStats {
    pub name: &'static str,
    pub damage: usize,
    pub power: usize,
    pub charge_time: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct ProjectileWeaponId(usize);

impl std::fmt::Debug for ProjectileWeaponId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        PROJECTILE_WEAPONS[self.0].common.name.fmt(f)
    }
}

impl std::ops::Deref for ProjectileWeaponId {
    type Target = ProjectileStats;

    fn deref(&self) -> &Self::Target {
        &PROJECTILE_WEAPONS[self.0]
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct BeamWeaponId(usize);

impl std::fmt::Debug for BeamWeaponId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        BEAM_WEAPONS[self.0].common.name.fmt(f)
    }
}

impl std::ops::Deref for BeamWeaponId {
    type Target = BeamStats;

    fn deref(&self) -> &Self::Target {
        &BEAM_WEAPONS[self.0]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum WeaponId {
    Projectile(ProjectileWeaponId),
    Beam(BeamWeaponId),
}

impl WeaponId {
    pub fn common(&self) -> &'static CommonStats {
        match self {
            WeaponId::Projectile(id) => &PROJECTILE_WEAPONS[id.0].common,
            WeaponId::Beam(id) => &BEAM_WEAPONS[id.0].common,
        }
    }

    pub fn uses_missile(&self) -> bool {
        match self {
            WeaponId::Projectile(weapon) => weapon.uses_missile,
            WeaponId::Beam(_) => false,
        }
    }
}

#[derive(MapEntities, Serialize, Deserialize, Debug)]
pub enum WeaponTarget {
    Projectile(#[entities] RoomTarget),

    Beam(#[entities] BeamTarget),
}

const PROJECTILE_WEAPONS: [ProjectileStats; 3] = [
    ProjectileStats {
        common: CommonStats {
            name: "Heavy Laser",
            damage: 2,
            power: 1,
            charge_time: 9.0,
        },
        shot_speed: 0.35,
        volley_size: 1,
        shield_pierce: 0,
        uses_missile: false,
        can_target_self: false,
    },
    ProjectileStats {
        common: CommonStats {
            name: "Hermes Missiles",
            damage: 3,
            power: 3,
            charge_time: 14.0,
        },
        shot_speed: 0.6,
        volley_size: 1,
        shield_pierce: 5,
        uses_missile: true,
        can_target_self: false,
    },
    ProjectileStats {
        common: CommonStats {
            name: "Burst Laser Mk I",
            damage: 1,
            power: 2,
            charge_time: 11.0,
        },
        shot_speed: 0.6,
        volley_size: 2,
        shield_pierce: 0,
        uses_missile: false,
        can_target_self: false,
    },
];

const BEAM_WEAPONS: [BeamStats; 2] = [
    BeamStats {
        common: CommonStats {
            name: "Pike Beam",
            damage: 1,
            power: 2,
            charge_time: 16.0,
        },
        speed: 0.8,
        length: 170.0,
    },
    BeamStats {
        common: CommonStats {
            name: "Halberd Beam",
            damage: 2,
            power: 3,
            charge_time: 17.0,
        },
        speed: 1.0,
        length: 80.0,
    },
];

pub const HEAVY_LASER: WeaponId = WeaponId::Projectile(ProjectileWeaponId(0));
pub const HERMES_MISSILES: WeaponId = WeaponId::Projectile(ProjectileWeaponId(1));
pub const BURST_LASER_MK_I: WeaponId = WeaponId::Projectile(ProjectileWeaponId(2));
pub const PIKE_BEAM: WeaponId = WeaponId::Beam(BeamWeaponId(0));
pub const HALBERD_BEAM: WeaponId = WeaponId::Beam(BeamWeaponId(1));
