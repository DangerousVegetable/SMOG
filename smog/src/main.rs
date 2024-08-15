#![windows_subsystem = "windows"]

use bevy::prelude::*;

mod ui;
use network::client::GameClient;
use packet_tools::game_packets::{GamePacket, PACKET_SIZE};
use render::{RenderSimulationPlugin, SimulationCamera};
use ui::{game::GamePlugin, lobby::LobbyPlugin, main_menu::MainMenuPlugin, over::WinScreenPlugin};

mod network;
mod controller;

#[derive(Resource)]
struct Client(GameClient<GamePacket, PACKET_SIZE>);

#[derive(Resource)]
struct GameError(String);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum GameState {
    #[default]
    Menu,
    InLobby,
    InGame,
    EndGame,
    Error,
}

fn setup(mut commands: Commands) {
    // spawn camera
    commands
        .spawn(Camera2dBundle::default())
        .insert(SimulationCamera);
}

fn display_error(commands: &mut Commands, next_state: &mut ResMut<NextState<GameState>>, error: &str) {
    commands.insert_resource(GameError(error.to_string()));
    next_state.set(GameState::Error)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SMOG".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RenderSimulationPlugin)
        .add_plugins((MainMenuPlugin, LobbyPlugin, GamePlugin, WinScreenPlugin))
        .add_systems(Startup, setup)
        .insert_state(GameState::Menu)
        .run();
}
