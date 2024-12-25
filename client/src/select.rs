use bevy::{
    color::palettes::basic::*,
    ecs::system::QueryLens,
    math::bounding::{Aabb2d, BoundingCircle, IntersectsVolume},
    prelude::*,
};

pub fn selection_plugin(app: &mut App) {
    app.add_event::<SelectEvent>();
    app.add_event::<DeselectAll>();
    app.init_resource::<SelectionEnabled>();
    app.add_systems(
        Update,
        (
            highlight_selected,
            handle_select_event,
            draw_selection.run_if(resource_exists::<Selection>),
            deselect_all.run_if(resource_removed::<SelectionEnabled>.or(on_event::<DeselectAll>)),
        )
            .chain(),
    );
}

#[derive(Event, Clone, Copy, Debug)]
pub enum SelectEvent {
    GrowTo(Vec2),
    Complete,
}

/// Marks an entity as selectable. Selectable entities have a bounding circle in the XY plane with
/// radius defined by this component
#[derive(Component, Clone, Copy, Debug)]
pub struct Selectable {
    pub radius: f32,
}

/// Tags a currently-selected entity
#[derive(Component, Clone, Copy, Debug)]
pub struct Selected;

/// The current selection box. This is updated by the plugin based on pointer motion.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Selection {
    pub start: Vec2,
    pub end: Vec2,
}

pub fn draw_selection(selection: Res<Selection>, mut gizmos: Gizmos) {
    let &Selection { start, end } = selection.as_ref();
    gizmos.rect_2d((start + end) / 2.0, end - start, LIME);
}

pub fn highlight_selected(
    selected: Query<(&GlobalTransform, &Selectable), With<Selected>>,
    mut gizmos: Gizmos,
) {
    for (transform, &Selectable { radius }) in &selected {
        gizmos.circle_2d(transform.translation().xy(), radius, LIME);
    }
}

pub fn pick_entities(
    selection: Aabb2d,
    mut select_targets: QueryLens<(Entity, &GlobalTransform, &Selectable)>,
) -> Vec<Entity> {
    select_targets
        .query()
        .iter()
        .filter(|(_, transform, &Selectable { radius })| {
            let bounds = BoundingCircle {
                center: transform.translation().xy(),
                circle: Circle { radius },
            };
            selection.intersects(&bounds)
        })
        .map(|(e, _, _)| e)
        .collect()
}

#[derive(Event)]
pub struct DeselectAll;

pub fn deselect_all(world: &mut World) {
    let to_deselect = world
        .query_filtered::<Entity, With<Selected>>()
        .iter(world)
        .collect::<Vec<_>>();
    for e in to_deselect {
        world.entity_mut(e).remove::<Selected>();
    }
}

#[derive(Resource, Default)]
pub struct SelectionEnabled;

pub fn handle_select_event(
    mut events: EventReader<SelectEvent>,
    mut selectables: Query<(Entity, &GlobalTransform, &Selectable)>,
    selected: Query<Entity, With<Selected>>,
    mut selection: Option<ResMut<Selection>>,
    mut commands: Commands,
    selection_enabled: Option<Res<SelectionEnabled>>,
) {
    for ev in events.read() {
        if selection_enabled.is_none() {
            continue;
        }
        match ev {
            &SelectEvent::GrowTo(pos) => {
                if let Some(selection) = selection.as_mut() {
                    selection.end = pos;
                } else {
                    commands.insert_resource(Selection {
                        start: pos,
                        end: pos,
                    });
                }
            }
            SelectEvent::Complete => {
                // Deselect all entities first
                for e in &selected {
                    commands.entity(e).remove::<Selected>();
                }
                // Then remove our select box
                commands.remove_resource::<Selection>();
                // Select all units in the selection box
                if let Some(selection) = selection.as_ref() {
                    let Selection { start, end } = *selection.as_ref();
                    let selection = Aabb2d {
                        min: start.min(end),
                        max: start.max(end),
                    };

                    for e in pick_entities(selection, selectables.transmute_lens()) {
                        commands.entity(e).insert(Selected);
                    }
                }
            }
        }
    }
}
