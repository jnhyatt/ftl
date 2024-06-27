use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts, EguiPlugin,
};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    renet::{
        transport::{ClientAuthentication, NetcodeClientTransport},
        ConnectionConfig, RenetClient,
    },
    RenetChannelsExt, RepliconRenetClientPlugin,
};
use common::{projectiles::*, *};
use events::*;
use intel::*;
use std::{
    net::{Ipv6Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EguiPlugin,
            RepliconPlugins.build().disable::<ServerPlugin>(),
            RepliconRenetClientPlugin,
            protocol_plugin,
        ))
        .add_systems(Startup, setup)
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

fn setup(channels: Res<RepliconChannels>, mut commands: Commands) {
    let server_channels_config = channels.get_server_configs();
    let client_channels_config = channels.get_client_configs();
    commands.insert_resource(RenetClient::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..default()
    }));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;
    let server_addr = SocketAddr::new(
        // Ipv6Addr::new(0x2a01, 0x4ff, 0x1f0, 0x9230, 0x0, 0x0, 0x0, 0x1).into(),
        Ipv6Addr::LOCALHOST.into(),
        5000,
    );
    let socket = UdpSocket::bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))).unwrap();
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
    commands.insert_resource(transport);
}

fn enemy_panels(
    mut ui: EguiContexts,
    ships: Query<&ShipIntel>,
    enemies: Query<(Entity, &Name, &IntelPackage, Has<Dead>)>,
    basic_intel: Query<&BasicIntel>,
) {
    for (e, name, intel, dead) in &enemies {
        // If e is in `ships`, it means we control it and we shouldn't show it as an
        // enemy
        if ships.iter().find(|x| x.0 == e).is_some() {
            continue;
        }
        let intel = basic_intel.get(intel.basic).unwrap();
        egui::Window::new(format!("Target {name:?}")).show(ui.ctx_mut(), |ui| {
            if dead {
                ui.label("DESTROYED");
            } else {
                ui.horizontal(|ui| {
                    ui.label("Hull Integrity");
                    let max = intel.max_hull;
                    let current = intel.hull;
                    ui.add(
                        egui::ProgressBar::new(current as f32 / max as f32).desired_width(400.0),
                    );
                    ui.label(format!("{current}/{max}"));
                });
                if let Some(shields) = &intel.shields {
                    ui.label("Shields");
                    ui.horizontal(|ui| {
                        for _ in 0..shields.layers {
                            let _ = ui.selectable_label(true, "O");
                        }
                        for _ in shields.layers..shields.max_layers {
                            let _ = ui.selectable_label(false, "O");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::ProgressBar::new(shields.charge));
                    });
                }
                if let Some(weapons) = &intel.weapons {
                    ui.label("Weapons");
                    for weapon in weapons {
                        ui.horizontal(|ui| {
                            let mut powered = weapon.powered;
                            ui.add_enabled_ui(false, |ui| {
                                ui.checkbox(&mut powered, "");
                            });
                            ui.label(weapon.weapon.name);
                        });
                    }
                }
            }
        });
    }
}

fn weapons_panel(
    mut ui: EguiContexts,
    ships: Query<(&Name, &Ship, &ShipIntel), Without<Dead>>,
    enemies: Query<(Entity, &Name, &IntelPackage)>,
    basic_intel: Query<&BasicIntel>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut weapon_targeting: EventWriter<SetProjectileWeaponTarget>,
    mut weapon_ordering: EventWriter<MoveWeapon>,
) {
    let room_name = |e: Option<ProjectileTarget>| {
        let Some(target) = e else {
            return String::from("No target");
        };
        let (_, name, intel) = enemies.get(target.ship_intel).unwrap();
        let intel = basic_intel.get(intel.basic).unwrap();
        format!("{name} {:?}", intel.rooms[target.room])
    };

    for (name, ship, me_intel) in &ships {
        let Some(weapons) = ship.systems.weapons() else {
            continue;
        };
        egui::Window::new(format!("Weapons ({name})")).show(ui.ctx_mut(), |ui| {
            let last_weapon = weapons.weapons().len() - 1;
            for (index, weapon) in weapons.weapons().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(index > 0, |ui| {
                        if ui.button("⬆").clicked() {
                            weapon_ordering.send(MoveWeapon {
                                weapon_index: index,
                                target_index: index - 1,
                            });
                        }
                    });
                    ui.add_enabled_ui(index < last_weapon, |ui| {
                        if ui.button("⬇").clicked() {
                            weapon_ordering.send(MoveWeapon {
                                weapon_index: index,
                                target_index: index + 1,
                            });
                        }
                    });
                    let mut powered = weapon.is_powered();
                    for _ in 0..weapon.weapon.power {
                        ui.checkbox(&mut powered, "");
                    }
                    if powered != weapon.is_powered() {
                        let dir = if powered {
                            PowerDir::Request
                        } else {
                            PowerDir::Remove
                        };
                        weapon_power.send(WeaponPower { dir, index });
                    }
                    ui.label(weapon.weapon.name);
                    let charge = weapon.charge / weapon.weapon.charge_time;

                    ui.add(egui::ProgressBar::new(charge).desired_width(100.0));
                    ui.add_enabled_ui(weapon.is_powered(), |ui| {
                        let mut new_target: Option<Option<ProjectileTarget>> = None;
                        let current_target = weapon.target();
                        egui::ComboBox::new(index, "Target")
                            .selected_text(room_name(current_target))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut new_target, Some(None), room_name(None));
                                for target in enemies
                                    .iter()
                                    .filter(|(e, _, _)| {
                                        weapon.weapon.can_target_self || *e != me_intel.0
                                    })
                                    .flat_map(|(ship_intel, _, intel)| {
                                        let intel = basic_intel.get(intel.basic).unwrap();
                                        (0..intel.rooms.len())
                                            .map(move |room| ProjectileTarget { ship_intel, room })
                                    })
                                {
                                    ui.selectable_value(
                                        &mut new_target,
                                        Some(Some(target)),
                                        room_name(Some(target)),
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
}

fn shields_panel(mut ui: EguiContexts, ships: Query<(&Name, &Ship), Without<Dead>>) {
    for (name, ship) in &ships {
        let Some(shields) = ship.systems.shields() else {
            continue;
        };
        egui::Window::new(format!("Shields ({name})")).show(ui.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                for _ in 0..shields.layers {
                    let _ = ui.selectable_label(true, "O");
                }
                for _ in shields.layers..shields.max_layers() {
                    let _ = ui.selectable_label(false, "O");
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::ProgressBar::new(shields.charge));
            });
        });
    }
}

#[allow(unused_must_use)]
fn power_bar(
    ui: &mut Ui,
    current: usize,
    max: usize,
    damage: usize,
    system: SystemId,
) -> Option<AdjustPower> {
    let mut result = None;
    ui.horizontal(|ui| {
        if ui.button("-").clicked() {
            result = Some(AdjustPower::remove(system));
        }
        if ui.button("+").clicked() {
            result = Some(AdjustPower::request(system));
        }
        for _ in 0..current {
            ui.selectable_label(true, "O");
        }
        let undamaged = max - damage;
        for _ in current..undamaged {
            ui.selectable_label(false, "O");
        }
        ui.add_enabled_ui(false, |ui| {
            for _ in 0..damage {
                ui.selectable_label(false, "X");
            }
        });
    });
    result
}

fn power_panel(
    mut ui: EguiContexts,
    ships: Query<(&Name, &Ship), Without<Dead>>,
    mut adjust_power: EventWriter<AdjustPower>,
) {
    for (name, ship) in &ships {
        egui::Window::new(format!("Power ({name})")).show(ui.ctx_mut(), |ui| {
            ui.label("Reactor");
            #[allow(unused_must_use)]
            ui.horizontal(|ui| {
                for _ in 0..ship.reactor.available {
                    ui.selectable_label(true, "O");
                }
                for _ in ship.reactor.available..ship.reactor.upgrade_level {
                    ui.selectable_label(false, "O");
                }
            });
            if let Some(shields) = ship.systems.shields() {
                ui.label("Shields");
                if let Some(request) = power_bar(
                    ui,
                    shields.current_power(),
                    shields.upgrade_level(),
                    shields.damage(),
                    SystemId::Shields,
                ) {
                    adjust_power.send(request);
                }
            }
            if let Some(engines) = ship.systems.engines() {
                ui.label("Engines");
                if let Some(request) = power_bar(
                    ui,
                    engines.current_power(),
                    engines.upgrade_level(),
                    engines.damage(),
                    SystemId::Engines,
                ) {
                    adjust_power.send(request);
                }
            }
            if let Some(weapons) = ship.systems.weapons() {
                ui.label("Weapons");
                if let Some(request) = power_bar(
                    ui,
                    weapons.current_power(),
                    weapons.upgrade_level(),
                    weapons.damage(),
                    SystemId::Weapons,
                ) {
                    adjust_power.send(request);
                }
            }
        });
    }
}

fn status_panel(mut ui: EguiContexts, ships: Query<(&Name, &Ship), Without<Dead>>) {
    for (name, ship) in &ships {
        egui::Window::new(format!("Ship Status ({name})")).show(ui.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Hull Integrity");
                let max = ship.max_hull;
                let current = ship.max_hull - ship.damage;
                ui.add(egui::ProgressBar::new(current as f32 / max as f32).desired_width(400.0));
                ui.label(format!("{current}/{max}"));
            });
            if let Some(engines) = ship.systems.engines() {
                let dodge_chance = engines.dodge_chance();
                ui.label(format!("Dodge Chance: {dodge_chance}%"));
            }
            if let Some(weapons) = ship.systems.weapons() {
                let missiles = weapons.missile_count();
                ui.label(format!("Missiles: {missiles}"));
            }
        });
    }
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

fn dead_panel(mut ui: EguiContexts, ships: Query<(), (With<Ship>, With<Dead>)>) {
    for _ in &ships {
        egui::Window::new("You Died").show(ui.ctx_mut(), |ui| {
            ui.label("Your ship has been destroyed");
        });
    }
}

fn crew_panel(
    mut ui: EguiContexts,
    ships: Query<(&Name, &Ship), Without<Dead>>,
    mut set_crew_goal: EventWriter<SetCrewGoal>,
) {
    for (name, ship) in &ships {
        egui::Window::new(format!("Crew ({name})")).show(ui.ctx_mut(), |ui| {
            for (crew_index, crew) in ship.crew.iter().enumerate() {
                ui.group(|ui| {
                    let cell = crew.nav_status.occupied_cell();
                    let current_room = ship
                        .rooms
                        .iter()
                        .position(|x| x.cells.iter().any(|x| *x == cell))
                        .unwrap();
                    ui.heading(&crew.name);
                    ui.label(format!(
                        "Health: {}/{}",
                        crew.health as usize, crew.max_health as usize
                    ));
                    let mut target_room = current_room;
                    let room_name = |room| format!("Room {}", room + 1);
                    egui::ComboBox::new(&crew.name, "Goal")
                        .selected_text(room_name(current_room))
                        .show_ui(ui, |ui| {
                            for room in 0..ship.rooms.len() {
                                ui.selectable_value(&mut target_room, room, room_name(room));
                            }
                        });
                    ui.label(format!("Current status: {:?}", crew.nav_status));
                    if target_room != current_room {
                        set_crew_goal.send(SetCrewGoal {
                            crew: crew_index,
                            target_room,
                        });
                    }
                });
            }
        });
    }
}

fn ready_panel(
    mut ui: EguiContexts,
    ready_state: Res<ReadyState>,
    mut client_ready: EventWriter<PlayerReady>,
    client: Res<RepliconClient>,
) {
    if let Some(client_id) = client.id() {
        egui::Window::new("Ready phase").show(ui.ctx_mut(), |ui| match ready_state.as_ref() {
            ReadyState::AwaitingClients { ready_clients } => {
                if ready_clients.contains(&client_id) {
                    ui.label("Waiting for players...");
                } else {
                    if ui.button("Ready").clicked() {
                        client_ready.send(default());
                    }
                }
            }
            ReadyState::Starting { countdown } => {
                ui.label(format!("Starting in {}", countdown.as_secs() + 1));
            }
        });
    }
}
