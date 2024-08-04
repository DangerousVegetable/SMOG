use bevy::asset::{AssetLoader, AssetPath, LoadedAsset};
use bevy::input::mouse::MouseWheel;
use bevy::math::vec2;
use bevy::prelude::*;

use bevy::render::camera::ScalingMode;
use bevy::render::render_asset::RenderAssets;
use bevy::render::texture::GpuImage;
use bevy::{
    self,
    app::App,
    prelude::{Camera2dBundle, Commands, Component, NodeBundle},
    ui::{AlignContent, JustifyContent, Style},
    DefaultPlugins,
};

use map_editor::constructor::{self, MapConstructor};
use smog::render::{RenderSimulationPlugin, RenderedSimulation, SimulationCamera};
use solver::{Link, Solver};

#[derive(Component)]
struct UiCamera;

fn setup_ui(mut commands: Commands) {
    // Button container
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::Flex,
                flex_direction: FlexDirection::ColumnReverse, // Reverse direction for bottom row
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            // Bottom row
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(10.0), // Adjust height as needed
                        display: Display::Flex,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    // Button 1
                    parent
                        .spawn(ButtonBundle {
                            style: Style {
                                width: Val::Px(80.0),
                                height: Val::Px(30.0),
                                margin: UiRect {
                                    left: Val::Px(10.0),
                                    right: Val::Px(10.0),
                                    ..default()
                                },
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                text: Text::from_section(
                                    "Button 1",
                                    TextStyle {
                                        font: Default::default(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                ..default()
                            });
                        });

                    parent
                        .spawn(ButtonBundle {
                            style: Style {
                                width: Val::Px(80.0),
                                height: Val::Px(30.0),
                                margin: UiRect {
                                    left: Val::Px(10.0),
                                    right: Val::Px(10.0),
                                    ..default()
                                },
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                text: Text::from_section(
                                    "Button 2",
                                    TextStyle {
                                        font: Default::default(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                ..default()
                            });
                        });

                    parent
                        .spawn(ButtonBundle {
                            style: Style {
                                width: Val::Px(80.0),
                                height: Val::Px(30.0),
                                margin: UiRect {
                                    left: Val::Px(10.0),
                                    right: Val::Px(10.0),
                                    ..default()
                                },
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                text: Text::from_section(
                                    "Button 3",
                                    TextStyle {
                                        font: Default::default(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                ..default()
                            });
                        });
                    // Button 2, 3, ...
                });

            // Right column
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(20.0), // Adjust width as needed
                        height: Val::Percent(100.0),
                        left: Val::Percent(80.),
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::End,
                        justify_content: JustifyContent::SpaceBetween,

                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    // Button 4
                    parent
                        .spawn(ButtonBundle {
                            style: Style {
                                width: Val::Px(80.0),
                                height: Val::Px(30.0),
                                margin: UiRect {
                                    top: Val::Px(10.0),
                                    bottom: Val::Px(10.0),
                                    ..default()
                                },
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                text: Text::from_section(
                                    "Button 4",
                                    TextStyle {
                                        font: Default::default(),
                                        font_size: 20.0,
                                        color: Color::WHITE,
                                    },
                                ),
                                ..default()
                            });
                        });
                    // Button 5, 6, ...
                });
        });
}

#[derive(Component)]
struct Constructor(MapConstructor);

fn setup(mut commands: Commands) {
    let constructor = MapConstructor::new(
        "map".to_string(),
        solver::Constraint::Box(vec2(-300., -50.), vec2(300., 150.)),
    );
    let (bl, tr) = constructor.constraint.bounds();
    let projection = OrthographicProjection {
        scale: 1.0,
        scaling_mode: ScalingMode::FixedHorizontal(tr.x - bl.x),
        ..Default::default()
    };

    // Spawn simulation camera
    commands
        .spawn(Camera2dBundle {
            projection: projection.into(),
            ..Default::default()
        })
        .insert(SimulationCamera);

    commands.spawn(RenderedSimulation(Solver::new(constructor.constraint, &[], &[])));
    commands.spawn(Constructor(constructor));

}

fn drag_and_drop_system(
    mut events: EventReader<FileDragAndDrop>,
    asset_server: Res<AssetServer>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for event in events.read() {
        let FileDragAndDrop::DroppedFile {
            window: _,
            path_buf,
        } = event
        else {
            return;
        };

        match state.get() {
            AppState::Main => {
                let img: Handle<Image> = asset_server.load(AssetPath::from_path(path_buf));
                info!("{:?}", img);
                next_state.set(AppState::PendingImage(Some(img)));
            }
            _ => (),
        }
    }
}

fn check_assets_system(
    mut events: EventReader<AssetEvent<Image>>,
    image_assets: Res<Assets<Image>>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut constructor: Query<&mut Constructor>,
) {
    let mut constructor = constructor.get_single_mut().unwrap();
    let AppState::PendingImage(Some(handle)) = state.get() else {
        return;
    };

    for event in events.read() {
        if event.is_added(handle) {
            let img = image_assets.get(handle).unwrap();
            constructor.0.add_layer();
            let layer = constructor.0.layers.last_mut().unwrap();
            layer.init_from_image(img.clone());
            layer.link = Some(Link::Rigid{
                length: 1.,
                durability: 1.,
                elasticity: 0.005,
            });
            layer.strength = 0.2;
            info!("Layer added!");
            next_state.set(AppState::Main);
        }
    }
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut simulation: Query<&mut RenderedSimulation>,
    constructor: Query<&Constructor>,
    mut camera: Query<(&Camera, &mut OrthographicProjection, &mut Transform)>,
) {
    let (camera, mut projection, mut camera_transform) = camera.single_mut();
    let mut simulation = simulation.single_mut();
    let constructor = constructor.single();

    // camera
    for ev in evr_scroll.read() {
        projection.scale *= f32::powf(1.25, ev.y);
    }

    let mut factor: f32 = 1.;
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        factor = 5.;
    }
    if keyboard_input.pressed(KeyCode::ArrowLeft) {
        camera_transform.translation.x -= 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::ArrowRight) {
        camera_transform.translation.x += 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        camera_transform.translation.y -= 0.1 * factor;
    }
    if keyboard_input.pressed(KeyCode::ArrowUp) {
        camera_transform.translation.y += 0.1 * factor;
    }
    
    if keyboard_input.pressed(KeyCode::Enter) {
        simulation.0 = constructor.0.solver();
    }

    if keyboard_input.pressed(KeyCode::Space) {
        let sub_ticks = 8;
        let dt = 1./60./sub_ticks as f32;
        for _ in 0..sub_ticks {
            simulation.0.solve(dt);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, States)]
enum AppState {
    Main,
    PendingTexture(Option<Handle<Image>>),
    PendingImage(Option<Handle<Image>>),
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, RenderSimulationPlugin))
        .insert_state(AppState::Main)
        .add_systems(Startup, setup_ui)
        .add_systems(Startup, setup)
        .add_systems(Update, drag_and_drop_system)
        .add_systems(Update, check_assets_system)
        .add_systems(Update, control_system)
        .run();
}
