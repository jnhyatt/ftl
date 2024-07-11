use bevy::prelude::*;
use bevy_replicon::{
    prelude::{has_authority, ChannelKind},
    server::events::{SendMode, ServerEventAppExt, ToClients},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait ReplicateResExt {
    fn replicate_resource<R: Resource + Serialize + DeserializeOwned + Clone>(&mut self);
}

impl ReplicateResExt for App {
    fn replicate_resource<R: Resource + Serialize + DeserializeOwned + Clone>(&mut self) {
        self.add_server_event::<UpdateResource<R>>(ChannelKind::Ordered);
        self.add_systems(
            PostUpdate,
            (
                send_changed_resource::<R>.run_if(resource_exists::<R>),
                send_removed_resource::<R>.run_if(resource_removed::<R>()),
            )
                .run_if(has_authority),
        );
        self.add_systems(
            PreUpdate,
            replicate_resource::<R>.run_if(not(has_authority)),
        );
    }
}

#[derive(Event, Serialize, Deserialize)]
struct UpdateResource<R: Resource + Serialize>(Option<R>);

fn send_changed_resource<R: Resource + Serialize + Clone>(
    r: Res<R>,
    mut updates: EventWriter<ToClients<UpdateResource<R>>>,
) {
    if r.is_changed() {
        updates.send(ToClients {
            mode: SendMode::Broadcast,
            event: UpdateResource(Some(r.clone())),
        });
    }
}

fn send_removed_resource<R: Resource + Serialize>(
    mut updates: EventWriter<ToClients<UpdateResource<R>>>,
) {
    updates.send(ToClients {
        mode: SendMode::Broadcast,
        event: UpdateResource(None),
    });
}

fn replicate_resource<R: Resource + Serialize + Clone>(
    mut updates: EventReader<UpdateResource<R>>,
    mut commands: Commands,
) {
    for update in updates.read() {
        if let Some(update) = &update.0 {
            commands.insert_resource(update.clone());
        } else {
            commands.remove_resource::<R>();
        }
    }
}
