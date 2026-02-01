use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait ReplicateResExt {
    fn replicate_resource<R: Resource + Serialize + DeserializeOwned + std::fmt::Debug + Clone>(
        &mut self,
    ) -> &mut Self;
}

impl ReplicateResExt for App {
    fn replicate_resource<R: Resource + Serialize + DeserializeOwned + std::fmt::Debug + Clone>(
        &mut self,
    ) -> &mut Self {
        self.add_server_message::<UpdateResource<R>>(Channel::Ordered)
            .add_systems(
                PostUpdate,
                (
                    send_changed_resource::<R>.run_if(resource_exists::<R>),
                    send_removed_resource::<R>.run_if(resource_removed::<R>),
                )
                    .run_if(in_state(ClientState::Disconnected)),
            )
            .add_systems(
                PreUpdate,
                sync_resource_from_server::<R>.run_if(not(in_state(ClientState::Disconnected))),
            );
        #[cfg(feature = "server")]
        self.add_observer(send_resource_on_connect::<R>);
        self
    }
}

#[derive(Message, Serialize, Deserialize, Debug)]
struct UpdateResource<R: Resource + Serialize>(Option<R>);

fn send_changed_resource<R: Resource + Serialize + Clone>(
    r: Res<R>,
    mut updates: MessageWriter<ToClients<UpdateResource<R>>>,
) {
    if r.is_changed() {
        updates.write(ToClients {
            mode: SendMode::Broadcast,
            message: UpdateResource(Some(r.clone())),
        });
    }
}

fn send_removed_resource<R: Resource + Serialize>(
    mut updates: MessageWriter<ToClients<UpdateResource<R>>>,
) {
    updates.write(ToClients {
        mode: SendMode::Broadcast,
        message: UpdateResource(None),
    });
}

fn sync_resource_from_server<R: Resource + Serialize + std::fmt::Debug + Clone>(
    mut updates: MessageReader<UpdateResource<R>>,
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

#[cfg(feature = "server")]
fn send_resource_on_connect<R: Resource + Serialize + Clone>(
    e: On<Add, AuthorizedClient>,
    r: Option<Res<R>>,
    mut updates: MessageWriter<ToClients<UpdateResource<R>>>,
) {
    if let Some(r) = &r {
        updates.write(ToClients {
            mode: SendMode::Direct(ClientId::Client(e.entity)),
            message: UpdateResource(Some((*r).clone())),
        });
    }
}
