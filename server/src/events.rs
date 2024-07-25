use bevy::prelude::*;
use bevy_replicon::prelude::*;
use common::{
    events::{
        AdjustPower, CrewStations, MoveWeapon, PowerDir, SetAutofire, SetBeamWeaponTarget,
        SetCrewGoal, SetDoorsOpen, SetProjectileWeaponTarget, WeaponPower,
    },
    ship::{Dead, Door, SHIPS},
};

use crate::{ship::ShipState, ClientShips};

pub fn adjust_power(
    mut events: EventReader<FromClient<AdjustPower>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: AdjustPower { dir, system },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        match dir {
            PowerDir::Request => ship.request_power(system),
            PowerDir::Remove => ship.remove_power(system),
        }
    }
}

pub fn weapon_power(
    mut events: EventReader<FromClient<WeaponPower>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: WeaponPower {
            dir,
            weapon_index: index,
        },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        match dir {
            PowerDir::Request => ship.power_weapon(index),
            PowerDir::Remove => ship.depower_weapon(index),
        }
    }
}

pub fn set_projectile_weapon_target(
    mut events: EventReader<FromClient<SetProjectileWeaponTarget>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: SetProjectileWeaponTarget {
            weapon_index,
            target,
        },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        let targeting_self = target.map(|x| x.ship == client_ship).unwrap_or_default();
        ship.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }
}

pub fn set_beam_weapon_target(
    mut events: EventReader<FromClient<SetBeamWeaponTarget>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: SetBeamWeaponTarget {
            weapon_index,
            target,
        },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        if let Some(target) = target {
            if target.ship == client_ship {
                eprintln!("Beams cannot target own ship.");
                continue;
            }
        }
        ship.set_beam_weapon_target(weapon_index, target);
    }
}

pub fn move_weapon(
    mut events: EventReader<FromClient<MoveWeapon>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: MoveWeapon {
            weapon_index,
            target_index,
        },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        ship.move_weapon(weapon_index, target_index);
    }
}

pub fn set_crew_goal(
    mut events: EventReader<FromClient<SetCrewGoal>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: SetCrewGoal {
            crew,
            room: target_room,
        },
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        ship.set_crew_goal(crew, target_room);
    }
}

pub fn set_autofire(
    mut events: EventReader<FromClient<SetAutofire>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: SetAutofire(autofire),
    } in events.read()
    {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        ship.set_autofire(autofire);
    }
}

pub fn set_doors_open(
    mut events: EventReader<FromClient<SetDoorsOpen>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient { client_id, event } in events.read() {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        match event {
            SetDoorsOpen::Single { door, open } => {
                ship.doors[door].open = open;
            }
            SetDoorsOpen::All { open } => {
                if open {
                    let interior_doors = SHIPS[ship.ship_type]
                        .doors
                        .iter()
                        .enumerate()
                        .filter(|(_, door)| matches!(door, Door::Interior(_, _)))
                        .map(|(x, _)| x);
                    if interior_doors.clone().all(|x| ship.doors[x].open) {
                        for door in &mut ship.doors {
                            door.open = true;
                        }
                    } else {
                        for door in interior_doors {
                            ship.doors[door].open = true;
                        }
                    }
                } else {
                    for door in &mut ship.doors {
                        door.open = false;
                    }
                }
            }
        }
    }
}

pub fn crew_stations(
    mut events: EventReader<FromClient<CrewStations>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) {
    for &FromClient { client_id, event } in events.read() {
        let Some(&client_ship) = client_ships.get(&client_id) else {
            eprintln!("No ship entry for client {client_id:?}.");
            continue;
        };
        let Ok(mut ship) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        match event {
            CrewStations::Save => {
                ship.save_crew_stations();
            }
            CrewStations::Return => {
                ship.crew_return_to_stations();
            }
        }
    }
}
