#![windows_subsystem = "windows"]

use bevy::{prelude::*, winit::WinitWindows};

mod ui;
use network::client::GameClient;
use packet_tools::game_packets::{GamePacket, PACKET_SIZE};
use render::{RenderSimulationPlugin, SimulationCamera};
use ui::{game::GamePlugin, lobby::LobbyPlugin, main_menu::MainMenuPlugin, over::WinScreenPlugin};
use winit::window::Icon;

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

fn set_window_icon(
    windows: NonSend<WinitWindows>,
) {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open("assets/textures/icon.png")
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();

    for window in windows.windows.values() {
        window.set_window_icon(Some(icon.clone()));
    }
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
        .add_systems(Startup, (setup, set_window_icon))
        .insert_state(GameState::Menu)
        .run();
}
