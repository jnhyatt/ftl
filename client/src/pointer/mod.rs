//! This module is a place to group all pointer-related logic. It was getting messy and hard to
//! reason about. So here it is, all in one place: observers that handle pointer events as well as
//! code that makes pointer event handling context specific -- for example,
//! [`targeting::TargetingWeapon`] changes a *lot* of pointer functionality based on whether we're
//! currently targeting a weapon, including what the pointer buttons do and what entities are
//! clickable.
//!
//! I think the model we want to lean towards is to remove observers that aren't currently relevant.
//! It means more systems to handle adding/removing observers, but it means pointer-related
//! observers aren't constantly clobbering each other and running at the same time.

pub mod selection;
pub mod targeting;

use bevy::{
    picking::{
        hover::PreviousHoverMap,
        pointer::{PointerAction, PointerInput},
        PickingSystems,
    },
    prelude::*,
};
use common::{
    events::{SetCrewGoal, SetDoorsOpen},
    intel::ShipIntel,
    ship::Dead,
};

use crate::{
    graphics::{CrewGraphic, DoorGraphic, RoomGraphic},
    selection::Selected,
};

pub fn pointer_plugin(app: &mut App) {
    app.add_systems(PreUpdate, extra_pointer_events.in_set(ExtraPointerEvents));
    app.configure_sets(PreUpdate, ExtraPointerEvents.after(PickingSystems::Backend));
}

#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct ExtraPointerEvents;

pub fn toggle_door(
    event: On<Pointer<JustClick>>,
    ships: Query<&ShipIntel, Without<Dead>>,
    doors: Query<(&DoorGraphic, &ChildOf)>,
    mut set_doors_open: MessageWriter<SetDoorsOpen>,
) -> Result {
    let (&DoorGraphic(door), &ChildOf(ship)) = doors.get(event.entity)?;
    // TODO Should we be checking door state here? This makes it impossible to quickly double-toggle
    // doors on ping. Maybe instead we should send a door toggle message and let the server handle
    // the toggling logic? There are really trade-offs either way.
    let is_open = ships.get(ship)?.basic.doors[door].open;
    set_doors_open.write(SetDoorsOpen::Single {
        door,
        open: !is_open,
    });
    Ok(())
}

pub fn set_crew_goal(
    event: On<Pointer<Press>>,
    cells: Query<&RoomGraphic>,
    selected_crew: Query<&CrewGraphic, With<Selected>>,
    mut set_crew_goal: MessageWriter<SetCrewGoal>,
) -> Result {
    if event.button != PointerButton::Secondary {
        return Ok(());
    }
    let &RoomGraphic(room) = cells.get(event.entity)?;
    for &CrewGraphic(crew) in &selected_crew {
        set_crew_goal.write(SetCrewGoal { crew, room });
    }
    Ok(())
}

/// Just like [`Click`], but enforces it's *just* a click, not also a [`JustDragEnd`].
#[derive(Clone, Debug, Reflect)]
pub struct JustClick;

/// Just like [`DragEnd`], but enforces it's *just* a drag end, not also a [`JustClick`].
pub struct _JustDragEnd;

pub fn extra_pointer_events(
    mut input_events: MessageReader<PointerInput>,
    previous_hover_map: Res<PreviousHoverMap>,
    mut commands: Commands,
) {
    for PointerInput {
        pointer_id,
        location,
        action,
    } in input_events.read().cloned()
    {
        match action {
            PointerAction::Release(_) => {
                if let Some(hits) = previous_hover_map.get(&pointer_id) {
                    for (&hovered_entity, _) in hits.iter() {
                        let click_event =
                            Pointer::new(pointer_id, location.clone(), JustClick, hovered_entity);
                        commands.trigger(click_event);
                    }
                }
            }
            _ => {}
        }
    }
}
