use bevy::{input::{keyboard::{Key, KeyboardInput}, ButtonState}, prelude::*};
use render::RenderedSimulation;

use crate::GameState;

use super::game::GameController;

#[derive(Component)]
struct WinScreen;

fn spawn(mut commands: Commands, controller: Query<(&GameController, &RenderedSimulation)>) {
    let _winscreen = build(&mut commands, &controller);
}

fn despawn(mut commands: Commands, win_screen: Query<Entity, With<WinScreen>>) {
    if let Ok(win_screen) = win_screen.get_single() {
        commands.entity(win_screen).despawn_recursive();
    }
}

const _BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const BORDER_COLOR_INACTIVE: Color = Color::srgb(0.25, 0.25, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

fn build(commands: &mut Commands, game: &Query<(&GameController, &RenderedSimulation)>) -> Entity {
    let text_style = TextStyle {
        font_size: 160.,
        color: TEXT_COLOR,
        ..default()
    };

    let (controller, simulation) = game.single();
    let (team, _) = controller.0.get_winners(&simulation.0).unwrap();

    let text = if team == controller.0.player.team {
        TextBundle::from_section(
            "VICTORY",
            text_style,
        )
    } else {
        TextBundle::from_section(
            "DEFEAT",
            TextStyle {
                color: Color::srgb(0.9, 0., 0.,),
                ..text_style
            },
        )
    };

    let node_bundle = NodeBundle {
        style: Style {
            width: Val::Percent(80.),
            border: UiRect::all(Val::Px(5.0)),
            padding: UiRect::all(Val::Px(5.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        border_color: BORDER_COLOR_INACTIVE.into(),
        background_color: BACKGROUND_COLOR.into(),
        ..default()
    };

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            },
            WinScreen,
        ))
        .with_children(|parent| {
            parent.spawn(node_bundle).with_children(|parent| {
                parent.spawn(text);
            });
        })
        .id()
}

pub fn esc_system(mut keyboard: EventReader<KeyboardInput>, mut next_state: ResMut<NextState<GameState>>) {
    for ev in keyboard.read() {
        if ev.state == ButtonState::Released {
            continue;
        }
        match &ev.logical_key {
            Key::Escape => {
                next_state.set(GameState::Menu);
            }
            _ => (),
        }
    }
}
pub struct WinScreenPlugin;

impl Plugin for WinScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::EndGame), spawn)
            .add_systems(OnExit(GameState::EndGame), despawn)
            .add_systems(Update, esc_system.run_if(in_state(GameState::EndGame)));
    }
}
