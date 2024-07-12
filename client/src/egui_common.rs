use bevy::prelude::*;
use bevy_egui::{egui, egui::Ui, EguiContexts};
use bevy_replicon::prelude::*;
use common::{
    compute_dodge_chance,
    events::{AdjustPower, MoveWeapon, PowerDir, WeaponPower},
    intel::{SelfIntel, ShipIntel, SystemsIntel},
    Dead, PlayerReady, ReadyState, SystemId, Weapon,
};

pub fn status_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel, Without<Dead>>,
    systems: Query<&SystemsIntel>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        // No connection to server
        return;
    };
    let Ok(intel) = ships.get(self_intel.ship) else {
        // Ship destroyed
        return;
    };
    let systems = systems.get(intel.systems).unwrap();
    egui::Window::new("Ship Status").show(ui.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Hull Integrity");
            let max = intel.basic.max_hull;
            let current = intel.basic.hull;
            ui.add(egui::ProgressBar::new(current as f32 / max as f32).desired_width(400.0));
            ui.label(format!("{current}/{max}"));
        });
        if let Some(engines) = systems.get(&SystemId::Engines) {
            let dodge_chance = compute_dodge_chance(engines.current_power);
            ui.label(format!("Dodge Chance: {dodge_chance}%"));
        }
        ui.label(format!("Missiles: {}", self_intel.missiles));
    });
}

pub fn power_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel, Without<Dead>>,
    systems: Query<&SystemsIntel>,
    mut adjust_power: EventWriter<AdjustPower>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        // No connection to server
        return;
    };
    let Ok(intel) = ships.get(self_intel.ship) else {
        // Ship destroyed
        return;
    };
    let systems = systems.get(intel.systems).unwrap();
    egui::Window::new("Power").show(ui.ctx_mut(), |ui| {
        ui.label("Reactor");
        #[allow(unused_must_use)]
        ui.horizontal(|ui| {
            for _ in 0..self_intel.free_power {
                ui.selectable_label(true, "O");
            }
            for _ in self_intel.free_power..self_intel.max_power {
                ui.selectable_label(false, "O");
            }
        });

        if let Some(shields) = systems.get(&SystemId::Shields) {
            ui.label("Shields");
            if let Some(request) = power_bar(
                ui,
                shields.current_power,
                shields.upgrade_level,
                shields.damage,
                SystemId::Shields,
            ) {
                adjust_power.send(request);
            }
        }
        if let Some(engines) = systems.get(&SystemId::Engines) {
            ui.label("Engines");
            if let Some(request) = power_bar(
                ui,
                engines.current_power,
                engines.upgrade_level,
                engines.damage,
                SystemId::Engines,
            ) {
                adjust_power.send(request);
            }
        }
        if let Some(weapons) = systems.get(&SystemId::Weapons) {
            ui.label("Weapons");
            if let Some(request) = power_bar(
                ui,
                weapons.current_power,
                weapons.upgrade_level,
                weapons.damage,
                SystemId::Weapons,
            ) {
                adjust_power.send(request);
            }
        }
    });
}

#[allow(unused_must_use)]
fn power_bar(
    ui: &mut Ui,
    current: usize,
    max: usize,
    damage: usize,
    system: SystemId,
) -> Option<AdjustPower> {
    let hotkey = match system {
        SystemId::Shields => 'A',
        SystemId::Weapons => 'W',
        SystemId::Engines => 'S',
    };
    let mut result = None;
    ui.horizontal(|ui| {
        if ui
            .button("-")
            .on_hover_text(format!("Remove power (Hotkey: Shift+{hotkey})"))
            .clicked()
        {
            result = Some(AdjustPower::remove(system));
        }
        if ui
            .button("+")
            .on_hover_text(format!("Add power (Hotkey: {hotkey})"))
            .clicked()
        {
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

pub fn shields_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel, Without<Dead>>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        // No connection to server
        return;
    };
    let Ok(intel) = ships.get(self_intel.ship) else {
        // Ship destroyed
        return;
    };
    let Some(shields) = &intel.basic.shields else {
        // No shields installed
        return;
    };
    egui::Window::new("Shields").show(ui.ctx_mut(), |ui| {
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
    });
}

pub fn ready_panel(
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

pub fn weapon_rearrange_ui(
    ui: &mut Ui,
    index: usize,
    last_weapon: usize,
    weapon_ordering: &mut EventWriter<MoveWeapon>,
) {
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
}

pub fn weapon_power_ui(
    ui: &mut Ui,
    powered: bool,
    index: usize,
    weapon: &Weapon,
    weapon_power: &mut EventWriter<WeaponPower>,
) {
    let mut new_powered = powered;
    for _ in 0..weapon.power {
        ui.checkbox(&mut new_powered, "")
            .on_hover_text(format!("Toggle weapon power (Hotkey: {})", index + 1));
    }
    if new_powered != powered {
        let dir = if new_powered {
            PowerDir::Request
        } else {
            PowerDir::Remove
        };
        weapon_power.send(WeaponPower {
            dir,
            weapon_index: index,
        });
    }
}

pub fn weapon_charge_ui(ui: &mut Ui, charge: f32, weapon: &Weapon) {
    let charge = charge / weapon.charge_time;
    ui.add(egui::ProgressBar::new(charge).desired_width(100.0));
}
