use bevy::prelude::*;

use crate::{display_error, Client, GameState};

#[derive(Component)]
struct Lobby;

fn spawn(mut commands: Commands, asset_server: Res<AssetServer>) {
    let _lobby = build(&mut commands, &asset_server);
}

fn despawn(mut commands: Commands, lobby: Query<Entity, With<Lobby>>) {
    if let Ok(lobby) = lobby.get_single() {
        commands.entity(lobby).despawn_recursive();
    }
}

const _BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const BORDER_COLOR_INACTIVE: Color = Color::srgb(0.25, 0.25, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

fn build(commands: &mut Commands, asset_server: &Res<AssetServer>) -> Entity {
    let text_style = TextStyle {
        font_size: 40.,
        color: TEXT_COLOR,
        ..default()
    };

    let node_bundle = NodeBundle {
        style: Style {
            width: Val::Px(600.0),
            border: UiRect::all(Val::Px(5.0)),
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        border_color: BORDER_COLOR_INACTIVE.into(),
        background_color: BACKGROUND_COLOR.into(),
        ..default()
    };

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            },
            Lobby,
        ))
        .with_children(|parent| {
            parent.spawn(node_bundle).with_children(|parent| {
                parent.spawn(TextBundle::from_section(
                    "Waiting for the host to start the game...",
                    text_style,
                ));
            });
        })
        .id()
}

fn lobby_system(mut commands: Commands, mut client: ResMut<Client>, mut next_state: ResMut<NextState<GameState>>) {
    if client.0.game_started() {
        match client.0.run() {
            Ok(_) => next_state.set(GameState::InGame), 
            Err(e) => display_error(&mut commands, &mut next_state, &e.to_string())
        }
    }
}
pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InLobby), spawn)
            .add_systems(OnExit(GameState::InLobby), despawn)
            .add_systems(Update, lobby_system.run_if(in_state(GameState::InLobby)));
    }
}
