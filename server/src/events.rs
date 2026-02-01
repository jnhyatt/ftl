use bevy::prelude::*;
use bevy_replicon::prelude::*;
use common::{
    events::{
        AdjustPower, CrewStations, MoveWeapon, PowerDir, SetAutofire, SetBeamWeaponTarget,
        SetCrewGoal, SetDoorsOpen, SetProjectileWeaponTarget, WeaponPower,
    },
    ship::{Dead, Door, SHIPS},
};

use crate::ship::ShipState;

pub fn adjust_power(
    mut events: MessageReader<FromClient<AdjustPower>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: AdjustPower { dir, system },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        match dir {
            PowerDir::Request => ship.request_power(system),
            PowerDir::Remove => ship.remove_power(system),
        }
    }
    Ok(())
}

pub fn weapon_power(
    mut events: MessageReader<FromClient<WeaponPower>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: WeaponPower {
            dir,
            weapon_index: index,
        },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        match dir {
            PowerDir::Request => ship.power_weapon(index),
            PowerDir::Remove => ship.depower_weapon(index),
        }
    }
    Ok(())
}

pub fn set_projectile_weapon_target(
    mut events: MessageReader<FromClient<SetProjectileWeaponTarget>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: SetProjectileWeaponTarget {
            weapon_index,
            target,
        },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        let targeting_self = target.map(|x| x.ship == client_id).unwrap_or_default();
        ship.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }
    Ok(())
}

pub fn set_beam_weapon_target(
    mut events: MessageReader<FromClient<SetBeamWeaponTarget>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: SetBeamWeaponTarget {
            weapon_index,
            target,
        },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        if let Some(target) = target {
            if target.ship == client_id {
                eprintln!("Beams cannot target own ship.");
                continue;
            }
        }
        ship.set_beam_weapon_target(weapon_index, target);
    }
    Ok(())
}

pub fn move_weapon(
    mut events: MessageReader<FromClient<MoveWeapon>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: MoveWeapon {
            weapon_index,
            target_index,
        },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        ship.move_weapon(weapon_index, target_index);
    }
    Ok(())
}

pub fn set_crew_goal(
    mut events: MessageReader<FromClient<SetCrewGoal>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: SetCrewGoal {
            crew,
            room: target_room,
        },
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        ship.set_crew_goal(crew, target_room);
    }
    Ok(())
}

pub fn set_autofire(
    mut events: MessageReader<FromClient<SetAutofire>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient {
        client_id,
        message: SetAutofire(autofire),
    } in events.read()
    {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        ship.set_autofire(autofire);
    }
    Ok(())
}

pub fn set_doors_open(
    mut events: MessageReader<FromClient<SetDoorsOpen>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient { client_id, message } in events.read() {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        // TODO Forward this to `ShipState`
        match message {
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
    Ok(())
}

pub fn crew_stations(
    mut events: MessageReader<FromClient<CrewStations>>,
    mut ships: Query<&mut ShipState, Without<Dead>>,
) -> Result {
    for &FromClient { client_id, message } in events.read() {
        let client_id = client_id.entity().ok_or("Client ID must be remote!")?;
        let mut ship = ships.get_mut(client_id)?;
        match message {
            CrewStations::Save => ship.save_crew_stations(),
            CrewStations::Return => ship.crew_return_to_stations(),
        }
    }
    Ok(())
}
