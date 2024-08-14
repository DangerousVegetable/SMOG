use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInputBundle, TextInputInactive, TextInputPlugin, TextInputSystem, TextInputValue,
};
use packet_tools::game_packets::GamePacket;

use crate::{display_error, network::client::GameClient, Client, GameError, GameState, PACKET_SIZE};

#[derive(Component)]
struct MainMenu;

fn spawn(mut commands: Commands, asset_server: Res<AssetServer>, error: Option<Res<GameError>>) {
    let _menu = build(&mut commands, &asset_server, &error);
}

fn despawn(mut commands: Commands, main_menu: Query<Entity, With<MainMenu>>) {
    if let Ok(main_menu) = main_menu.get_single() {
        commands.entity(main_menu).despawn_recursive();
    }
}

const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const BORDER_COLOR_INACTIVE: Color = Color::srgb(0.25, 0.25, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

fn build(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    error: &Option<Res<GameError>>,
) -> Entity {
    let text_style = TextStyle {
        font_size: 40.,
        color: TEXT_COLOR,
        ..default()
    };

    let node_bundle = NodeBundle {
        style: Style {
            width: Val::Px(300.0),
            border: UiRect::all(Val::Px(5.0)),
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        border_color: BORDER_COLOR_INACTIVE.into(),
        background_color: BACKGROUND_COLOR.into(),
        // Prevent clicks on the input from also bubbling down to the container
        // behind it
        focus_policy: bevy::ui::FocusPolicy::Block,
        ..default()
    };

    commands.remove_resource::<GameError>();

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            },
            // Make this container node bundle to be Interactive so that clicking on it removes
            // focus from the text input.
            Interaction::None,
            MainMenu
        ))
        .with_children(|parent| {
            parent.spawn((
                node_bundle.clone(),
                TextInputBundle::default()
                    .with_text_style(text_style.clone())
                    .with_placeholder("nickname", None)
                    .with_inactive(true),
                NicknameInput,
            ));
            parent.spawn((
                node_bundle.clone(),
                TextInputBundle::default()
                    .with_text_style(text_style.clone())
                    .with_placeholder("127.0.0.1:8080", None)
                    .with_inactive(true),
                AddrInput,
            ));

            parent
                .spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(200.),
                            border: UiRect::all(Val::Px(5.0)),
                            padding: UiRect::all(Val::Px(5.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        border_color: BorderColor(BORDER_COLOR_INACTIVE),
                        background_color: BACKGROUND_COLOR.into(),
                        ..default()
                    },
                    ConnectButton,
                ))
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section("Connect", text_style.clone()));
                });

            if let Some(error) = error {
                parent.spawn(node_bundle.clone()).with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        error.0.clone(),
                        TextStyle {
                            color: Color::srgb(1., 0., 1.),
                            ..text_style
                        },
                    ));
                });
            }
        })
        .id()
}

fn focus(
    query: Query<(Entity, &Interaction), Changed<Interaction>>,
    mut text_input_query: Query<(Entity, &mut TextInputInactive, &mut BorderColor)>,
) {
    for (interaction_entity, interaction) in &query {
        if *interaction == Interaction::Pressed {
            for (entity, mut inactive, mut border_color) in &mut text_input_query {
                if entity == interaction_entity {
                    inactive.0 = false;
                    *border_color = BORDER_COLOR_ACTIVE.into();
                } else {
                    inactive.0 = true;
                    *border_color = BORDER_COLOR_INACTIVE.into();
                }
            }
        }
    }
}

fn connect_system(
    mut commands: Commands,
    nick: Query<&TextInputValue, With<NicknameInput>>,
    addr: Query<&TextInputValue, With<AddrInput>>,
    mut next_state: ResMut<NextState<GameState>>,
    connect_button: Query<&Interaction, (With<ConnectButton>, Changed<Interaction>)>,
) {
    for interaction in &connect_button {
        if matches!(interaction, Interaction::Pressed) {
            let nick = nick.single().0.clone();
            let addr = addr.single().0.clone();

            match GameClient::<GamePacket, PACKET_SIZE>::new(addr, nick) {
                Ok(client) => {
                    commands.insert_resource(Client(client));
                    next_state.set(GameState::InLobby);
                }
                Err(e) => display_error(&mut commands, &mut next_state, &e.to_string())
            }
        }
    }
}

#[derive(Component)]
struct NicknameInput;

#[derive(Component)]
struct AddrInput;

#[derive(Component)]
struct ConnectButton;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TextInputPlugin)
            .add_systems(OnEnter(GameState::Menu), spawn)
            .add_systems(OnExit(GameState::Menu), despawn)
            .add_systems(
                Update,
                (focus.before(TextInputSystem), connect_system).run_if(in_state(GameState::Menu)),
            )
            .add_systems(
                Update,
                (|mut next_state: ResMut<NextState<GameState>>| {next_state.set(GameState::Menu)}).run_if(in_state(GameState::Error))
            );
    }
}
