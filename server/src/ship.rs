use bevy::prelude::*;
use common::{
    intel::{
        BasicIntel, CrewVisionIntel, InteriorIntel, RoomIntel, SelfIntel, ShieldIntel,
        SystemsIntel, WeaponChargeIntel, WeaponIntel, WeaponsIntel,
    },
    nav::{Cell, CrewNav, CrewNavStatus, NavMesh, PathGraph},
    projectiles::RoomTarget,
    ship::{SystemId, SHIPS},
    util::IterAvg,
    Crew,
};
use strum::IntoEnumIterator;

use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, ShipSystems},
    weapons::ProjectileInfo,
};

#[derive(Component, Clone, Debug)]
pub struct ShipState {
    pub ship_type: usize,
    pub reactor: Reactor,
    pub systems: ShipSystems,
    pub max_hull: usize,
    pub damage: usize,
    pub crew: Vec<Crew>,
    pub missiles: usize,
    /// Oxygen level for each room in `[0, 1]`. Crew take damage below `x < 0.05`.
    pub oxygen: Vec<f32>,
    nav_mesh: NavMesh,
    path_graph: PathGraph,
}

impl ShipState {
    pub fn new() -> Self {
        let ship_type = 0;
        let (nav_lines, nav_squares) = SHIPS[ship_type].nav_mesh;
        let paths = SHIPS[ship_type].path_graph;
        Self {
            ship_type,
            reactor: Reactor::new(0),
            systems: default(),
            max_hull: 30,
            damage: 0,
            crew: default(),
            missiles: 10,
            oxygen: vec![1.0; SHIPS[ship_type].rooms.len()],
            nav_mesh: NavMesh {
                lines: nav_lines.into(),
                squares: nav_squares.into(),
            },
            path_graph: PathGraph {
                edges: paths
                    .iter()
                    .map(|&(key, values)| (key, values.iter().copied().collect()))
                    .collect(),
            },
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
                .map(|weapons| weapons.weapons().iter().map(|x| x.target()).collect())
                .unwrap_or_default(),
            crew: self.crew.clone(),
            autofire: self
                .systems
                .weapons
                .as_ref()
                .map(|weapons| weapons.autofire)
                .unwrap_or(false),
            oxygen: self.oxygen.iter().copied().average().unwrap(),
        }
    }

    pub fn basic_intel(&self) -> BasicIntel {
        BasicIntel {
            ship_type: self.ship_type,
            max_hull: self.max_hull,
            hull: self.max_hull - self.damage,
            system_locations: SHIPS[self.ship_type]
                .room_systems
                .iter()
                .enumerate()
                .filter_map(|(room, system)| system.map(|system| (system, room)))
                .collect(),
            shields: self.systems.shields.as_ref().map(|shields| ShieldIntel {
                max_layers: shields.max_layers(),
                layers: shields.layers,
                charge: shields.charge,
                damage: shields.damage_intel(),
            }),
            engines: self
                .systems
                .engines
                .as_ref()
                .map(|engines| engines.damage_intel()),
            weapons: self.systems.weapons.as_ref().map(|weapons| WeaponsIntel {
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
            oxygen: self
                .systems
                .oxygen
                .as_ref()
                .map(|oxygen| oxygen.damage_intel()),
        }
    }

    pub fn crew_vision_intel(&self) -> CrewVisionIntel {
        CrewVisionIntel
    }

    pub fn interior_intel(&self) -> InteriorIntel {
        InteriorIntel {
            rooms: SHIPS[self.ship_type]
                .rooms
                .iter()
                .enumerate()
                .map(|(i, room)| RoomIntel {
                    crew: self
                        .crew
                        .iter()
                        .filter(|x| x.is_in_room(room))
                        .map(|x| x.intel())
                        .collect(),
                    oxygen: self.oxygen[i],
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
                .map(|weapons| weapons.weapons().iter().map(|x| x.charge).collect())
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
        self.systems.weapons.as_mut().map(|weapons| {
            let missiles = &mut self.missiles;
            let autofire = weapons.autofire;
            weapons
                .weapons_mut()
                .filter_map(move |x| x.charge_and_fire(missiles, autofire))
        })
    }

    pub fn update_repair_status(&mut self) {
        for (i, room) in SHIPS[self.ship_type].rooms.iter().enumerate() {
            if let Some(system) = SHIPS[self.ship_type].room_systems[i] {
                if !self.crew.iter().any(|x| x.is_in_room(room)) {
                    let system = self.systems.system_mut(system).unwrap();
                    system.cancel_repair();
                }
            }
        }
    }

    pub fn update_crew(&mut self) {
        for crew in &mut self.crew {
            let cell = crew.nav_status.current_cell();
            let room = SHIPS[self.ship_type]
                .rooms
                .iter()
                .position(|x| x.cells.iter().any(|x| *x == cell))
                .unwrap();
            if self.oxygen[room] < 0.05 {
                let rate = -6.4;
                let dt = 1.0 / 64.0;
                crew.health += rate * dt;
            }
        }
        self.crew.retain(|x| x.health > 0.0);
        for crew in &mut self.crew {
            crew.nav_status.step(&self.nav_mesh);
            if let &CrewNavStatus::At(cell) = &crew.nav_status {
                let room = SHIPS[self.ship_type]
                    .rooms
                    .iter()
                    .position(|x| x.cells.iter().any(|x| *x == cell))
                    .unwrap();
                // if enemy_crew_in_room {
                //     KILL HIM
                // } else if fire_in_room {
                //     stop drop and roll
                // } else if hull_breach_in_room {
                //     fix it
                // } else
                if let Some(system) = SHIPS[self.ship_type].room_systems[room] {
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

    pub fn update_oxygen(&mut self) {
        let fill_rate = match self
            .systems
            .oxygen
            .as_ref()
            .map_or(0, |x| x.current_power())
        {
            1 => 0.012,
            2 => 0.048,
            3 => 0.084,
            _ => -0.012,
        };
        // for door in doors {
        //     let diff: f32 = door.b.o2 - door.a.o2;
        //     fill_rate[door.a] += diff;
        //     fill_rate[door.b] -= diff;
        // }
        let dt = 1.0 / 64.0;
        for room_oxygen in &mut self.oxygen {
            *room_oxygen = (*room_oxygen + fill_rate * dt).clamp(0.0, 1.0);
        }
        // let rooms = zip(SHIPS[self.ship_type].rooms, &self.oxygen);
        // let room_o2 = rooms.map(|(room, o2)| room.cells.len() as f32 * o2);
        // let total_o2 = room_o2.clone().fold(0.0, |x, y| x + y);
        // let o2_per_cell = total_o2 / SHIPS[self.ship_type].cell_positions.len() as f32;
    }

    pub fn install_system(&mut self, system: SystemId) {
        if self.systems.system(system).is_some() {
            eprintln!("Can't install {system} on ship, system is already installed.");
            return;
        }
        self.systems.install(system);
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
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't power weapon, weapons system not installed.");
            return;
        };
        weapons.power_weapon(index, self.missiles, &mut self.reactor);
    }

    pub fn depower_weapon(&mut self, index: usize) {
        let Some(weapons) = &mut self.systems.weapons else {
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
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't set weapon target, weapons system notinstalled.");
            return;
        };
        weapons.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }

    pub fn move_weapon(&mut self, weapon_index: usize, target_index: usize) {
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't move weapon, weapons system not installed.");
            return;
        };
        weapons.move_weapon(weapon_index, target_index);
    }

    pub fn set_crew_goal(&mut self, crew_index: usize, room_index: usize) {
        let Some(room) = SHIPS[self.ship_type].rooms.get(room_index) else {
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
        let occupied_room = SHIPS[self.ship_type]
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
                .section_with_cells(*cell, path.next_waypoint().unwrap())
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
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't set autofire, weapons system not installed.");
            return;
        };
        weapons.autofire = autofire;
    }
}
