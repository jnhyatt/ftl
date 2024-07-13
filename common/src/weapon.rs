use std::ops::Deref;

use serde::{Deserialize, Serialize};

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
