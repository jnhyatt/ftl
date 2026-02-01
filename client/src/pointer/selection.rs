use bevy::prelude::*;

use crate::selection::SelectEvent;

pub fn start_selection(event: On<Pointer<Press>>, mut select_events: MessageWriter<SelectEvent>) {
    if event.button == PointerButton::Primary {
        let world_cursor = event.hit.position.unwrap().xy();
        select_events.write(SelectEvent::GrowTo(world_cursor));
    }
}

pub fn grow_selection(
    event: On<Pointer<Drag>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut select_events: MessageWriter<SelectEvent>,
) {
    let PointerButton::Primary = event.button else {
        return;
    };
    let (camera, camera_transform) = *camera;
    let Ok(world_cursor) =
        camera.viewport_to_world_2d(camera_transform, event.pointer_location.position)
    else {
        return;
    };
    select_events.write(SelectEvent::GrowTo(world_cursor));
}

pub fn finish_selection(
    event: On<Pointer<Release>>,
    mut select_events: MessageWriter<SelectEvent>,
) {
    let PointerButton::Primary = event.button else {
        return;
    };
    select_events.write(SelectEvent::Complete);
}
