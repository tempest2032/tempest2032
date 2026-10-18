//! Bevy app startup and the ordered input, simulation, rendering, and game-flow
//! schedules. Keeps fixed-tick system ordering and run conditions together.

use crate::enemies::{pulsar_render, pulsars};
#[cfg(not(target_arch = "wasm32"))]
use bevy::diagnostic::SystemInformationDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowMode};

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    capture::{
        CapturePath, CapturePreview, MotionCapture, capture_frame, capture_motion, preview_input,
        preview_transition,
    },
    frame_pacing::warn_late_frames,
};

use crate::{
    animations::{
        flipper_capture::animate_capture, glider_death as shot_death, level_entry as entry,
        level_exit as exit,
    },
    camera::{fit_camera, follow_glider},
    cli,
    collisions::resolve_hits,
    enemies::{
        aliens, beasts,
        flipper_render::{draw_approaching_flippers, update_enemy_colors},
        flippers::{move_enemies, spawn_enemies},
        fuseballs, mirrors,
        shots::{fire_enemy_shots, move_enemy_shots},
        spikers, tankers,
    },
    explosion,
    game::{Game, Status, check_game, controlling, load_level},
    glider::{animate_glider, move_glider},
    hud::{FpsDisplay, FpsMeter, spawn_fps_label, toggle_fps, update_fps, update_hud},
    level::{Level, Levels},
    powerups,
    scene::{setup, sync_web, update_lane},
    stars, warp,
    weapons::{draw_particle_shots, fire_bullets, move_bullets},
};

#[derive(Resource)]
struct StartingLevel(usize);

#[derive(Resource)]
struct StartingWarp {
    kind: warp::ZoneKind,
    course: Option<u8>,
    gate: Option<u8>,
}

fn normal_scene(game: Res<Game>) -> bool {
    game.status != Status::Bonus
}

#[cfg(not(target_arch = "wasm32"))]
fn toggle_fullscreen(
    mut input: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    front: Option<Res<crate::menu::FrontEnd>>,
    mut window: Single<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
) {
    for event in input.read() {
        if event.key_code != KeyCode::KeyF
            || event.state != bevy::input::ButtonState::Pressed
            || event.repeat
            || event.window != window.0
            || front
                .as_ref()
                .is_some_and(|front| front.screen == crate::menu::Screen::Initials)
            || keys.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::AltLeft,
                KeyCode::AltRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ])
        {
            continue;
        }
        window.1.mode = if window.1.mode == WindowMode::Windowed {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        keys.reset(KeyCode::KeyF);
    }
}

pub(crate) fn run(args: cli::Cli) {
    if let Err(error) = args.validate() {
        error.exit();
    }
    let mut app = App::new();
    let antialiasing = args
        .antialiasing
        .unwrap_or_else(crate::antialiasing::detected_start);
    crate::antialiasing::add_systems(&mut app, antialiasing, args.antialiasing.is_none());
    let direct = args.direct_start();
    let mut store = crate::scores::Store::load();
    let difficulty = args.difficulty.unwrap_or(store.data.difficulty);
    if let Some(pattern) = args.starfield {
        app.insert_resource(stars::Animation::new(pattern));
    }
    crate::messages::add_systems(&mut app);
    if let Some(id) = args.message_preview {
        app.insert_resource(crate::messages::Preview {
            id,
            capture_tick: args.message_tick.or(args.screenshot.as_ref().map(|_| 60)),
            elapsed: 0,
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(SystemInformationDiagnosticsPlugin)
        .add_systems(
            Update,
            warn_late_frames.run_if(|display: Res<FpsDisplay>| display.0),
        );
    app.insert_resource(FpsDisplay(args.fps))
        .init_resource::<FpsMeter>()
        .add_systems(Startup, spawn_fps_label)
        .add_systems(
            PreUpdate,
            toggle_fps
                .after(bevy::input::InputSystems)
                .before(crate::menu::input)
                .before(crate::pause::input),
        )
        .add_systems(
            Update,
            update_fps.run_if(|display: Res<FpsDisplay>| display.0),
        );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        PreUpdate,
        toggle_fullscreen
            .after(bevy::input::InputSystems)
            .before(crate::menu::input)
            .before(crate::pause::input),
    );
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = args.screenshot {
        app.insert_resource(CapturePath(path))
            .add_systems(Update, capture_frame);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let recording = args
        .record_motion
        .map(|directory| {
            (
                directory,
                if args.warp_zone.is_some() { 30 } else { 6 },
                CapturePreview::Motion,
            )
        })
        .or_else(|| {
            args.record_exit
                .map(|directory| (directory, 14, CapturePreview::Exit))
        })
        .or_else(|| {
            args.record_shot_hit
                .map(|directory| (directory, 15, CapturePreview::ShotHit))
        });
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((directory, default_seconds, preview)) = recording {
        std::fs::create_dir_all(&directory).expect("could not create motion capture directory");
        app.insert_resource(MotionCapture {
            directory,
            tick: 0,
            next_capture: 0,
            frame: 0,
            finished: false,
            end_tick: args.seconds.unwrap_or(default_seconds) * 60,
            preview,
        })
        .add_systems(FixedUpdate, preview_input.before(move_glider))
        .add_systems(Update, capture_motion);
    }
    let starting_level = args.level.unwrap_or(1) as usize;
    let mut levels = match Levels::load(&args.levels) {
        Ok(levels) => levels,
        Err(error) => {
            eprintln!("Could not load levels: {error}");
            std::process::exit(1);
        }
    };
    // Adjust the parsed catalog, never the TOML, so the shipped source values
    // stay intact and every later level swap inherits the chosen difficulty.
    let eligible = !direct && levels.is_standard();
    let mut front = crate::menu::FrontEnd::new(
        if let Some(preview) = args.menu_preview {
            match preview {
                crate::menu::Preview::Welcome | crate::menu::Preview::WelcomeKey => {
                    crate::menu::Screen::Welcome
                }
                crate::menu::Preview::Configuration => crate::menu::Screen::Configuration,
                crate::menu::Preview::Initials | crate::menu::Preview::KeyInitials => {
                    crate::menu::Screen::Initials
                }
            }
        } else if direct {
            crate::menu::Screen::Gameplay
        } else {
            crate::menu::Screen::Welcome
        },
        eligible,
        levels.0.clone(),
    );
    match args.menu_preview {
        Some(crate::menu::Preview::Initials) => front.preview_initials(),
        Some(crate::menu::Preview::KeyInitials) => front.preview_key_initials(),
        Some(crate::menu::Preview::WelcomeKey) => front.preview_key_start(&mut store, difficulty),
        _ => {}
    }
    app.insert_resource(front).insert_resource(store);
    for level in &mut levels.0 {
        level.apply_difficulty(difficulty);
    }
    app.insert_resource(difficulty);
    let initial = levels
        .0
        .get(starting_level - 1)
        .unwrap_or_else(|| {
            eprintln!(
                "Level {starting_level} does not exist; choose 1 through {}",
                levels.0.len()
            );
            std::process::exit(1);
        })
        .clone();
    app.insert_resource(initial)
        .insert_resource(levels)
        .insert_resource(StartingLevel(starting_level - 1));
    if let Some(kind) = args.warp_zone {
        app.insert_resource(StartingWarp {
            kind,
            course: args.warp_course,
            gate: args.warp_gate,
        });
    }
    app.insert_resource(ClearColor(Color::BLACK))
        // All game materials are unlit; PBR still allocates fallback shadow maps.
        .insert_resource(bevy::light::PointLightShadowMap { size: 1 })
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 1 })
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                #[cfg(target_arch = "wasm32")]
                canvas: Some("#game".into()),
                #[cfg(target_arch = "wasm32")]
                fit_canvas_to_parent: true,
                title: format!("Tempest 2032 | Level {starting_level}"),
                resolution: args.message_size.map_or_else(
                    || (1100, 850).into(),
                    |size| {
                        bevy::window::WindowResolution::new(size.x, size.y)
                            .with_scale_factor_override(1.)
                    },
                ),
                present_mode: PresentMode::Fifo,
                mode: if args.fullscreen {
                    // A new window has no monitor yet, so this must not use Current.
                    WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                } else {
                    WindowMode::Windowed
                },
                ..default()
            }),
            ..default()
        }))
        .add_systems(
            Startup,
            (
                setup,
                warp::render::setup,
                crate::hud::setup,
                powerups::superzapper::setup,
            ),
        );
    add_systems(&mut app);
    // Per-pass GPU times let automatic antialiasing tell GPU load from other slowdowns.
    #[cfg(not(target_arch = "wasm32"))]
    if args.antialiasing.is_none() {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    app.insert_resource(bevy::audio::GlobalVolume::new(bevy::audio::Volume::Linear(
        args.volume,
    )));
    crate::sound::add_systems(&mut app);
    crate::music::add_systems(&mut app);
    app.insert_resource(crate::music::MusicVolume(args.music_volume));
    crate::pause::add_systems(&mut app);
    crate::gamepad::add_systems(&mut app);
    crate::menu::add_systems(&mut app);
    #[cfg(target_arch = "wasm32")]
    crate::browser::add_systems(&mut app);
    stars::add_systems(&mut app);
    app.run();
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "tests/window.rs"]
mod tests;

fn add_systems(app: &mut App) {
    let initial = app
        .world()
        .get_resource::<Level>()
        .cloned()
        .unwrap_or_default();
    let index = app
        .world()
        .get_resource::<StartingLevel>()
        .map_or(0, |start| start.0);
    let start_warp =
        app.world()
            .get_resource::<StartingWarp>()
            .map(|start| crate::level::LoadReason::Warp {
                kind: start.kind,
                course: start.course,
                gate: start.gate,
                destination: index,
            });
    let mut game = Game {
        load: start_warp,
        deferred_start_bonus: start_warp
            .is_none()
            .then(|| Game::starting_bonus(initial.number)),
        level_index: index,
        ..default()
    };
    crate::game::reset_wave(&mut game, &initial);
    app.init_resource::<Level>()
        .init_resource::<Levels>()
        .insert_resource(game)
        .init_resource::<powerups::Upgrades>()
        .init_resource::<pulsars::Pulse>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .add_systems(
            PreUpdate,
            (powerups::queue_jump, powerups::superzapper::queue)
                .after(bevy::input::InputSystems)
                .run_if(normal_scene)
                .run_if(crate::pause::running)
                .run_if(crate::menu::playing),
        )
        .add_systems(
            FixedUpdate,
            (
                (
                    crate::difficulty::advance_enemy_clock,
                    move_glider.run_if(controlling),
                    powerups::update_jump,
                    animate_glider,
                    update_lane,
                    follow_glider.run_if(|game: Res<Game>| {
                        !matches!(
                            game.status,
                            Status::Entering
                                | Status::Captured
                                | Status::ShotHit
                                | Status::Spiked
                                | Status::Fried
                                | Status::Exiting
                                | Status::Bonus
                        )
                    }),
                    crate::game::award_start_bonus,
                    crate::scene::animate_surface,
                    move_bullets,
                    fire_bullets.run_if(controlling),
                    pulsars::pulse,
                    spawn_enemies,
                    tankers::spawn,
                    spikers::spawn,
                    beasts::spawn,
                    fuseballs::spawn,
                    pulsars::spawn,
                    mirrors::spawn,
                    aliens::spawn,
                )
                    .chain(),
                (
                    powerups::superzapper::advance,
                    move_enemies,
                    mirrors::advance,
                    aliens::advance,
                    fuseballs::advance,
                    move_enemy_shots,
                    pulsars::check_sparks,
                    pulsars::advance,
                    resolve_hits,
                    pulsars::check_player,
                    aliens::check_player,
                    tankers::move_tankers,
                    spikers::advance,
                    fire_enemy_shots,
                    powerups::advance_powerups,
                    powerups::move_droid,
                )
                    .chain(),
                (
                    explosion::age_explosions,
                    entry::advance,
                    animate_capture,
                    shot_death::animate,
                    exit::animate,
                    warp::advance,
                    check_game,
                    load_level,
                )
                    .chain(),
            )
                .chain()
                .run_if(crate::messages::running),
        )
        .add_systems(
            Update,
            (fit_camera.run_if(normal_scene), load_level, update_hud).chain(),
        )
        .add_systems(Update, sync_web.after(load_level))
        .add_systems(Update, warp::failure::draw.after(warp::render::draw))
        .add_systems(Update, warp::victory::draw.after(warp::render::draw))
        .add_systems(
            Update,
            warp::render::draw
                .after(load_level)
                .after(fit_camera)
                .after(sync_web),
        )
        .add_systems(Update, update_enemy_colors.after(load_level))
        .add_systems(Update, draw_approaching_flippers.after(fit_camera))
        .add_systems(Update, tankers::draw_points.after(fit_camera))
        .add_systems(Update, fuseballs::draw_points.after(fit_camera))
        .add_systems(Update, mirrors::draw_points.after(fit_camera))
        .add_systems(
            Update,
            (aliens::draw_points, aliens::draw_beams).after(fit_camera),
        )
        .add_systems(Update, draw_particle_shots.after(fit_camera))
        .add_systems(
            Update,
            powerups::superzapper::draw
                .after(fit_camera)
                .after(load_level),
        )
        .add_systems(
            Update,
            pulsar_render::draw.after(fit_camera).after(sync_web),
        )
        .add_systems(
            Update,
            (
                powerups::draw_powerups.after(fit_camera),
                entry::draw.after(load_level),
                exit::draw.after(load_level),
                shot_death::draw.after(load_level),
                spikers::draw.after(load_level),
            ),
        );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        FixedUpdate,
        preview_transition.after(preview_input).before(move_glider),
    );
}
