use bevy::math::{vec2, vec3};
use bevy::{ 
    input::mouse::MouseWheel, prelude::*,
    render::camera::ScalingMode, window::PrimaryWindow,
};

use common::MAX_TEAMS;
use interface::OverlayPlugin;
use map_editor::map::MapLoader;
use render::{RenderedSimulation, SimulationCamera, SimulationTextures};
use packet_tools::game_packets::GamePacket;
use crate::{display_error, Client, GameState};
use crate::controller::{model::RawPlayerModel, Controller};

mod interface;

const SUB_TICKS: usize = 8;

#[derive(Component)]
pub struct GameController(pub Controller);

#[derive(Component)]
struct PlayerBanner(u8);

fn setup_simulation(
    mut commands: Commands,
    client: Res<Client>,
    asset_server: Res<AssetServer>,
    mut camera: Query<&mut OrthographicProjection, With<SimulationCamera>>,
    controller: Query<Entity, With<GameController>>,
) {
    // despawn old simulations
    despawn(&mut commands, &controller);

    // setup simulation
    let tank = RawPlayerModel::generate_tank();
    let lobby = &client.0.lobby;
    let map_loader = MapLoader::init_from_file(&lobby.map, &asset_server).unwrap(); // TODO: error handling
    commands.insert_resource(SimulationTextures {
        textures: map_loader.textures,
        background: map_loader.background,
    });

    let mut solver = map_loader.map.solver();
    let spawns = map_loader.map.spawns;
    let mut player_model = None;
    let mut players = Vec::new();
    for (id, name) in lobby.players.iter() {
        let model = RawPlayerModel::place_in_solver(
            tank.clone(),
            spawns[*id as usize].pos,
            &mut solver,
        );
        if *id == lobby.id {
            player_model = Some(model.clone());
        }
        players.push((*id, name.clone(), model));
    }

    let simulation = RenderedSimulation(solver);

    // setup camera
    let (bl, tr) = simulation.0.constraint.bounds();
    let projection = OrthographicProjection {
        scale: 1.0,
        scaling_mode: ScalingMode::FixedHorizontal(tr.x - bl.x),
        ..Default::default()
    };
    let mut camera_projection = camera.single_mut();
    *camera_projection = projection;

    // spawn player banners
    for (id, name, _) in players.iter() {
        let team = spawns[*id as usize].team;
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(name.clone(), TextStyle {
                    font_size: 60., 
                    color: Color::hsl(360. * team as f32 / MAX_TEAMS as f32, 1., 0.5),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .insert(PlayerBanner(*id));
    }

    // spawn controller
    commands
        .spawn(SpatialBundle {
            visibility: Visibility::Visible,
            transform: Transform::IDENTITY,
            ..default()
        })
        .insert(simulation)
        .insert(GameController(Controller::new(
            lobby.id,
            client.0.name.clone(),
            player_model.unwrap(),
            players,
            &spawns,
        )));
}

fn despawn(commands: &mut Commands, controller: &Query<Entity, With<GameController>>) {
    if let Ok(controller) = controller.get_single() {
        commands.entity(controller).despawn_recursive();
    }
}

fn update_physics(
    client: Res<Client>,
    mut simulation: Query<(&mut RenderedSimulation, &mut GameController)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let (mut simulation, mut controller) = simulation.single_mut();
    let packets = client.0.get_packets(1 * SUB_TICKS);
    let dt = 1. / 60. / SUB_TICKS as f32;

    for p in packets {
        controller.0.handle_packets(&mut simulation.0, &p);
        simulation.0.solve(dt);
        if controller.0.get_winners(&simulation.0).is_some() {
            next_state.set(GameState::EndGame);
            return;
        }
    }
}

fn update_banners(
    mut banners: Query<(&mut Transform, &PlayerBanner)>,
    simulation: Query<(&RenderedSimulation, &GameController)>
) {
    let (simulation, controller) = simulation.single();
    for (mut transform, id) in &mut banners {
        let player = controller.0.get_player(id.0).unwrap();
        let pos = controller.0.get_player_pos(player, &simulation.0) + vec2(0., 10.);
        *transform = Transform::from_translation(pos.extend(-0.5)).with_scale(vec3(0.1, 0.1, 1.));
    }
}

fn control_system(
    mut commands: Commands,
    mut evr_scroll: EventReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_position: Local<Option<Vec2>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    client: Res<Client>,
    mut simulation: Query<(&mut RenderedSimulation, &mut GameController)>,
    mut camera: Query<(&Camera, &mut OrthographicProjection, &mut Transform)>,
    mut next_state: ResMut<NextState<GameState>>,
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

    let mut packets: Vec<GamePacket> = vec![];
    // player
    if keyboard.pressed(KeyCode::KeyA) {
        packets.extend(&controller.0.move_tank(1.));
    } else if keyboard.pressed(KeyCode::KeyD) {
        packets.extend(&controller.0.move_tank(-1.));
    } 
    if keyboard.just_released(KeyCode::KeyA) || keyboard.just_released(KeyCode::KeyD) {
        packets.extend(&controller.0.move_tank(0.));
    }
    if keyboard.just_released(KeyCode::KeyW) {
        controller.0.player.gear_up()
    }
    if keyboard.just_released(KeyCode::KeyS) {
        controller.0.player.gear_down()
    }
    // rotation
    if keyboard.pressed(KeyCode::KeyQ) {
        packets.extend(&controller.0.rotate_tank(-0.01));
    } else if keyboard.pressed(KeyCode::KeyE) {
        packets.extend(&controller.0.rotate_tank(0.01));
    } 
    if keyboard.just_released(KeyCode::KeyQ) || keyboard.just_released(KeyCode::KeyE) {
        packets.extend(&controller.0.rotate_tank(0.))
    }
    // dash
    if keyboard.pressed(KeyCode::Space) {
        packets.extend(&controller.0.dash());
    }

    // shooting
    if let Some(cursor_world_position) = window.cursor_position().and_then(|cursor| {
        camera.viewport_to_world_2d(&GlobalTransform::from(camera_transform.clone()), cursor)
    }) {
        let digits = vec![
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
        ];

        for (projectile, key) in digits.into_iter().enumerate() {
            if keyboard.pressed(key) {
                controller.0.player.projectile = projectile as u8;
            }
        }

        if shift_pressed {
            packets.extend(&controller.0.move_muzzle(cursor_world_position));
        } 
        if keyboard.just_released(KeyCode::ShiftLeft){
            packets.extend(&controller.0.reset_muzzle());
        }

        if mouse.pressed(MouseButton::Left) {
            packets.extend(&controller.0.fire());
        }
    }

    match client.0.send_packets(&packets) {
        Err(e) => display_error(&mut commands, &mut next_state, &e.to_string()),
        _ => (),
    }
}

fn exit_system(mut commands: Commands, banners: Query<Entity, With<PlayerBanner>>) {
    commands.remove_resource::<Client>();
    for banner in &banners {
        commands.entity(banner).despawn_recursive();
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(OverlayPlugin)
        .insert_resource(Time::<Fixed>::from_hz(64.0))
            .add_systems(OnEnter(GameState::InGame), setup_simulation)
            .add_systems(OnExit(GameState::InGame), exit_system)
            .add_systems(Update, (control_system, update_banners).run_if(in_state(GameState::InGame)))
            .add_systems(
                FixedUpdate,
                (update_physics).run_if(in_state(GameState::InGame)),
            );
    }
}
