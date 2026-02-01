use bevy::prelude::*;
use common::{
    intel::{SelfIntel, ShipIntel},
    nav::{Cell, CrewNavStatus, LineSection, NavLocation, SquareSection},
    ship::SHIPS,
};

use crate::{
    graphics::{CrewGraphic, Z_CREW},
    selection::Selectable,
};

/// Sync the number of crew graphics to match the number of crew members in the self ship.
/// [`CrewGraphic`] contains an index into the crew array in [`SelfIntel`]. As crew die, their
/// graphics are despawned in order from highest index to lowest, so the indices remain valid. Note
/// that this means that crew graphics may not correspond to the same crew members over time.
pub fn sync_crew_count(
    self_intel: Single<&SelfIntel>,
    crew: Query<(Entity, &ChildOf, &CrewGraphic)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let crew_graphics = crew
        .iter()
        .filter(|&(_, &ChildOf(parent), _)| parent == self_intel.ship)
        .collect::<Vec<_>>();
    let crew_count = self_intel.crew.len();
    let crew_graphic_count = crew_graphics.len();
    for i in crew_count..crew_graphic_count {
        let e = crew_graphics.iter().find(|(_, _, x)| x.0 == i).unwrap().0;
        commands.entity(e).despawn();
    }

    for x in crew_graphic_count..crew_count {
        let new_crew_member = commands
            .spawn((
                CrewGraphic(x),
                Name::new(format!("Crew {x}")),
                Selectable { radius: 10.0 },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                },
                Sprite {
                    image: assets.load("crew.png"),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, Z_CREW),
            ))
            .id();
        commands.entity(self_intel.ship).add_child(new_crew_member);
    }
}

/// Sync the positions of crew graphics to match the positions of crew members in the self ship.
pub fn sync_crew_positions(
    self_intel: Single<&SelfIntel>,
    ships: Query<&ShipIntel>,
    mut crew: Query<(&mut Transform, &ChildOf, &CrewGraphic)>,
) {
    let ship = &SHIPS[ships.get(self_intel.ship).unwrap().basic.ship_type];
    let mut crew_graphics = crew
        .iter_mut()
        .filter(|&(_, &ChildOf(parent), _)| parent == self_intel.ship)
        .collect::<Vec<_>>();
    crew_graphics.sort_unstable_by_key(|(_, _, x)| x.0);
    let crew = self_intel.crew.iter();
    let cell_pos = |&Cell(cell)| ship.cell_positions[cell];
    for (crew, (mut graphic, _, _)) in crew.zip(crew_graphics) {
        let crew_z = graphic.translation.z;
        let crew_xy = match &crew.nav_status {
            CrewNavStatus::At(x) => cell_pos(x),
            CrewNavStatus::Navigating(x) => match &x.current_location {
                NavLocation::Line(LineSection([a, b]), x) => cell_pos(a).lerp(cell_pos(b), *x),
                NavLocation::Square(SquareSection([[a, b], [c, d]]), x) => {
                    let bottom = cell_pos(a).lerp(cell_pos(b), x.y);
                    let top = cell_pos(c).lerp(cell_pos(d), x.y);
                    bottom.lerp(top, x.x)
                }
            },
        };
        graphic.translation = crew_xy.extend(crew_z);
    }
}
