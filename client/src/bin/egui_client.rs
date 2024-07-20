use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use client::{
    client_plugin,
    egui_common::{
        enemy_panels, power_panel, ready_panel, shields_panel, status_panel, weapon_charge_ui,
        weapon_power_ui, weapon_rearrange_ui,
    },
};
use common::{
    events::{MoveWeapon, SetProjectileWeaponTarget, WeaponPower},
    intel::{SelfIntel, ShipIntel, WeaponChargeIntel},
    lobby::ReadyState,
    projectiles::{NeedsDodgeTest, RoomTarget, Traversal, WeaponDamage},
    ship::{Dead, SHIPS},
    util::round_to_usize,
    RACES,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, EguiPlugin, client_plugin))
        .add_systems(
            Update,
            (
                power_panel,
                status_panel,
                weapons_panel,
                shields_panel,
                bullet_panels,
                enemy_panels,
                dead_panel,
                crew_panel,
                ready_panel.run_if(resource_exists::<ReadyState>),
            ),
        )
        .run();
}

fn weapons_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    ships: Query<(Entity, &ShipIntel), Without<Dead>>,
    charge_intel: Query<&WeaponChargeIntel>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut weapon_targeting: EventWriter<SetProjectileWeaponTarget>,
    mut weapon_ordering: EventWriter<MoveWeapon>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        // No connection to server
        return;
    };
    let Ok((_, intel)) = ships.get(self_intel.ship) else {
        // Ship destroyed
        return;
    };
    let Some(weapons) = &intel.basic.weapons else {
        // No weapons system
        return;
    };
    let weapon_charges = charge_intel.get(intel.weapon_charge).unwrap();
    egui::Window::new(format!("Weapons")).show(ui.ctx_mut(), |ui| {
        let last_weapon = weapons.weapons.len() - 1;
        for (index, weapon) in weapons.weapons.iter().enumerate() {
            ui.horizontal(|ui| {
                weapon_rearrange_ui(ui, index, last_weapon, &mut weapon_ordering);
                weapon_power_ui(ui, weapon.powered, index, &weapon.weapon, &mut weapon_power);
                ui.label(weapon.weapon.name);
                weapon_charge_ui(ui, weapon_charges.levels[index], &weapon.weapon);

                ui.add_enabled_ui(weapon.powered, |ui| {
                    let mut new_target: Option<Option<RoomTarget>> = None;
                    let current_target = self_intel.weapon_targets[index];
                    egui::ComboBox::new(index, "Target")
                        .selected_text(format!("{:?}", current_target))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut new_target, Some(None), "No target");
                            let targets = ships
                                .iter()
                                .filter(|(e, _)| {
                                    weapon.weapon.can_target_self || *e != self_intel.ship
                                })
                                .flat_map(|(ship, intel)| {
                                    (0..SHIPS[intel.basic.ship_type].rooms.len())
                                        .map(move |room| RoomTarget { ship, room })
                                });
                            for target in targets {
                                ui.selectable_value(
                                    &mut new_target,
                                    Some(Some(target)),
                                    format!("{:?}", target),
                                );
                            }
                        });
                    if let Some(new_target) = new_target {
                        weapon_targeting.send(SetProjectileWeaponTarget {
                            weapon_index: index,
                            target: new_target,
                        });
                    }
                });
            });
        }
    });
}

fn bullet_panels(
    mut ui: EguiContexts,
    bullets: Query<(Entity, &Traversal, Has<WeaponDamage>, Has<NeedsDodgeTest>)>,
) {
    for (bullet, &progress, has_damage, pending_dodge) in &bullets {
        egui::Window::new(format!("Bullet {bullet:?}")).show(ui.ctx_mut(), |ui| {
            ui.add(egui::ProgressBar::new(*progress));
            if !has_damage && !pending_dodge {
                ui.label("MISS");
            }
        });
    }
}

fn dead_panel(mut ui: EguiContexts, self_intel: Query<&SelfIntel>, ships: Query<Has<Dead>>) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    if ships.get(self_intel.ship).unwrap() {
        egui::Window::new("You Died").show(ui.ctx_mut(), |ui| {
            ui.label("Your ship has been destroyed");
        });
    }
}

fn crew_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    // ships: Query<&ShipIntel, Without<Dead>>,
    // mut set_crew_goal: EventWriter<SetCrewGoal>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    egui::Window::new("Crew").show(ui.ctx_mut(), |ui| {
        for (_crew_index, crew) in self_intel.crew.iter().enumerate() {
            ui.group(|ui| {
                // let cell = crew.nav_status.occupied_cell();
                // let current_room = ship
                //     .rooms
                //     .iter()
                //     .position(|x| x.cells.iter().any(|x| *x == cell))
                //     .unwrap();
                ui.heading(&crew.name);
                ui.label(format!(
                    "Health: {}/{}",
                    round_to_usize(crew.health),
                    round_to_usize(RACES[crew.race].max_health)
                ));
                // let mut target_room = current_room;
                // let room_name = |room| format!("Room {}", room + 1);
                // egui::ComboBox::new(&crew.name, "Goal")
                //     .selected_text(room_name(current_room))
                //     .show_ui(ui, |ui| {
                //         for room in 0..ship.rooms.len() {
                //             ui.selectable_value(&mut target_room, room, room_name(room));
                //         }
                //     });
                ui.label(format!("Current status: {:?}", crew.nav_status));
                // if target_room != current_room {
                //     set_crew_goal.send(SetCrewGoal {
                //         crew: crew_index,
                //         target_room,
                //     });
                // }
            });
        }
    });
}
