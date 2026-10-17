//! Front-end screens stay responsive without advancing the gameplay simulation.

mod background;
mod lettering;

use crate::{
    difficulty::Difficulty,
    game::{Game, Status},
    level::{Levels, LoadReason},
    scores::Store,
};
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
    },
    prelude::*,
    window::Ime,
};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver};

const CONFIG_CHOICES: usize = if cfg!(target_arch = "wasm32") { 2 } else { 3 };

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    Welcome,
    Configuration,
    Initials,
    #[default]
    Gameplay,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(crate) enum Preview {
    Welcome,
    WelcomeKey,
    Configuration,
    Initials,
    KeyInitials,
}

#[derive(Resource)]
pub(crate) struct FrontEnd {
    pub(crate) screen: Screen,
    pub(crate) eligible: bool,
    pub(crate) originals: Vec<crate::level::Level>,
    selected: usize,
    start_selected: usize,
    start_level: usize,
    initials: [u8; 3],
    cursor: usize,
    typed: usize,
    score: u32,
    high_score: bool,
    key_level: Option<u8>,
    highlight: Option<(Difficulty, usize)>,
    #[cfg(not(target_arch = "wasm32"))]
    install: Option<std::sync::Mutex<Receiver<Result<String, String>>>>,
    notice: String,
    blocked: Vec<KeyCode>,
    physical: ButtonInput<KeyCode>,
}

impl FrontEnd {
    pub(crate) fn preview_initials(&mut self) {
        self.enter_initials(12345, "AAA");
    }

    pub(crate) fn preview_key_initials(&mut self) {
        self.preview_initials();
        self.high_score = false;
        self.key_level = Some(31);
    }

    pub(crate) fn preview_key_start(&mut self, store: &mut Store, difficulty: Difficulty) {
        if self
            .available_keys(&store.data, difficulty)
            .next()
            .is_none()
        {
            store.data.save_key(difficulty, 31, "AAA");
        }
        self.start_selected = 1;
        self.start_level = self.maximum_start_level(&store.data, difficulty);
    }

    pub(crate) fn new(screen: Screen, eligible: bool, originals: Vec<crate::level::Level>) -> Self {
        Self {
            screen,
            eligible,
            originals,
            selected: 0,
            start_selected: 0,
            start_level: 1,
            initials: *b"AAA",
            cursor: 0,
            typed: 0,
            score: 0,
            high_score: true,
            key_level: None,
            highlight: None,
            #[cfg(not(target_arch = "wasm32"))]
            install: None,
            notice: String::new(),
            blocked: Vec::new(),
            physical: default(),
        }
    }

    fn enter_initials(&mut self, score: u32, previous: &str) {
        self.screen = Screen::Initials;
        self.score = score;
        self.high_score = true;
        self.key_level = None;
        self.initials.copy_from_slice(previous.as_bytes());
        self.cursor = 0;
        self.typed = 0;
    }

    fn available_keys<'a>(
        &self,
        data: &'a crate::scores::Data,
        difficulty: Difficulty,
    ) -> impl Iterator<Item = &'a crate::scores::PlayerKey> {
        let maximum = self.originals.len();
        data.keys.iter().filter(move |key| {
            let level = key.levels[difficulty.index()];
            level != 0 && usize::from(level) <= maximum
        })
    }

    fn selected_key<'a>(
        &self,
        data: &'a crate::scores::Data,
        difficulty: Difficulty,
    ) -> Option<&'a crate::scores::PlayerKey> {
        self.start_selected
            .checked_sub(1)
            .and_then(|index| self.available_keys(data, difficulty).nth(index))
    }

    fn maximum_start_level(&self, data: &crate::scores::Data, difficulty: Difficulty) -> usize {
        self.selected_key(data, difficulty).map_or_else(
            || {
                let maximum = self.originals.len().clamp(1, 15);
                maximum - usize::from(maximum.is_multiple_of(2))
            },
            |key| usize::from(key.levels[difficulty.index()]),
        )
    }

    pub(crate) fn preview_level(&self) -> usize {
        self.start_level
    }

    fn save_result(&mut self, store: &mut Store, difficulty: Difficulty, initials: String) {
        store.data.initials.clone_from(&initials);
        self.highlight = store
            .data
            .insert(difficulty, self.score, initials.clone())
            .map(|rank| (difficulty, rank));
        self.notice.clear();
        if let Some(level) = self.key_level
            && store.data.save_key(difficulty, level, &initials)
        {
            self.notice = format!("{initials} KEY SAVED AT LEVEL {level}");
        }
        store.save();
    }

    fn type_letters(&mut self, text: &str) {
        for letter in text.bytes().filter(u8::is_ascii_alphabetic) {
            if self.typed == 3 {
                break;
            }
            self.initials[self.cursor] = letter.to_ascii_uppercase();
            self.typed += 1;
            self.cursor = (self.cursor + 1).min(2);
        }
    }
}

pub(crate) fn playing(front: Option<Res<FrontEnd>>) -> bool {
    front.is_none_or(|front| front.screen == Screen::Gameplay)
}

pub(crate) fn add_systems(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(
            PreUpdate,
            input
                .after(crate::gamepad::map_controllers)
                .before(crate::pause::input),
        )
        .add_systems(Update, (draw, cover_gameplay))
        .add_systems(Update, draw_game_over_hint);
    lettering::add_systems(app);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, installation.before(draw));
    background::add_systems(app);
}

/// Menus replace the game, so stop rendering it underneath. The overlay camera
/// keeps rendering the UI, but not the HUD and messages on its layer.
#[allow(clippy::type_complexity)]
fn cover_gameplay(
    front: Res<FrontEnd>,
    mut cameras: Query<&mut Camera, Or<(With<crate::camera::WebCamera>, With<Camera2d>)>>,
    mut overlay: Query<&mut bevy::camera::visibility::RenderLayers, With<IsDefaultUiCamera>>,
) {
    let active = front.screen == Screen::Gameplay;
    for mut camera in &mut cameras {
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
    for mut layers in &mut overlay {
        layers.set_if_neq(if active {
            bevy::camera::visibility::RenderLayers::layer(3)
        } else {
            bevy::camera::visibility::RenderLayers::none()
        });
    }
}

fn confirm(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space])
}

fn welcome(front: &mut FrontEnd, messages: &mut crate::messages::Messages) {
    front.screen = Screen::Welcome;
    front.start_selected = 0;
    front.start_level = 1;
    front.key_level = None;
    *messages = default();
}

pub(crate) fn abandon(
    front: &mut FrontEnd,
    game: &mut Game,
    messages: &mut crate::messages::Messages,
    commands: &mut Commands,
) {
    // Keep the epoch so the next game's first load still reads as a new one.
    *game = Game {
        message_epoch: game.message_epoch,
        ..default()
    };
    crate::sound::reset(commands);
    welcome(front, messages);
    front.blocked = front.physical.get_pressed().copied().collect();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut text: MessageReader<KeyboardInput>,
    mut ime: MessageReader<Ime>,
    mut front: ResMut<FrontEnd>,
    mut store: ResMut<Store>,
    mut difficulty: ResMut<Difficulty>,
    mut levels: ResMut<Levels>,
    mut game: ResMut<Game>,
    mut messages: ResMut<crate::messages::Messages>,
    mut windows: Query<&mut Window>,
    #[cfg(not(target_arch = "wasm32"))] mut exit: MessageWriter<AppExit>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    time: Option<Res<Time<Virtual>>>,
    mut commands: Commands,
) {
    let old_screen = front.screen;
    if focus_lost.read().next().is_some() {
        front.physical.reset_all();
        front.blocked.clear();
    }
    let events: Vec<_> = text.read().cloned().collect();
    for event in &events {
        if event.state == ButtonState::Released {
            front.physical.release(event.key_code);
            front.blocked.retain(|key| *key != event.key_code);
        } else {
            front.physical.press(event.key_code);
        }
    }
    for key in &front.blocked {
        keys.reset(*key);
    }
    if front.screen == Screen::Gameplay {
        ime.clear();
        if keys.just_pressed(KeyCode::Escape) && time.is_none_or(|time| !time.is_paused()) {
            abandon(&mut front, &mut game, &mut messages, &mut commands);
        } else if game.status != Status::GameOver || !confirm(&keys) {
            return;
        } else {
            let high_score = front.eligible && store.data.rank(*difficulty, game.score).is_some();
            let checkpoint = game
                .key_level()
                .filter(|&level| front.eligible && store.data.key_qualifies(*difficulty, level));
            if front.eligible
                && let Some(initials) = game.key_initials
            {
                front.score = game.score;
                front.key_level = checkpoint;
                front.save_result(
                    &mut store,
                    *difficulty,
                    String::from_utf8(initials.to_vec()).unwrap(),
                );
                welcome(&mut front, &mut messages);
            } else if high_score || checkpoint.is_some() {
                front.enter_initials(game.score, &store.data.initials);
                front.high_score = high_score;
                front.key_level = checkpoint;
            } else {
                welcome(&mut front, &mut messages);
            }
        }
    } else {
        let typed: String = events
            .iter()
            .filter(|event| event.state == ButtonState::Pressed)
            .filter_map(|event| match &event.logical_key {
                Key::Character(character) => Some(character.as_str()),
                _ => event.text.as_deref(),
            })
            .collect();
        let committed: String = ime
            .read()
            .filter_map(|event| {
                if let Ime::Commit { value, .. } = event {
                    Some(value.as_str())
                } else {
                    None
                }
            })
            .collect();
        match front.screen {
            Screen::Welcome => {
                if keys.any_just_pressed([KeyCode::KeyP, KeyCode::Pause]) {
                    front.screen = Screen::Configuration;
                    front.selected = 0;
                } else if confirm(&keys) {
                    levels.0 = front.originals.clone();
                    for level in &mut levels.0 {
                        level.scale_difficulty(difficulty.scale());
                    }
                    let initials = front
                        .selected_key(&store.data, *difficulty)
                        .map(|key| key.initials.as_bytes().try_into().unwrap());
                    game.load = Some(LoadReason::StartLevel {
                        index: front.start_level - 1,
                        initials,
                    });
                    front.screen = Screen::Gameplay;
                    front.highlight = None;
                    front.notice.clear();
                } else if keys.just_pressed(KeyCode::Escape) {
                    #[cfg(not(target_arch = "wasm32"))]
                    exit.write(AppExit::Success);
                } else {
                    let left = keys.just_pressed(KeyCode::ArrowLeft);
                    let right = keys.just_pressed(KeyCode::ArrowRight);
                    if left != right {
                        let choices = 1 + front.available_keys(&store.data, *difficulty).count();
                        let previous = front.start_selected;
                        front.start_selected =
                            (front.start_selected + if left { choices - 1 } else { 1 }) % choices;
                        if front.start_selected != previous {
                            front.start_level = front
                                .selected_key(&store.data, *difficulty)
                                .map_or(1, |key| usize::from(key.levels[difficulty.index()]));
                        }
                    }
                    let up = keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]);
                    let down = keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]);
                    if up != down {
                        let maximum = front.maximum_start_level(&store.data, *difficulty);
                        front.start_level = if up {
                            (front.start_level + 2).min(maximum)
                        } else {
                            front.start_level.saturating_sub(2).max(1)
                        };
                    }
                }
            }
            Screen::Configuration => {
                if keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyP, KeyCode::Pause]) {
                    welcome(&mut front, &mut messages);
                } else {
                    let up = keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]);
                    let down = keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]);
                    if up != down {
                        front.selected = (front.selected + if up { CONFIG_CHOICES - 1 } else { 1 })
                            % CONFIG_CHOICES;
                    }
                    if front.selected == 0 {
                        let left = keys.just_pressed(KeyCode::ArrowLeft);
                        let right = keys.just_pressed(KeyCode::ArrowRight) || confirm(&keys);
                        if left != right {
                            *difficulty = Difficulty::ALL
                                [(difficulty.index() + if left { 2 } else { 1 }) % 3];
                            store.data.difficulty = *difficulty;
                            front.start_selected = 0;
                            front.start_level = 1;
                            store.save();
                        }
                    } else if confirm(&keys) {
                        if front.selected == CONFIG_CHOICES - 1 {
                            welcome(&mut front, &mut messages);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        if front.selected == 1 && front.install.is_none() {
                            if std::env::var_os("FLATPAK_ID").is_some() {
                                front.notice = "Steam installation is unavailable inside Flatpak. Use a native build to add the game to Steam.".into();
                            } else {
                                let (sender, receiver) = mpsc::channel();
                                front.install = Some(std::sync::Mutex::new(receiver));
                                front.notice = "Installing in Steam...".into();
                                std::thread::spawn(move || {
                                    let result = crate::steam::install_detected(None, None)
                                        .map(|installed| installed.message())
                                        .map_err(|error| error.to_string());
                                    let _ = sender.send(result);
                                });
                            }
                        }
                    }
                }
            }
            Screen::Initials => {
                front.type_letters(&typed);
                front.type_letters(&committed);
                if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Escape) {
                    front.cursor = if front.typed > 0 {
                        front.typed - 1
                    } else {
                        front.cursor.saturating_sub(1)
                    };
                    front.typed = front.cursor;
                    let cursor = front.cursor;
                    front.initials[cursor] = b'A';
                }
                if keys.just_pressed(KeyCode::ArrowLeft) {
                    front.cursor = (front.cursor + 2) % 3;
                    front.typed = front.cursor;
                } else if keys.just_pressed(KeyCode::ArrowRight) {
                    front.cursor = (front.cursor + 1) % 3;
                    front.typed = front.cursor;
                }
                let up = keys.just_pressed(KeyCode::ArrowUp);
                let down = keys.just_pressed(KeyCode::ArrowDown);
                if up != down {
                    let cursor = front.cursor;
                    front.initials[cursor] =
                        b'A' + (front.initials[cursor] - b'A' + if up { 1 } else { 25 }) % 26;
                }
                if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
                    let initials = String::from_utf8(front.initials.to_vec()).unwrap();
                    if front.eligible {
                        front.save_result(&mut store, *difficulty, initials);
                    }
                    welcome(&mut front, &mut messages);
                }
            }
            Screen::Gameplay => unreachable!(),
        }
    }
    if old_screen != front.screen {
        front.blocked = front.physical.get_pressed().copied().collect();
        for mut window in &mut windows {
            window.ime_enabled = front.screen == Screen::Initials;
            window.ime_position = Vec2::new(window.width() / 2., window.height() * 0.48);
        }
        keys.reset_all();
    } else {
        keys.clear();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn installation(mut front: ResMut<FrontEnd>) {
    let result =
        front
            .install
            .as_ref()
            .and_then(|receiver| match receiver.lock().unwrap().try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Installer stopped unexpectedly".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
    if let Some(result) = result {
        front.install = None;
        front.notice = result.unwrap_or_else(|error| format!("Steam installation failed: {error}"));
    }
}

#[derive(Component)]
struct Root;
#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Body;
#[derive(Component)]
struct Logo;
#[derive(Component)]
struct GameOverHint;
#[derive(Component)]
enum Label {
    Title,
    Body,
    Footer,
    Notice,
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let logo = images.add(
        Image::from_buffer(
            include_bytes!("../assets/steam/logo.png"),
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::linear(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .unwrap(),
    );
    lettering::setup(&mut commands, &mut meshes, &mut materials);
    background::setup_preview(&mut commands, &mut meshes, &mut materials);
    commands.spawn((
        GameOverHint,
        Text::default(),
        TextFont {
            font_size: 18.,
            ..default()
        },
        TextColor(Color::srgb(1., 0.88, 0.48)),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(24),
            width: percent(100),
            ..default()
        },
        GlobalZIndex(10),
        Visibility::Hidden,
    ));
    commands
        .spawn((
            Root,
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            // The menu cameras clear the window to black behind the UI.
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                Panel,
                Node {
                    width: percent(70),
                    padding: UiRect::all(px(20)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(16),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.015, 0.035, 0.78)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Logo,
                    ImageNode::new(logo),
                    Node {
                        height: percent(22),
                        aspect_ratio: Some(1.),
                        flex_shrink: 0.,
                        ..default()
                    },
                ));
                panel
                    .spawn((
                        Body,
                        Node {
                            width: percent(100),
                            height: percent(62),
                            flex_shrink: 0.,
                            ..default()
                        },
                    ))
                    .with_children(|body| {
                        body.spawn((
                            lettering::Canvas,
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                height: percent(100),
                                ..default()
                            },
                        ));
                    });
                for (label, color) in [
                    (Label::Title, Color::srgb(0.4, 0.9, 1.)),
                    (Label::Body, Color::srgb(1., 0.88, 0.48)),
                    (Label::Footer, Color::srgb(0.8, 0.85, 0.95)),
                    (Label::Notice, Color::srgb(1., 0.6, 0.65)),
                ] {
                    panel.spawn((
                        label,
                        Text::default(),
                        TextFont::default(),
                        TextColor(color),
                        TextLayout::new_with_justify(Justify::Center),
                    ));
                }
            });
        });
}

fn draw_game_over_hint(
    front: Res<FrontEnd>,
    game: Res<Game>,
    store: Res<Store>,
    difficulty: Res<Difficulty>,
    mut hint: Single<(&mut Visibility, &mut Text), With<GameOverHint>>,
) {
    let (visible, text) = &mut *hint;
    **visible = if front.screen == Screen::Gameplay && game.status == Status::GameOver {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let value = if front.eligible && game.key_initials.is_some() {
        "ENTER / A  SAVE RESULTS"
    } else if front.eligible && store.data.rank(*difficulty, game.score).is_some() {
        "ENTER / A  ENTER HIGH-SCORE INITIALS"
    } else if game
        .key_level()
        .is_some_and(|level| front.eligible && store.data.key_qualifies(*difficulty, level))
    {
        "ENTER / A  SAVE A PLAYER KEY"
    } else {
        "ENTER / A  RETURN TO WELCOME"
    };
    if text.0 != value {
        text.0 = value.into();
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    front: Res<FrontEnd>,
    store: Res<Store>,
    difficulty: Res<Difficulty>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut root: Query<&mut Node, With<Root>>,
    mut panels: Query<(&mut Node, &mut BackgroundColor), (With<Panel>, Without<Root>)>,
    mut illustrated_nodes: Query<
        &mut Node,
        (
            Or<(With<Logo>, With<Body>, With<lettering::Canvas>)>,
            Without<Root>,
            Without<Panel>,
            Without<Label>,
        ),
    >,
    mut labels: Query<
        (&Label, &mut Text, &mut TextFont, &mut Node),
        (Without<Root>, Without<Panel>),
    >,
) {
    let screen = front.screen;
    let illustrated = matches!(screen, Screen::Welcome | Screen::Initials);
    let scale = windows.iter().next().map_or(1., |window| {
        (window.height() / 800.).min(window.width() / 1100.)
    });
    for mut node in &mut root {
        show(&mut node, screen != Screen::Gameplay);
    }
    if screen == Screen::Gameplay {
        return;
    }
    for (mut panel, mut background) in &mut panels {
        let layout = Node {
            width: percent(if screen == Screen::Welcome { 90 } else { 70 }),
            height: if illustrated { percent(86) } else { Val::Auto },
            padding: UiRect::all(px(20. * scale)),
            row_gap: px(16. * scale),
            ..panel.clone()
        };
        panel.set_if_neq(layout);
        background.set_if_neq(BackgroundColor(if screen == Screen::Welcome {
            Color::NONE
        } else {
            Color::srgba(0.01, 0.015, 0.035, 0.78)
        }));
    }
    for mut node in &mut illustrated_nodes {
        show(&mut node, illustrated);
    }
    let (title, body, footer) = match screen {
        Screen::Welcome => {
            let has_keys = front
                .available_keys(&store.data, *difficulty)
                .next()
                .is_some();
            let detail = if has_keys {
                "LEFT / RIGHT  PLAYER     "
            } else {
                ""
            };
            let select = "UP / DOWN  WEB     ";
            let exit = if cfg!(target_arch = "wasm32") {
                ""
            } else {
                "     ESC / B  EXIT"
            };
            (
                String::new(),
                String::new(),
                format!("{detail}{select}ENTER / A  PLAY\nP / START  CONFIGURATION{exit}"),
            )
        }
        Screen::Configuration => {
            let choices = [
                format!("DIFFICULTY     < {} >", difficulty.label()),
                #[cfg(not(target_arch = "wasm32"))]
                if front.install.is_some() {
                    "INSTALLING IN STEAM...".into()
                } else {
                    "INSTALL IN STEAM".into()
                },
                "BACK".into(),
            ];
            let body = choices
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    format!(
                        "{} {choice}",
                        if index == front.selected { ">" } else { " " }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            ("CONFIGURATION".into(), body,
                "UP / DOWN  SELECT     LEFT / RIGHT  DIFFICULTY\nENTER / A  CONFIRM     P / START / ESC / B  BACK".into())
        }
        Screen::Initials => (
            String::new(),
            String::new(),
            if cfg!(target_arch = "wasm32") {
                "LEFT / RIGHT  SLOT     UP / DOWN  LETTER\nENTER / A  SAVE     BACKSPACE / B  ERASE\nOR TYPE THREE LETTERS.".into()
            } else {
                "LEFT / RIGHT  SLOT     UP / DOWN  LETTER\nENTER / A  SAVE     BACKSPACE / B  ERASE\nOR TYPE THREE LETTERS. STEAM DECK: STEAM + X".into()
            },
        ),
        Screen::Gameplay => unreachable!(),
    };
    let notice = [&front.notice, &store.notice]
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    for (label, mut text, mut font, mut node) in &mut labels {
        let (value, size) = match label {
            Label::Title => (&title, 44.),
            Label::Body => (&body, 25.),
            Label::Footer => (&footer, 16.),
            Label::Notice => (&notice, 16.),
        };
        show(&mut node, !value.is_empty());
        if text.0 != *value {
            text.0.clone_from(value);
        }
        if font.font_size != size * scale {
            font.font_size = size * scale;
        }
    }
}

/// Layout changes re-run UI layout, so leave unchanged nodes untouched.
fn show(node: &mut Mut<Node>, visible: bool) {
    let display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
}

#[cfg(test)]
mod tests;
