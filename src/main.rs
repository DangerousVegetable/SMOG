use std::time::{Duration, Instant};

use bevy::{
    color::palettes::css::RED,
    input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
    math::{vec2, vec3, Vec3A},
    prelude::*,
    render::{camera::ScalingMode, extract_component::ExtractComponent, primitives::Aabb},
    sprite::Anchor,
};

mod multithreaded;
mod particle;
mod solver;

mod pipeline;

use pipeline::RenderSimulationPlugin;
use solver::Solver;

const SUB_TICKS: usize = 8;

#[derive(Component)]
struct Simulation(Solver);

#[derive(Component)]
struct ParticleId(usize);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let solver = Solver::new(
        solver::Constraint::Box(vec2(-150., -50.), vec2(150., 250.)),
        solver::PARTICLE_SIZE * 2.,
        &[],
        &[],
    );
    let mut simulation = Simulation(solver);
    let (bl, tr) = simulation.0.constraint.bounds();
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

    for i in 0..100 {
        for j in 0..100 {
            let pos_x = (tr.x - bl.x) / 150. * i as f32
                + bl.x
                + solver::PARTICLE_SIZE
                + if j % 2 == 0 {
                    0.
                } else {
                    solver::PARTICLE_SIZE
                };
            let pos_y = -(tr.y - bl.y) / 150. * j as f32 + tr.y - solver::PARTICLE_SIZE;
            simulation
                .0
                .add_particle(particle::SAND.place(vec2(pos_x, pos_y)));
        }
    }

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

fn update_particle_sprites(
    mut simulation: Query<&mut Simulation>,
    mut counter: Query<&mut Text>,
) {
    let simulation = simulation.single_mut();

    let mut text = counter.single_mut();
    text.sections[0].value = format!("{}", simulation.0.particles.len());
}

fn update_physics(mut simulation: Query<&mut Simulation>) {
    //let time = Instant::now();
    let mut simulation = simulation.single_mut();
    for _ in 0..SUB_TICKS {
        let dt = 1./60./ SUB_TICKS as f32;
        simulation.0.solve(dt);
    }
    //println!("elapsed: {}", (Instant::now() - time).as_nanos() as f32 / 1000000.);
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut simulation: Query<&mut Simulation>,
    mut query: Query<(&mut OrthographicProjection, &mut Transform), With<Camera>>,
) {
    let (mut projection, mut camera_transform) = query.single_mut();
    let mut simulation = simulation.single_mut();

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

    let size = simulation.0.particles.len();
    if keyboard_input.pressed(KeyCode::Space) {
        simulation.0.change_number(size + 10 * factor as usize);
    }
    if keyboard_input.pressed(KeyCode::Delete) {
        simulation
            .0
            .change_number(std::cmp::max(0, size as isize - 40 * factor as isize) as usize);
    }
}

fn main() {
    //rayon::ThreadPoolBuilder::new()
    //    .num_threads(10)
    //    .build_global()
    //    .unwrap();

    const PHYSICS_UPDATE_TIME: u64 = 1000000000/64;

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RenderSimulationPlugin)
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_nanos(PHYSICS_UPDATE_TIME)))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, update_physics)
        .add_systems(Update, update_particle_sprites)
        .add_systems(Update, control_system)
        .run();
}
