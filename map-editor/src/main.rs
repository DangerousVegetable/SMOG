use bevy::asset::{AssetLoader, AssetPath, LoadState, LoadedAsset};
use bevy::ecs::observer::TriggerTargets;
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

use text_io::try_read;

use map_editor::constructor::{self, MapConstructor};
use smog::render::{RenderSimulationPlugin, RenderedSimulation, SimulationCamera};
use solver::{Link, Solver};

#[derive(Component)]
struct UiCamera;

#[derive(Component)]
struct TextureColumn;

#[derive(Component)]
enum ButtonAction {
    AddTexture,
    RemoveTexture(Entity, Handle<Image>),
}

//#[derive(Component)]
//enum InputAction {
//    ChangeMass,
//    ChangeTexture,
//}

#[derive(Component)]
enum TextMarker {
    Mass,
    Texture,
    Strength,
    Durability,
    Elasticity,
}

fn setup_ui(mut commands: Commands) {
    let style = Style {
        width: Val::Px(160.0),
        height: Val::Px(30.0),
        border: UiRect::all(Val::Px(2.)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };

    let button = ButtonBundle {
        style: style.clone(),
        border_color: BorderColor(Color::WHITE),
        background_color: BackgroundColor(Color::BLACK),
        border_radius: BorderRadius::all(Val::Px(10.)),
        ..default()
    };

    let text_style = TextStyle {
        font: Default::default(),
        font_size: 20.0,
        color: Color::WHITE,
    };

    let text_node = NodeBundle {
        style: Style {
            width: Val::Px(160.0),
            height: Val::Percent(100.),
            border: UiRect::all(Val::Px(2.)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,    
            ..default()
        },
        border_color: Color::WHITE.into(),
        background_color: Color::BLACK.into(),
        border_radius: BorderRadius::all(Val::Px(10.)),
        ..default()
    };

    // Button container
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.),
                    height: Val::Percent(100.),
                    display: Display::Flex,
                    flex_direction: FlexDirection::ColumnReverse,
                    ..default()
                },
                ..default()
            },
            Interaction::None,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(10.0),
                        display: Display::Flex,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    // mass
                    parent.spawn(text_node.clone()).with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section("[M]ass:", text_style.clone()),
                            ..default()
                        });

                        parent
                            .spawn(TextBundle {
                                text: Text::from_section("---", text_style.clone()),
                                ..default()
                            })
                            .insert(TextMarker::Mass);
                    });
                    // texture
                    parent.spawn(text_node.clone()).with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section("[T]exture:", text_style.clone()),
                            ..default()
                        });

                        parent
                            .spawn(TextBundle {
                                text: Text::from_section("---", text_style.clone()),
                                ..default()
                            })
                            .insert(TextMarker::Texture);
                    });
                    // strength
                    parent.spawn(text_node.clone()).with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section("[S]trength:", text_style.clone()),
                            ..default()
                        });

                        parent
                            .spawn(TextBundle {
                                text: Text::from_section("---", text_style.clone()),
                                ..default()
                            })
                            .insert(TextMarker::Strength);
                    });

                    // durability
                    parent.spawn(text_node.clone()).with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section("[D]urability:", text_style.clone()),
                            ..default()
                        });

                        parent
                            .spawn(TextBundle {
                                text: Text::from_section("---", text_style.clone()),
                                ..default()
                            })
                            .insert(TextMarker::Durability);
                    });

                    // elasticity
                    parent.spawn(text_node.clone()).with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section("[E]lasticity:", text_style.clone()),
                            ..default()
                        });

                        parent
                            .spawn(TextBundle {
                                text: Text::from_section("---", text_style.clone()),
                                ..default()
                            })
                            .insert(TextMarker::Elasticity);
                    });
                });
            // Right column
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(20.0),
                        height: Val::Percent(100.0),
                        left: Val::Percent(80.),
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::End,
                        justify_content: JustifyContent::Start,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    // Button 4
                    parent
                        .spawn(button.clone())
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                text: Text::from_section("Add texture", text_style.clone()),
                                ..default()
                            });
                        })
                        .insert(ButtonAction::AddTexture);
                })
                .insert(TextureColumn);
        });
}

const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const BORDER_COLOR_INACTIVE: Color = Color::srgb(0.25, 0.25, 0.25);

fn update_ui_system(mut query: Query<(&mut Text, &TextMarker)>, constructor: Query<&Constructor>) {
    let constructor = constructor.single();
    if constructor.0.layers.len() > 0 {
        let layer = &constructor.0.layers[constructor.1];
        for (mut text, marker) in &mut query {
            match marker {
                TextMarker::Mass => text.sections[0].value = layer.base_particle.mass.to_string(),
                TextMarker::Texture => text.sections[0].value = layer.base_particle.texture.to_string(),
                TextMarker::Strength if layer.link.is_some() => text.sections[0].value = layer.strength.to_string(),
                TextMarker::Durability if layer.link.is_some() => {
                    text.sections[0].value = layer.link.unwrap().durability().to_string();
                },
                TextMarker::Elasticity if layer.link.is_some() => {
                    text.sections[0].value = format!("{} %", layer.link.unwrap().elasticity().to_string());
                },
                _ => text.sections[0].value = "---".to_string()
            }
        }
    }
}

#[derive(Component)]
struct Constructor(MapConstructor, usize);

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

    commands.spawn(RenderedSimulation(Solver::new(
        constructor.constraint,
        &[],
        &[],
    )));
    commands.spawn(Constructor(constructor, 0));
}

const NORMAL_BUTTON: Color = Color::BLACK;
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

fn button_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &ButtonAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut constructor: Query<&mut Constructor>,
) {
    let mut constructor = constructor.single_mut();
    for (interaction, button_action, mut background_color) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button_action {
                ButtonAction::AddTexture => {
                    if *state == AppState::Main {
                        *background_color = PRESSED_BUTTON.into();
                        next_state.set(AppState::PendingTexture(None));
                    } else {
                        *background_color = NORMAL_BUTTON.into();
                        next_state.set(AppState::Main);
                    }
                }
                ButtonAction::RemoveTexture(button, handle) => {
                    let Some(ind) = constructor.0.textures.iter().position(|h| h == handle) else {
                        return;
                    };
                    constructor.0.textures.remove(ind);
                    commands.entity(*button).despawn_recursive();
                    info!("Texture removed!");
                }
            }
        }
    }
}

//fn text_input_system

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
            AppState::Main | AppState::PendingImage(None) => {
                let img: Handle<Image> = asset_server.load(AssetPath::from_path(path_buf));
                info!("Loading layer with image: {:?}", img.path());
                next_state.set(AppState::PendingImage(Some(img)));
            }
            AppState::PendingTexture(None) => {
                let img: Handle<Image> = asset_server.load(AssetPath::from_path(path_buf));
                info!("Loading texture: {:?}", img.path());
                next_state.set(AppState::PendingTexture(Some(img)));
            }
            _ => (),
        }
    }
}

fn add_layer_from_image(constructor: &mut Constructor, img: &Image) {
    constructor.0.add_layer();
    let layer = constructor.0.layers.last_mut().unwrap();

    layer.init_from_image(img.clone());
    layer.link = Some(Link::Rigid {
        length: 1.,
        durability: 1.,
        elasticity: 1.,
    });
    layer.strength = 0.5;

    info!("Layer added!");
}

fn check_assets_system(
    mut commands: Commands,
    image_assets: Res<Assets<Image>>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut constructor: Query<&mut Constructor>,
    texture_column: Query<Entity, With<TextureColumn>>,
) {
    let mut constructor = constructor.get_single_mut().unwrap();
    let column = texture_column.single();
    match state.get() {
        AppState::PendingImage(Some(handle)) => {
            let Some(img) = image_assets.get(handle) else {
                return;
            };
            add_layer_from_image(&mut constructor, img);
            next_state.set(AppState::Main);
        }
        AppState::PendingTexture(Some(handle)) => {
            let Some(_) = image_assets.get(handle) else {
                return;
            };
            constructor.0.textures.push(handle.clone());
            info!("Texture added!");
            next_state.set(AppState::PendingTexture(None));

            let style = Style {
                width: Val::Px(160.0),
                height: Val::Px(30.0),
                border: UiRect::all(Val::Px(2.)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };

            let button = ButtonBundle {
                style: style.clone(),
                border_color: BorderColor(Color::WHITE),
                background_color: BackgroundColor(Color::BLACK),
                border_radius: BorderRadius {
                    top_left: Val::Px(10.),
                    top_right: Val::Px(10.),
                    bottom_left: Val::Px(10.),
                    bottom_right: Val::Px(10.),
                },
                image: UiImage::new(handle.clone()),
                ..default()
            };

            let texture_button = commands.spawn(button).id();
            commands
                .entity(texture_button)
                .insert(ButtonAction::RemoveTexture(texture_button, handle.clone()));
            commands.entity(column).push_children(&[texture_button]);
        }
        _ => (),
    }
}

fn control_system(
    mut evr_scroll: EventReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut simulation: Query<&mut RenderedSimulation>,
    mut constructor: Query<&mut Constructor>,
    mut camera: Query<(&Camera, &mut OrthographicProjection, &mut Transform)>,
) {
    let (camera, mut projection, mut camera_transform) = camera.single_mut();
    let mut simulation = simulation.single_mut();
    let mut constructor = constructor.single_mut();

    // camera controls
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

    // layer controls
    let layers_num = constructor.0.layers.len();
    if layers_num > 0 {
        if keyboard_input.just_pressed(KeyCode::ArrowLeft) {
            let ind = (constructor.1 + (layers_num - 1)) % layers_num;
            constructor.1 = ind;
            simulation.0 = constructor.0.layers[ind].solver();
            info!("Switching to layer: {ind}");
        }
        if keyboard_input.just_pressed(KeyCode::ArrowRight) {
            let ind = (constructor.1 + 1) % layers_num;
            constructor.1 = ind;
            simulation.0 = constructor.0.layers[ind].solver();
            info!("Switching to layer: {ind}");
        }

        let layer_ind = constructor.1;
        let layer = &mut constructor.0.layers[layer_ind];
        if keyboard_input.pressed(KeyCode::ControlLeft) {
            if keyboard_input.just_pressed(KeyCode::KeyM) {
                print!("mass << ");
                let read: Result<f32, _> = try_read!();
                let Ok(read) = read else {error!("Incorrect input!"); return};
                layer.base_particle.mass = read;
                info!("Mass updated!");
            }
            if keyboard_input.just_pressed(KeyCode::KeyT) {
                print!("texture << ");
                let read: Result<u32, _> = try_read!();
                let Ok(read) = read else {error!("Incorrect input!"); return};
                layer.base_particle.texture = read;
                info!("Texture updated!");
            }
            if keyboard_input.just_pressed(KeyCode::KeyS) {
                print!("strength << ");
                let read: Result<f32, _> = try_read!();
                let Ok(read) = read else {error!("Incorrect input!"); return};
                layer.strength = read;
                info!("Strength updated!");
            }
            if keyboard_input.just_pressed(KeyCode::KeyD) {
                print!("durability << ");
                let read: Result<f32, _> = try_read!();
                let Ok(read) = read else {error!("Incorrect input!"); return};
                let elasticity = layer.link.map_or(1., |l| l.elasticity());
                layer.link = Some(Link::Rigid{length: 1., durability: read, elasticity});
                info!("Durability updated!");
            }
            if keyboard_input.just_pressed(KeyCode::KeyE) {
                print!("elasticity << ");
                let read: Result<f32, _> = try_read!();
                let Ok(read) = read else {error!("Incorrect input!"); return};
                let durability = layer.link.map_or(1., |l| l.durability());
                layer.link = Some(Link::Rigid{length: 1., durability, elasticity: read});
                info!("Elasticity updated!");
            }
            if keyboard_input.just_pressed(KeyCode::Backspace) {
                layer.link = None;
                info!("All connections removed!");
            }
            layer.bake();
        }

        if keyboard_input.just_pressed(KeyCode::ArrowDown) {
            simulation.0 = constructor.0.layers[layer_ind].solver();
            info!("Showing layer: {layer_ind}");
        }
        if keyboard_input.just_released(KeyCode::Delete) {
            constructor.0.layers.remove(layer_ind);
            constructor.1 = usize::max(1, layer_ind) - 1;
            info!("Layer {layer_ind} removed");
        }
    }

    // simulation controls
    if keyboard_input.just_pressed(KeyCode::Enter) {
        constructor.0.bake_layers();
        simulation.0 = constructor.0.solver();
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        simulation.0 = constructor.0.solver();
    }

    if keyboard_input.pressed(KeyCode::Space) {
        let sub_ticks = 8;
        let dt = 1. / 60. / sub_ticks as f32;
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
        .add_systems(Update, update_ui_system)
        .add_systems(Update, button_system)
        .add_systems(Update, control_system)
        .run();
}
