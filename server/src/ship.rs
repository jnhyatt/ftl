use bevy::prelude::*;
use common::{
    intel::{
        BasicIntel, CrewVisionIntel, InteriorIntel, RoomIntel, SelfIntel, ShieldIntel,
        SystemsIntel, WeaponChargeIntel, WeaponIntel, WeaponsIntel,
    },
    nav::{Cell, CrewNav, CrewNavStatus, LineSection, NavMesh, PathGraph, SquareSection},
    projectiles::RoomTarget,
    ship::{Room, SystemId},
    Crew,
};
use strum::IntoEnumIterator;

use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, ShipSystems},
    weapons::ProjectileInfo,
};

#[derive(Component, Clone, Debug)]
pub struct Ship {
    pub reactor: Reactor,
    pub systems: ShipSystems,
    pub max_hull: usize,
    pub damage: usize,
    pub rooms: Vec<Room>,
    pub crew: Vec<Crew>,
    pub nav_mesh: NavMesh,
    pub path_graph: PathGraph,
    pub missiles: usize,
}

impl Ship {
    pub fn new() -> Self {
        let rooms = vec![
            Room {
                cells: vec![Cell(0), Cell(1), Cell(2), Cell(3)],
            },
            Room {
                cells: vec![Cell(4), Cell(5)],
            },
            Room {
                cells: vec![Cell(6), Cell(7)],
            },
        ];
        let nav_mesh = NavMesh {
            lines: vec![
                LineSection([Cell(4), Cell(5)]),
                LineSection([Cell(6), Cell(7)]),
                LineSection([Cell(3), Cell(5)]),
                LineSection([Cell(5), Cell(7)]),
            ],
            squares: vec![SquareSection([[Cell(0), Cell(1)], [Cell(2), Cell(3)]])],
        };
        let path_graph = PathGraph {
            edges: [
                (Cell(0), [Cell(1), Cell(2), Cell(3)].into()),
                (Cell(1), [Cell(0), Cell(2), Cell(3)].into()),
                (Cell(2), [Cell(0), Cell(1), Cell(3)].into()),
                (Cell(3), [Cell(0), Cell(1), Cell(2), Cell(5)].into()),
                (Cell(4), [Cell(5)].into()),
                (Cell(5), [Cell(3), Cell(4), Cell(7)].into()),
                (Cell(6), [Cell(7)].into()),
                (Cell(7), [Cell(5), Cell(6)].into()),
            ]
            .into(),
        };
        Self {
            reactor: Reactor::new(0),
            systems: default(),
            max_hull: 30,
            damage: 0,
            rooms,
            crew: default(),
            nav_mesh,
            path_graph,
            missiles: 10,
        }
    }

    pub fn self_intel(&self, ship: Entity) -> SelfIntel {
        SelfIntel {
            ship,
            max_power: self.reactor.upgrade_level,
            free_power: self.reactor.available,
            missiles: self.missiles,
            weapon_targets: self
                .systems
                .weapons
                .as_ref()
                .map(|(weapons, _)| weapons.weapons().iter().map(|x| x.target()).collect())
                .unwrap_or_default(),
            crew: self.crew.clone(),
        }
    }

    pub fn basic_intel(&self) -> BasicIntel {
        BasicIntel {
            max_hull: self.max_hull,
            hull: self.max_hull - self.damage,
            rooms: (0..self.rooms.len())
                .map(|room| self.systems.system_in_room(room))
                .collect(),
            shields: self
                .systems
                .shields
                .as_ref()
                .map(|(shields, _)| ShieldIntel {
                    max_layers: shields.max_layers(),
                    layers: shields.layers,
                    charge: shields.charge,
                    damage: shields.damage_intel(),
                }),
            engines: self
                .systems
                .engines
                .as_ref()
                .map(|(engines, _)| engines.damage_intel()),
            weapons: self
                .systems
                .weapons
                .as_ref()
                .map(|(weapons, _)| WeaponsIntel {
                    weapons: weapons
                        .weapons()
                        .iter()
                        .map(|x| WeaponIntel {
                            weapon: x.weapon.clone(),
                            powered: x.is_powered(),
                        })
                        .collect(),
                    damage: weapons.damage_intel(),
                }),
        }
    }

    pub fn crew_vision_intel(&self) -> CrewVisionIntel {
        CrewVisionIntel
    }

    pub fn interior_intel(&self) -> InteriorIntel {
        InteriorIntel {
            rooms: self
                .rooms
                .iter()
                .map(|room| RoomIntel {
                    crew: self
                        .crew
                        .iter()
                        .filter(|x| x.is_in_room(room))
                        .map(|x| x.intel())
                        .collect(),
                })
                .collect(),
            cells: default(),
        }
    }

    pub fn weapon_charge_intel(&self) -> WeaponChargeIntel {
        WeaponChargeIntel {
            levels: self
                .systems
                .weapons
                .as_ref()
                .map(|(weapons, _)| weapons.weapons().iter().map(|x| x.charge).collect())
                .unwrap_or_default(),
        }
    }

    pub fn systems_intel(&self) -> SystemsIntel {
        SystemsIntel(
            SystemId::iter()
                .filter_map(|system| self.systems.system(system).map(|x| (system, x.intel())))
                .collect(),
        )
    }

    pub fn update_weapons(&mut self) -> Option<impl Iterator<Item = ProjectileInfo> + '_> {
        self.systems.weapons.as_mut().map(|(weapons, _)| {
            let missiles = &mut self.missiles;
            let autofire = weapons.autofire;
            weapons
                .weapons_mut()
                .filter_map(move |x| x.charge_and_fire(missiles, autofire))
        })
    }

    pub fn update_repair_status(&mut self) {
        for (i, room) in self.rooms.iter().enumerate() {
            if let Some(system) = self.systems.system_in_room(i) {
                if !self.crew.iter().any(|x| x.is_in_room(room)) {
                    let system = self.systems.system_mut(system).unwrap();
                    system.cancel_repair();
                }
            }
        }
    }

    pub fn update_crew(&mut self) {
        for crew in &mut self.crew {
            crew.nav_status.step(&self.nav_mesh);
            if let CrewNavStatus::At(cell) = &crew.nav_status {
                let room = self
                    .rooms
                    .iter()
                    .position(|x| x.cells.iter().any(|x| x == cell))
                    .unwrap();
                // if enemy_crew_in_room {
                //     KILL HIM
                // } else if fire_in_room {
                //     stop drop and roll
                // } else if hull_breach_in_room {
                //     fix it
                // } else
                if let Some(system) = self.systems.system_in_room(room) {
                    let system = self.systems.system_mut(system).unwrap();
                    if system.damage() > 0 {
                        system.crew_repair(1.0 / 768.0);
                    } else {
                        // Move to manning station if unoccupied
                        // Man system
                    }
                }
            }
        }
    }

    pub fn install_shields(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install shields in room {room}, room is already occupied.");
            return;
        }
        if self.systems.shields.is_some() {
            eprintln!("Can't install shields on ship, a shields module is already installed.");
            return;
        }
        self.systems.shields = Some((default(), room));
    }

    pub fn install_engines(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install engines in room {room}, room is already occupied.");
            return;
        }
        if self.systems.engines.is_some() {
            eprintln!("Can't install engines on ship, an engines module is already installed.");
            return;
        }
        self.systems.engines = Some((default(), room));
    }

    pub fn install_weapons(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install engines in room {room}, room is already occupied.");
            return;
        }
        if self.systems.weapons.is_some() {
            eprintln!("Can't install weapons on ship, a weapons module is already installed.");
            return;
        }
        self.systems.weapons = Some((default(), room));
    }

    pub fn request_power(&mut self, system: SystemId) {
        let Some(system) = self.systems.system_mut(system) else {
            eprintln!("Can't add power to {system}, system not installed.");
            return;
        };
        system.add_power(
            &mut self.reactor,
            PowerContext {
                missiles: self.missiles,
            },
        );
    }

    pub fn remove_power(&mut self, system: SystemId) {
        let Some(system) = self.systems.system_mut(system) else {
            eprintln!("Can't remove power from {system}, system not installed.");
            return;
        };
        system.remove_power(&mut self.reactor);
    }

    pub fn power_weapon(&mut self, index: usize) {
        let Some((weapons, _)) = &mut self.systems.weapons else {
            eprintln!("Can't power weapon, weapons system not installed.");
            return;
        };
        weapons.power_weapon(index, self.missiles, &mut self.reactor);
    }

    pub fn depower_weapon(&mut self, index: usize) {
        let Some((weapons, _)) = &mut self.systems.weapons else {
            eprintln!("Can't depower weapon, weapons system not installed.");
            return;
        };
        weapons.depower_weapon(index, &mut self.reactor);
    }

    pub fn set_projectile_weapon_target(
        &mut self,
        weapon_index: usize,
        target: Option<RoomTarget>,
        targeting_self: bool,
    ) {
        let Some((weapons, _)) = &mut self.systems.weapons else {
            eprintln!("Can't set weapon target, weapons system notinstalled.");
            return;
        };
        weapons.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }

    pub fn move_weapon(&mut self, weapon_index: usize, target_index: usize) {
        let Some((weapons, _)) = &mut self.systems.weapons else {
            eprintln!("Can't move weapon, weapons system not installed.");
            return;
        };
        weapons.move_weapon(weapon_index, target_index);
    }

    pub fn set_crew_goal(&mut self, crew_index: usize, room_index: usize) {
        let Some(room) = self.rooms.get(room_index) else {
            eprintln!("Can't set crew goal, room {room_index} doesn't exist");
            return;
        };
        let is_unoccupied = |cell: Cell| {
            // cell is unoccupied if all crew are not in it
            self.crew
                .iter()
                .all(|crew| crew.nav_status.occupied_cell() != cell)
        };
        let Some(target_cell) = room.cells.iter().cloned().find(|&x| is_unoccupied(x)) else {
            eprintln!("Can't set crew goal, room {room_index} is fully occupied.");
            return;
        };
        let Some(crew) = self.crew.get_mut(crew_index) else {
            eprintln!("Can't set crew goal, crew {crew_index} doesn't exist.");
            return;
        };
        let crew = &mut crew.nav_status;
        let occupied_room = self
            .rooms
            .iter()
            .position(|x| x.cells.iter().any(|x| *x == crew.occupied_cell()))
            .unwrap();
        if room_index == occupied_room {
            eprintln!("Can't set crew goal, crew is already in room {room_index}.");
            return;
        }

        let pathing = self.path_graph.pathing_to(target_cell);
        let Some(path) = self.nav_mesh.find_path(&pathing, crew.current_location()) else {
            eprintln!(
                "Can't set crew goal, room {room_index} is unreachable by crew {crew_index}."
            );
            return;
        };
        let current_location = match crew {
            CrewNavStatus::At(cell) => self
                .nav_mesh
                .section_with_path(*cell, path.next_waypoint().unwrap())
                .unwrap()
                .to_location(*cell),
            CrewNavStatus::Navigating(nav) => nav.current_location,
        };
        *crew = CrewNavStatus::Navigating(CrewNav {
            path,
            current_location,
        });
    }

    pub fn set_autofire(&mut self, autofire: bool) {
        let Some((weapons, _)) = &mut self.systems.weapons else {
            eprintln!("Can't set autofire, weapons system not installed.");
            return;
        };
        weapons.autofire = autofire;
    }
}
