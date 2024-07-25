use std::time::{Duration, Instant};

use bevy::{
    color::palettes::css::RED, input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel}, math::{vec2, vec3}, prelude::*, render::camera::ScalingMode, sprite::Anchor
};

mod multithreaded;
mod particle;
mod solver;

use solver::Solver;

const SUB_TICKS: usize = 8;

#[derive(Resource)]
struct Simulation(Solver);

#[derive(Component)]
struct ParticleId(usize);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut simulation: ResMut<Simulation>) {
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

    simulation.0.change_number(10000);

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
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    simulation: Res<Simulation>,
    mut counter: Query<&mut Text>,
    mut sprites: Query<(Entity, &mut Transform, &ParticleId)>,
) {
    let mut last_ind = None;
    for (sprite, mut transform, i) in &mut sprites {
        let ind = i.0;
        if ind >= simulation.0.particles.len() {
            commands.entity(sprite).despawn();
            continue;
        }

        let p = &simulation.0.particles[ind];
        transform.translation.x = p.pos.x;
        transform.translation.y = p.pos.y;
        last_ind = Some(ind);
    }

    let last_ind = last_ind.map_or(0, |i| i + 1);

    let texture_handle: Handle<Image> = asset_server.load("particle-xd.png");
    for i in last_ind..simulation.0.particles.len() {
        let p = &simulation.0.particles[i];
        commands.spawn((
            SpriteBundle {
                texture: texture_handle.clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(
                        2. * solver::PARTICLE_SIZE,
                        2. * solver::PARTICLE_SIZE,
                    )),
                    ..Default::default()
                },
                transform: Transform::from_xyz(p.pos.x, p.pos.y, -1.),
                ..default()
            },
            ParticleId(i),
        ));
    }

    let mut text = counter.single_mut();
    text.sections[0].value = format!("{}", simulation.0.particles.len());
}

fn update_physics(mut simulation: ResMut<Simulation>) {
    //let time = Instant::now();
    for _ in 0..SUB_TICKS {
        let dt = 0.08 / SUB_TICKS as f32;
        simulation.0.solve(dt);
    }
    //println!("elapsed: {}", (Instant::now() - time).as_nanos() as f32 / 1000000.);
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<Simulation>,
    mut query: Query<(&mut OrthographicProjection, &mut Transform), With<Camera>>,
) {
    let (mut projection, mut camera_transform) = query.single_mut();

    for ev in evr_scroll.read() {
        projection.scale *= f32::powf(1.25, ev.y);
    }

    let mut factor: f32 = 1.;
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        factor = 5.;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        camera_transform.translation.x -= 0.05 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        camera_transform.translation.x += 0.05 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        camera_transform.translation.y -= 0.05 * factor;
    }
    if keyboard_input.pressed(KeyCode::KeyW) {
        camera_transform.translation.y += 0.05 * factor;
    }

    let size = simulation.0.particles.len();
    if keyboard_input.pressed(KeyCode::Space) {
        simulation.0.change_number(size + 10*factor as usize);
    }
    if keyboard_input.pressed(KeyCode::Delete) {
        simulation.0.change_number(std::cmp::max(0, size as isize - 40*factor as isize) as usize);
    }
}

fn main() {
    //rayon::ThreadPoolBuilder::new()
    //    .num_threads(10)
    //    .build_global()
    //    .unwrap();

    let solver = Solver::new(
        solver::Constraint::Box(vec2(-60., -10.), vec2(60., 40.)),
        solver::PARTICLE_SIZE * 2.,
        &[],
        &[],
    );

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_millis(16)))
        .insert_resource(Simulation(solver))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, update_physics)
        .add_systems(Update, update_particle_sprites)
        .add_systems(Update, control_system)
        .run();
}
