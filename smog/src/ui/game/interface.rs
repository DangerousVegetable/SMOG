use bevy::prelude::*;

use crate::GameState;

use super::GameController;

#[derive(Component)]
struct Overlay;

#[derive(Component)]
enum OverlayTexture {
    Projectile(usize, Handle<Image>, Handle<Image>),
    Gear(Vec<Handle<Image>>),
}

#[derive(Component)]
enum OverlayProgress {
    DashProgress,
    ReloadProgress,
}

fn spawn(mut commands: Commands, asset_server: Res<AssetServer>) {
    let _display = build(&mut commands, &asset_server);
}

fn despawn(mut commands: Commands, lobby: Query<Entity, With<Overlay>>) {
    if let Ok(lobby) = lobby.get_single() {
        commands.entity(lobby).despawn_recursive();
    }
}

const BORDER_COLOR: Color = Color::srgb(0.25, 0.25, 0.25);
const BACKGROUND_COLOR: Color = Color::srgba(0., 0., 0., 0.9);

fn build(commands: &mut Commands, asset_server: &Res<AssetServer>) -> Entity {
    let projectile_node = NodeBundle {
        style: Style {
            width: Val::Px(80.0),
            height: Val::Px(80.0),
            ..default()
        },
        border_radius: BorderRadius::all(Val::Px(5.)),
        ..default()
    };

    let progress_texture = asset_server.load("textures/progress.png");

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Start,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            },
            Overlay,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.),
                        height: Val::Px(7.5),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(UiImage::new(progress_texture.clone()).with_color(Color::srgba(0., 0.7, 0., 0.9)))
                .insert(OverlayProgress::DashProgress);

            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.),
                        height: Val::Px(7.5),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(UiImage::new(progress_texture.clone()).with_color(Color::srgba(1., 0., 0., 0.9)))
                .insert(OverlayProgress::ReloadProgress);

            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(100.),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(80.),
                                height: Val::Percent(100.),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceAround,
                                flex_direction: FlexDirection::Row,
                                ..default()
                            },
                            border_radius: BorderRadius::bottom_left(Val::Px(50.)),
                            border_color: BORDER_COLOR.into(),
                            background_color: BACKGROUND_COLOR.into(),
                            ..default()
                        })
                        .with_children(|parent| {
                            for i in 0..3 {
                                let off =
                                    asset_server.load(format!("textures/projectiles/{}.png", i));
                                let on = asset_server
                                    .load(format!("textures/projectiles/{}-selected.png", i));
                                parent
                                    .spawn(projectile_node.clone())
                                    .insert(UiImage::default())
                                    .insert(OverlayTexture::Projectile(i, off, on));
                            }
                        });

                    let digits: Vec<_> = (0..6)
                        .map(|i| asset_server.load(format!("textures/digits/{}.png", i)))
                        .collect();

                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Px(100.),
                                height: Val::Percent(100.),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            border_radius: BorderRadius::bottom_right(Val::Px(50.)),
                            ..default()
                        })
                        .insert(UiImage {
                            color: Color::srgba(1., 1., 1., 0.9),
                            ..Default::default()
                        })
                        .insert(OverlayTexture::Gear(digits));
                });
        })
        .id()
}

fn update_overlay_textures(
    mut overlays: Query<(&mut UiImage, &OverlayTexture)>,
    controller: Query<&GameController>,
) {
    let controller = controller.single();
    let projectile = controller.0.player.projectile as usize;

    for (mut ui_image, overlay) in &mut overlays {
        match overlay {
            OverlayTexture::Projectile(id, off, on) => {
                if *id == projectile {
                    ui_image.texture = on.clone();
                } else {
                    ui_image.texture = off.clone();
                }
            }
            OverlayTexture::Gear(digits) => {
                ui_image.texture = digits[controller.0.player.gear].clone();
            }
        }
    }
}

fn update_overlay_progress(
    mut overlays: Query<(&mut Style, &OverlayProgress)>,
    controller: Query<&GameController>,
) {
    let controller = controller.single();

    for (mut style, overlay) in &mut overlays {
        match overlay {
            OverlayProgress::ReloadProgress => {
                let progress = controller.0.player.reload_timer.progress() * 100.;
                style.width = Val::Percent(progress);
            },
            OverlayProgress::DashProgress => {
                let progress = controller.0.player.dash_timer.progress() * 100.;
                style.width = Val::Percent(progress);
            }
        }
    }
}

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn)
            .add_systems(OnExit(GameState::InGame), despawn)
            .add_systems(
                Update,
                (update_overlay_textures, update_overlay_progress).run_if(in_state(GameState::InGame)),
            );
    }
}
