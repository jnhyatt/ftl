use std::{
    collections::{HashSet, VecDeque},
    iter::zip,
};

use bevy::prelude::*;
use common::{
    bullets::{BeamTarget, RoomTarget},
    intel::{
        BasicIntel, InteriorIntel, RoomIntel, SelfIntel, ShieldIntel, SystemsIntel,
        WeaponChargeIntel, WeaponIntel, WeaponsIntel,
    },
    nav::{Cell, CrewNav, CrewNavStatus, NavMesh, PathGraph},
    ship::{Door, SystemId, SHIPS},
    util::IterAvg,
    Crew, DoorState,
};
use strum::IntoEnumIterator;

use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, ShipSystems},
    weapons::Volley,
};

#[derive(Component, Debug)]
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
    pub doors: Vec<DoorState>,
    nav_mesh: NavMesh,
    path_graph: PathGraph,
}

impl ShipState {
    pub fn new() -> Self {
        let ship_type = 0;
        let (nav_lines, nav_squares) = SHIPS[ship_type].nav_mesh;
        Self {
            ship_type,
            reactor: Reactor::new(),
            systems: default(),
            max_hull: 30,
            damage: 0,
            crew: default(),
            missiles: 10,
            oxygen: vec![1.0; SHIPS[ship_type].rooms.len()],
            doors: SHIPS[ship_type]
                .doors
                .iter()
                .map(|_| DoorState::default())
                .collect(),
            nav_mesh: NavMesh {
                lines: nav_lines.into(),
                squares: nav_squares.into(),
            },
            path_graph: PathGraph {
                edges: SHIPS[ship_type]
                    .path_graph
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
                        weapon: x.weapon(),
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
            doors: self.doors.clone(),
        }
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
                .map(|weapons| weapons.weapons().iter().map(|x| x.charge()).collect())
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

    pub fn update_weapons(&mut self) -> Option<impl Iterator<Item = Option<Volley>> + '_> {
        self.systems.weapons.as_mut().map(|weapons| {
            let missiles = &mut self.missiles;
            let autofire = weapons.autofire;
            weapons
                .weapons_mut()
                .map(move |x| x.charge_and_fire(missiles, autofire))
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
        let ship = &SHIPS[self.ship_type];
        let room_count = ship.rooms.len();
        let mut fill_rate = vec![fill_rate; room_count];
        for door in ship
            .doors
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.doors[i].is_open())
            .map(|(_, x)| *x)
        {
            match door {
                Door::Interior(a, b) => {
                    let a_room = ship.cell_room(a);
                    let b_room = ship.cell_room(b);
                    let diff = self.oxygen[b_room] - self.oxygen[a_room];
                    fill_rate[a_room] += diff;
                    fill_rate[b_room] -= diff;
                }
                Door::Exterior(cell, _) => {
                    let room = ship.cell_room(cell);
                    fill_rate[room] -= self.oxygen[room];
                }
            }
        }
        let dt = 1.0 / 64.0;
        for (room_oxygen, fill_rate) in zip(&mut self.oxygen, fill_rate) {
            *room_oxygen = (*room_oxygen + fill_rate * dt).clamp(0.0, 1.0);
        }
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

    pub fn set_beam_weapon_target(&mut self, weapon_index: usize, target: Option<BeamTarget>) {
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't set weapon target, weapons system notinstalled.");
            return;
        };
        weapons.set_beam_weapon_target(weapon_index, target);
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

        match Self::path_crew_to(&self.path_graph, &self.nav_mesh, crew, target_cell) {
            Ok(()) => {}
            Err(()) => {
                eprintln!("Can't set crew {crew_index} goal, room {room_index} is unreachable.");
            }
        }
    }

    #[must_use]
    fn path_crew_to(
        path_graph: &PathGraph,
        nav_mesh: &NavMesh,
        crew: &mut CrewNavStatus,
        target_cell: Cell,
    ) -> Result<(), ()> {
        let pathing = path_graph.pathing_to(target_cell);
        let Some(path) = nav_mesh.find_path(&pathing, crew.current_location()) else {
            // Path unreachable by crew
            return Err(());
        };
        let current_location = match crew {
            CrewNavStatus::At(cell) => nav_mesh
                .section_with_cells(*cell, path.next_waypoint())
                .unwrap()
                .to_location(*cell),
            CrewNavStatus::Navigating(nav) => nav.current_location,
        };
        *crew = CrewNavStatus::Navigating(CrewNav {
            path,
            current_location,
        });
        Ok(())
    }

    pub fn set_autofire(&mut self, autofire: bool) {
        let Some(weapons) = &mut self.systems.weapons else {
            eprintln!("Can't set autofire, weapons system not installed.");
            return;
        };
        weapons.autofire = autofire;
    }

    pub fn save_crew_stations(&mut self) {
        for crew in &mut self.crew {
            crew.station = Some(crew.nav_status.occupied_cell());
        }
    }

    pub fn crew_return_to_stations(&mut self) {
        for (i, crew) in self.crew.iter_mut().enumerate() {
            if let Some(target_cell) = crew.station {
                match Self::path_crew_to(
                    &self.path_graph,
                    &self.nav_mesh,
                    &mut crew.nav_status,
                    target_cell,
                ) {
                    Ok(()) => {}
                    Err(()) => {
                        eprintln!("Can't set crew {i} goal, cell {target_cell:?} is unreachable.");
                    }
                }
            }
        }

        // At this point we're in a potentially invalid state. If there are crew that don't have
        // saved stations *and* if those crew are standing in other crew's stations, we could
        // potentially have multiple crew "occupying" the same cell. To correct this, we find crew
        // without stations that share a cell with any other crew and find them a new spot.
        for i in 0..self.crew.len() {
            let occupied_cell = self.crew[i].nav_status.occupied_cell();
            if self.crew[i].station.is_none() {
                let mut not_me = self.crew.iter().enumerate().filter(|(x, _)| *x != i);
                // If any crew (that aren't me) share my cell, find me a new cell
                if not_me.any(|(_, x)| x.nav_status.occupied_cell() == occupied_cell) {
                    let new_cell = self.room_or_nearby(
                        SHIPS[self.ship_type].cell_room(occupied_cell),
                        i,
                        true,
                    );
                    Self::path_crew_to(
                        &self.path_graph,
                        &self.nav_mesh,
                        &mut self.crew[i].nav_status,
                        new_cell,
                    )
                    .expect("`room_or_nearby` should only return reachable cells!");
                }
            }
        }
    }

    /// Return a cell within the specified room. If the room is full, a cell within the nearest room
    /// that does have space. Crew must be specified so the algorithm doesn't find self-
    /// obstructions. Finally, caller must specify whether to only consider cells reachable from the
    /// given room. For moving out of the way of other crew, this should be true. For teleporting
    /// onto a ship, the can be false. This may strand crew in unconnected "islands" of cells.
    fn room_or_nearby(&self, room: usize, crew: usize, reachable_only: bool) -> Cell {
        // Breadth-first search for a room with space, beginning with the specified room
        let mut frontier = VecDeque::new();
        frontier.push_back(room);
        let mut visited = HashSet::new();
        visited.insert(room);

        while let Some(current) = frontier.pop_front() {
            for &cell in SHIPS[self.ship_type].rooms[current].cells {
                if reachable_only {
                    let pathing = self.path_graph.pathing_to(cell);
                    let reachable = self
                        .nav_mesh
                        .find_path(&pathing, self.crew[crew].nav_status.current_location())
                        .is_some();
                    if !reachable {
                        continue;
                    }
                }
                let cell_clear_besides_me = self
                    .crew
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != crew)
                    .all(|(_, x)| x.nav_status.occupied_cell() != cell);
                if cell_clear_besides_me {
                    return cell;
                }
            }
            let next_cells = SHIPS[self.ship_type]
                .neighbors_of_room(current)
                .filter(|x| !visited.contains(&x))
                .collect::<Vec<_>>();
            for next in next_cells {
                frontier.push_back(next);
                visited.insert(next);
            }
        }
        panic!("Ship is overstuffed!");
    }
}
