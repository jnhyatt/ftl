use super::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use common::{events::*, *};

pub fn adjust_power(
    mut events: EventReader<FromClient<AdjustPower>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut Ship, Without<Dead>>,
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
    mut ships: Query<&mut Ship, Without<Dead>>,
) {
    for &FromClient {
        client_id,
        event: WeaponPower { dir, index },
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
    mut ships: Query<(&mut Ship, &ShipIntel), Without<Dead>>,
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
        let Ok((mut ship, intel)) = ships.get_mut(client_ship) else {
            eprintln!("Entity {client_ship:?} is not a ship.");
            continue;
        };
        let targeting_self = target.map(|x| x.ship_intel) == Some(intel.0);
        ship.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }
}

pub fn set_autofire(
    mut events: EventReader<FromClient<SetAutofire>>,
    client_ships: Res<ClientShips>,
    mut ships: Query<&mut Ship, Without<Dead>>,
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
