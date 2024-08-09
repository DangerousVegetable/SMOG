use std::time::{Duration, Instant};

use bevy::{
    color::palettes::css::RED,
    diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
    math::{vec2, vec3, Vec3A, VectorSpace},
    prelude::*,
    render::{camera::ScalingMode, extract_component::ExtractComponent, primitives::Aabb},
    sprite::Anchor,
    tasks::futures_lite::future,
    window::PrimaryWindow,
};

use map_editor::map::MapLoader;
use solver::Solver;
use solver::{particle, PARTICLE_RADIUS};

use render::{RenderSimulationPlugin, RenderedSimulation, SimulationCamera};

mod network;
use network::{client::GameClient};

use packet_tools::{game_packets::{GamePacket, PACKET_SIZE}};

mod controller;
use controller::Controller;

const SUB_TICKS: usize = 8;

#[derive(Component)]
struct GameController(Controller);

fn setup_simulation(mut commands: Commands, client: Res<Client>, asset_server: Res<AssetServer>) {
    let map_loader = MapLoader::init_from_file(&client.0.map, "assets/maps", &asset_server);
    let solver = map_loader.map.solver();

    let simulation = RenderedSimulation(solver);
    let (bl, tr) = simulation.0.constraint.bounds();
    let projection = OrthographicProjection {
        scale: 1.0,
        scaling_mode: ScalingMode::FixedHorizontal(tr.x - bl.x),
        ..Default::default()
    };

    // Spawn the camera with the custom projection
    commands
        .spawn(Camera2dBundle {
            projection: projection.into(),
            ..Default::default()
        })
        .insert(SimulationCamera);

    commands
        .spawn(SpatialBundle {
            visibility: Visibility::Visible,
            transform: Transform::IDENTITY,
            ..default()
        })
        .insert(simulation)
        .insert(GameController(Controller::new(client.0.id)));

    // Spawn the counter
    let text_style = TextStyle {
        font: Default::default(),
        font_size: 60.,
        color: Color::Srgba(RED),
    };

    commands.spawn(Text2dBundle {
        text: Text::from_section("", text_style),
        transform: Transform::from_translation(vec3(0., 0., -0.5)).with_scale(vec3(0.1, 0.1, 1.)),
        ..default()
    });
}

fn update_sprites(mut simulation: Query<&mut RenderedSimulation>, mut counter: Query<&mut Text>) {
    let simulation = simulation.single_mut();

    let mut text = counter.single_mut();
    text.sections[0].value = format!("{}", simulation.0.size());
}

fn update_physics(
    client: Res<Client>,
    mut simulation: Query<(&mut RenderedSimulation, &mut GameController)>,
) {
    let (mut simulation, mut controller) = simulation.single_mut();
    let packets = client.0.get_packets(SUB_TICKS);
    let dt = 1. / 60. / SUB_TICKS as f32;

    for p in packets {
        controller.0.handle_packets(&mut simulation.0, &p);
        simulation.0.solve(dt);
    }
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_position: Local<Option<Vec2>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    client: Res<Client>,
    mut simulation: Query<(&mut RenderedSimulation, &mut GameController)>,
    mut camera: Query<(&Camera, &mut OrthographicProjection, &mut Transform)>,
) {
    let (camera, mut projection, mut camera_transform) = camera.single_mut();
    let (mut simulation, mut controller) = simulation.single_mut();
    let window = windows.single();

    // camera
    for ev in evr_scroll.read() {
        projection.scale *= f32::powf(1.25, ev.y);
    }

    let new_mouse_position = window.cursor_position().and_then(|cursor| {
        camera.viewport_to_world_2d(&GlobalTransform::from(camera_transform.clone()), cursor)
    });
    let delta = if new_mouse_position.is_some() && mouse_position.is_some() {
        new_mouse_position.unwrap() - mouse_position.unwrap()
    } else {
        Vec2::ZERO
    };
    if mouse.pressed(MouseButton::Right) {
        camera_transform.translation -= delta.extend(0.);
    } else {
        *mouse_position = new_mouse_position;
    }

    let mut factor: f32 = 1.;
    let mut shift_pressed = false;
    if keyboard.pressed(KeyCode::ShiftLeft) {
        factor = 5.;
        shift_pressed = true;
    }
    if keyboard.pressed(KeyCode::ArrowLeft) {
        camera_transform.translation.x -= 0.1 * factor;
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        camera_transform.translation.x += 0.1 * factor;
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        camera_transform.translation.y -= 0.1 * factor;
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        camera_transform.translation.y += 0.1 * factor;
    }

    // player
    if keyboard.pressed(KeyCode::KeyA) {
        client.0.send_packets(&controller.0.move_player(1.));
    } else if keyboard.pressed(KeyCode::KeyD) {
        client.0.send_packets(&controller.0.move_player(-1.));
    } else if keyboard.just_released(KeyCode::KeyA) || keyboard.just_released(KeyCode::KeyD) {
        client.0.send_packets(&controller.0.move_player(0.));
    }
    if keyboard.just_released(KeyCode::KeyW) {
        controller.0.player.gear_up()
    }
    if keyboard.just_released(KeyCode::KeyS) {
        controller.0.player.gear_down()
    }

    if let Some(cursor_world_position) = window.cursor_position().and_then(|cursor| {
        camera.viewport_to_world_2d(&GlobalTransform::from(camera_transform.clone()), cursor)
    }) {
        if keyboard.pressed(KeyCode::Digit1) {
            let range = if shift_pressed { -5..=5 } else { 0..=0 };
            for i in range {
                let pos = cursor_world_position + 2. * PARTICLE_RADIUS * i as f32;
                client.0.send_packet(GamePacket::Spawn(pos));
            }
        }

        if keyboard.just_released(KeyCode::Digit3) {
            client
                .0
                .send_packet(GamePacket::Tank(cursor_world_position));
        }
    }
}

fn lobby_system(mut client: ResMut<Client>, mut next_state: ResMut<NextState<GameState>>) {
    if client.0.game_started() {
        next_state.set(GameState::InGame);
        client.0.run();
    }
}

#[derive(Resource)]
struct Client(GameClient<GamePacket, PACKET_SIZE>);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum GameState {
    #[default]
    Menu,
    InLobby,
    InGame,
}

fn main() {
    const PHYSICS_UPDATE_TIME: u64 = 1000000000 / 64;

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 1 {
        warn!("provide an ip of the server as a command line argument");
    }

    let addr = &args[1];
    let default_name = "player".to_string();
    let name = args.get(2).unwrap_or(&default_name);
    let client = GameClient::<GamePacket, PACKET_SIZE>::new(addr, name.clone());

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RenderSimulationPlugin)
        .insert_resource(Client(client))
        .insert_state(GameState::InLobby)
        .add_systems(Update, lobby_system.run_if(in_state(GameState::InLobby)))
        .add_systems(OnEnter(GameState::InGame), setup_simulation)
        .add_systems(
            Update, 
            (update_physics, update_sprites, control_system).run_if(in_state(GameState::InGame)))
        .run();
}
