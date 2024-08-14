use bevy::{
    color::palettes::css::RED,
    input::mouse::MouseWheel,
    math::{vec3, VectorSpace},
    prelude::*,
    render::camera::ScalingMode,
    window::PrimaryWindow,
};

use map_editor::map::MapLoader;
use solver::PARTICLE_RADIUS;

use render::{RenderSimulationPlugin, RenderedSimulation, SimulationCamera, SimulationTextures};

mod network;
use network::client::GameClient;

use packet_tools::game_packets::{GamePacket, PACKET_SIZE};

mod controller;
use controller::{model::RawPlayerModel, Controller, Player};

const SUB_TICKS: usize = 8;

#[derive(Component)]
struct GameController(Controller);

fn setup_simulation(mut commands: Commands, client: Res<Client>, asset_server: Res<AssetServer>) {
    // setup simulation
    let tank = RawPlayerModel::generate_tank();
    let lobby = &client.0.lobby;
    let map_loader = MapLoader::init_from_file(&lobby.map, &asset_server);
    commands.insert_resource(SimulationTextures {
        textures: map_loader.textures,
        background: map_loader.background,
    });

    let mut solver = map_loader.map.solver();
    let mut player_model = None;
    let mut players = Vec::new();
    for (id, _) in lobby.players.iter() {
        let model = RawPlayerModel::place_in_solver(
            tank.clone(),
            map_loader.map.spawns[*id as usize].pos,
            &mut solver,
        );
        if *id == lobby.id {
            player_model = Some(model.clone());
        }
        players.push((*id, model));
    }

    let simulation = RenderedSimulation(solver);

    // setup camera
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
        .insert(GameController(Controller::new(
            lobby.id,
            player_model.unwrap(),
            players,
        )));

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
    let packets = client.0.get_packets(1 * SUB_TICKS);
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
        client.0.send_packets(&controller.0.move_tank(1.));
    } else if keyboard.pressed(KeyCode::KeyD) {
        client.0.send_packets(&controller.0.move_tank(-1.));
    } else if keyboard.just_released(KeyCode::KeyA) || keyboard.just_released(KeyCode::KeyD) {
        client.0.send_packets(&controller.0.move_tank(0.));
    }
    if keyboard.just_released(KeyCode::KeyW) {
        controller.0.player.gear_up()
    }
    if keyboard.just_released(KeyCode::KeyS) {
        controller.0.player.gear_down()
    }
    // rotation
    if keyboard.pressed(KeyCode::KeyQ) {
        client.0.send_packets(&controller.0.rotate_tank(false));
    }
    if keyboard.pressed(KeyCode::KeyE) {
        client.0.send_packets(&controller.0.rotate_tank(true));
    }
    // dash
    if keyboard.pressed(KeyCode::Space) {
        client.0.send_packets(&controller.0.dash());
    }

    // shooting
    if let Some(cursor_world_position) = window.cursor_position().and_then(|cursor| {
        camera.viewport_to_world_2d(&GlobalTransform::from(camera_transform.clone()), cursor)
    }) {
        if keyboard.pressed(KeyCode::Digit1) {
            controller.0.player.projectile = 0;
        }
        if keyboard.pressed(KeyCode::Digit2) {
            controller.0.player.projectile = 1;
        }
        if keyboard.pressed(KeyCode::Digit3) {
            controller.0.player.projectile = 2;
        }
        if keyboard.pressed(KeyCode::Digit4) {
            controller.0.player.projectile = 3;
        }
        if keyboard.pressed(KeyCode::Digit5) {
            controller.0.player.projectile = 4;
        }

        if shift_pressed {
            client
                .0
                .send_packets(&controller.0.move_muzzle(cursor_world_position));
        }

        if mouse.pressed(MouseButton::Left) {
            client.0.send_packets(&controller.0.fire())
        }

        // debug commands
        if keyboard.pressed(KeyCode::Digit0) {
            let range = if shift_pressed { -5..=5 } else { 0..=0 };
            for i in range {
                let pos = cursor_world_position + 2. * PARTICLE_RADIUS * i as f32;
                client.0.send_packet(GamePacket::Spawn(pos));
            }
        }
        if keyboard.just_released(KeyCode::Digit9) {
            let _ = RawPlayerModel::generate_tank()
                .place_in_solver(cursor_world_position, &mut simulation.0);
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
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 1 {
        warn!("provide an ip of the server as a command line argument");
    }

    let addr = &args[1];
    let default_name = "player".to_string();
    let name = args.get(2).unwrap_or(&default_name);
    let client = GameClient::<GamePacket, PACKET_SIZE>::new(addr, name.clone()).unwrap();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SMOG".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RenderSimulationPlugin)
        .insert_resource(Time::<Fixed>::from_hz(64.0))
        .insert_resource(Client(client))
        .insert_state(GameState::InLobby)
        .add_systems(Update, lobby_system.run_if(in_state(GameState::InLobby)))
        .add_systems(OnEnter(GameState::InGame), setup_simulation)
        .add_systems(
            Update,
            (update_sprites, control_system).run_if(in_state(GameState::InGame)),
        )
        .add_systems(FixedUpdate, (update_physics).run_if(in_state(GameState::InGame)))
        .run();
}
