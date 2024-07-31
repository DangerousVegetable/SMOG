use std::time::{Duration, Instant};

use bevy::{
    color::palettes::css::RED, diagnostic::FrameTimeDiagnosticsPlugin, input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel}, math::{vec2, vec3, Vec3A}, prelude::*, render::{camera::ScalingMode, extract_component::ExtractComponent, primitives::Aabb}, sprite::Anchor, tasks::futures_lite::future, window::PrimaryWindow
};

mod solver;
use solver::particle;
use solver::Solver;

mod pipeline;
use pipeline::RenderSimulationPlugin;

mod network;
use network::{client::TcpClient, packets::GamePacket, packets::PACKET_SIZE};

mod controller;
use controller::Controller;

const SUB_TICKS: usize = 8;

#[derive(Component)]
struct Simulation(Controller);

fn setup(mut commands: Commands, client: Res<Client>) {
    let solver = Solver::new(
        solver::Constraint::Box(vec2(-300., -50.), vec2(300., 150.)),
        solver::PARTICLE_SIZE * 2.,
        &[],
        &[],
    );
    let simulation = Simulation(Controller::new(client.0.id, solver));
    let (bl, tr) = simulation.0.solver.constraint.bounds();
    let projection = OrthographicProjection {
        scale: 1.0,
        scaling_mode: ScalingMode::FixedHorizontal(tr.x - bl.x),
        ..Default::default()
    };

    // Spawn the camera with the custom projection
    commands.spawn(Camera2dBundle {
        projection: projection.into(),
        ..Default::default()
    });

    commands
        .spawn(SpatialBundle {
            visibility: Visibility::Visible,
            transform: Transform::IDENTITY,
            ..default()
        })
        .insert(simulation);

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

fn udpate_sprites(mut simulation: Query<&mut Simulation>, mut counter: Query<&mut Text>) {
    let simulation = simulation.single_mut();

    let mut text = counter.single_mut();
    text.sections[0].value = format!("{}", simulation.0.solver.size());
}

fn update_physics(client: Res<Client>, mut simulation: Query<&mut Simulation>) {
    let mut simulation = simulation.single_mut();
    let packets = client.0.get_packets(SUB_TICKS);
    let dt = 1./60./SUB_TICKS as f32;

    for p in packets {
        simulation.0.handle_packets(&p);
        simulation.0.solver.solve(dt);
    }
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    client: Res<Client>,
    mut simulation: Query<&mut Simulation>,
    mut query: Query<(&Camera, &mut OrthographicProjection, &mut Transform)>,
) {
    let (camera, mut projection, mut camera_transform) = query.single_mut();
    let mut simulation = simulation.single_mut();
    let window = windows.single();

    for ev in evr_scroll.read() {
        projection.scale *= f32::powf(1.25, ev.y);
    }

    let mut factor: f32 = 1.;
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        factor = 5.;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        camera_transform.translation.x -= 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        camera_transform.translation.x += 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        camera_transform.translation.y -= 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyW) {
        camera_transform.translation.y += 0.1 * factor;
    }

    let size = simulation.0.solver.size();
    //if keyboard_input.pressed(KeyCode::Space) {
    //    simulation.0.change_number(size + 10 * factor as usize);
    //}
    //if keyboard_input.pressed(KeyCode::Delete) {
    //    simulation
    //        .0
    //        .change_number(std::cmp::max(0, size as isize - 40 * factor as isize) as usize);
    //}

    if let Some(cursor_world_position) = window
        .cursor_position()
        .and_then(|cursor| {
            camera.viewport_to_world(&GlobalTransform::from(camera_transform.clone()), cursor)
        })
        .map(|ray| ray.origin.truncate())
    {
        if keyboard_input.pressed(KeyCode::Digit1) {
            client.0.send_packet(GamePacket::Spawn(cursor_world_position));
        }

        //if keyboard_input.pressed(KeyCode::Digit2) {
        //    let p = particle::METAL
        //        .place(cursor_world_position)
        //        .velocity(vec2(0., -0.5));
        //    simulation.0.add_particle(p);
        //}

        if keyboard_input.just_released(KeyCode::Digit3) {
            client.0.send_packet(GamePacket::Tank(cursor_world_position));
        }
    }
}

#[derive(Resource)]
struct Client(TcpClient<GamePacket, PACKET_SIZE>);

pub fn establish_connection(mut commands: Commands) {
    let client = TcpClient::<GamePacket, PACKET_SIZE>::new("127.0.0.1:8080");
    commands.insert_resource(Client(client));
}

fn exit_system(mut commands: Commands, events: EventReader<AppExit>) {
    if !events.is_empty() {
        info!("Stopping the client");
        commands.remove_resource::<Client>();
    }
}

fn main() {
    //rayon::ThreadPoolBuilder::new()
    //    .num_threads(10)
    //    .build_global()
    //    .unwrap();

    const PHYSICS_UPDATE_TIME: u64 = 1000000000 / 64;

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 1 {
        warn!("provide an ip of the server as a command line argument");
    }
    
    let addr = &args[1];
    let client = TcpClient::<GamePacket, PACKET_SIZE>::new(addr);

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RenderSimulationPlugin)
        .insert_resource(Client(client))
        //.insert_resource(Time::<Fixed>::from_duration(Duration::from_nanos(
        //    PHYSICS_UPDATE_TIME,
        //)))
        //.add_systems(PreStartup, establish_connection)
        .add_systems(Startup, setup)
        .add_systems(Update, update_physics)
        .add_systems(Update, udpate_sprites)
        .add_systems(Update, control_system)
        //.add_systems(Update, exit_system)
        .run();
}
