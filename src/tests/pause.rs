//! Pause input, full fixed-loop suspension, virtual clocks, and resume UI.

use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

#[derive(Resource, Default)]
struct Ticks(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
            16_666_667,
        )))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::game::Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<crate::powerups::Upgrades>()
        .init_resource::<Ticks>()
        .add_systems(FixedUpdate, |mut ticks: ResMut<Ticks>| ticks.0 += 1);
    add_systems(&mut app);
    app.update();
    app
}

fn key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(key);
}

#[test]
fn pointer_pauses_and_clicks_each_choice_without_keyboard_navigation() {
    let mut app = app();
    app.init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(crate::menu::FrontEnd::new(
            crate::menu::Screen::Gameplay,
            true,
            Vec::new(),
        ))
        .init_resource::<crate::messages::Messages>();
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (960, 720).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ))
        .id();
    let click = |app: &mut App, point: Vec2| {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
    };
    click(&mut app, Vec2::new(10., 10.));
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    click(&mut app, Vec2::new(10., 10.));
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    for row in 0..3 {
        if row > 0 {
            click(&mut app, Vec2::new(10., 10.));
        }
        let (center, _) = choice_layout(row, 320.);
        click(&mut app, Vec2::new(480., 360. - center * 3.));
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        if row == 1 {
            assert!(matches!(
                app.world().resource::<crate::game::Game>().load,
                Some(crate::level::LoadReason::Restart)
            ));
            app.world_mut().resource_mut::<crate::game::Game>().load = None;
        }
    }
    assert_eq!(
        app.world().resource::<crate::menu::FrontEnd>().screen,
        crate::menu::Screen::Welcome
    );
}

#[test]
fn pointer_targets_follow_pause_lettering_in_portrait_and_landscape() {
    for size in [
        Vec2::new(320., 450.),
        Vec2::new(850., 350.),
        Vec2::new(1280., 800.),
    ] {
        let viewport = size.x * 240. / size.y;
        for row in 0..3 {
            let (center, _) = choice_layout(row, viewport);
            let point = size / 2. - Vec2::Y * center * size.y / 240.;
            assert_eq!(choice_at(point, size), Some(row));
            assert_eq!(choice_at(Vec2::new(0., point.y), size), None);
        }
        assert_eq!(choice_at(Vec2::ZERO, size), None);
    }
}

#[test]
fn pause_stops_fixed_updates_and_resumes_without_catching_up() {
    let mut app = app();
    for _ in 0..10 {
        app.update();
    }
    let ticks = app.world().resource::<Ticks>().0;
    key(&mut app, KeyCode::KeyP);
    assert_eq!(app.world().resource::<Ticks>().0, ticks);
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    for _ in 0..600 {
        app.update();
    }
    assert_eq!(app.world().resource::<Ticks>().0, ticks);
    assert_eq!(app.world().resource::<Time<Virtual>>().elapsed(), elapsed);
    key(&mut app, KeyCode::Pause);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(app.world().resource::<Ticks>().0, ticks + 10);
}

#[test]
fn front_end_stops_gameplay_ticks_but_keeps_virtual_time_for_menu_music() {
    let mut app = app();
    app.insert_resource(crate::menu::FrontEnd::new(
        crate::menu::Screen::Welcome,
        true,
        Vec::new(),
    ));
    let ticks = app.world().resource::<Ticks>().0;
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(app.world().resource::<Ticks>().0, ticks);
    assert!(app.world().resource::<Time<Virtual>>().elapsed() > elapsed);
    app.world_mut()
        .resource_mut::<crate::menu::FrontEnd>()
        .screen = crate::menu::Screen::Gameplay;
    app.update();
    assert_eq!(app.world().resource::<Ticks>().0, ticks + 1);
}

#[test]
fn pause_defaults_to_resume_and_confirm_resumes() {
    let mut app = app();
    key(&mut app, KeyCode::KeyP);
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    for _ in 0..10 {
        app.update();
    }
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(
        app.world_mut().query::<&Text>().iter(app.world()).count(),
        0
    );
    assert_eq!(
        app.world_mut().query::<&Button>().iter(app.world()).count(),
        0
    );
    assert_eq!(app.world().resource::<Menu>().selected, 0);
    key(&mut app, KeyCode::Enter);
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(
        app.world_mut().query::<&Node>().iter(app.world()).count(),
        0
    );
}

#[test]
fn restart_resumes_and_discarded_jump_inputs_do_not_reappear() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<crate::powerups::Upgrades>()
        .jump_queued = true;
    app.world_mut()
        .resource_mut::<crate::powerups::Upgrades>()
        .zapper
        .queued = true;
    key(&mut app, KeyCode::KeyP);
    assert!(
        !app.world()
            .resource::<crate::powerups::Upgrades>()
            .jump_queued
    );
    assert!(
        !app.world()
            .resource::<crate::powerups::Upgrades>()
            .zapper
            .queued
    );
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<Menu>().selected, 1);
    let ticks = app.world().resource::<Ticks>().0;
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Ticks>().0, ticks);
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert!(matches!(
        app.world().resource::<crate::game::Game>().load,
        Some(crate::level::LoadReason::Restart)
    ));
}

#[test]
fn menu_wraps_exits_and_resets_on_reopening() {
    let mut app = app();
    app.insert_resource(crate::menu::FrontEnd::new(
        crate::menu::Screen::Gameplay,
        true,
        Vec::new(),
    ))
    .insert_resource(crate::scores::Store::open(None))
    .init_resource::<crate::difficulty::Difficulty>()
    .init_resource::<crate::messages::Messages>();
    app.world_mut().resource_mut::<crate::game::Game>().score = 1234;
    key(&mut app, KeyCode::KeyP);
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(app.world().resource::<Menu>().selected, 2);
    key(&mut app, KeyCode::KeyP);
    key(&mut app, KeyCode::KeyP);
    assert_eq!(app.world().resource::<Menu>().selected, 0);
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::Space);
    assert_eq!(
        app.world().resource::<crate::menu::FrontEnd>().screen,
        crate::menu::Screen::Welcome
    );
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(app.world().resource::<crate::game::Game>().score, 0);
    assert!(app.world().resource::<crate::game::Game>().load.is_none());
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
}

#[test]
fn restart_key_no_longer_restarts_or_resumes() {
    let mut app = app();
    key(&mut app, KeyCode::KeyR);
    assert!(app.world().resource::<crate::game::Game>().load.is_none());
    key(&mut app, KeyCode::KeyP);
    key(&mut app, KeyCode::KeyR);
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    assert!(app.world().resource::<crate::game::Game>().load.is_none());
}

#[test]
fn game_over_accepts_any_fresh_key_including_pause_and_escape() {
    for button in [
        KeyCode::KeyQ,
        KeyCode::Enter,
        KeyCode::KeyP,
        KeyCode::Escape,
    ] {
        let mut app = app();
        app.world_mut().resource_mut::<crate::game::Game>().status = crate::game::Status::GameOver;
        key(&mut app, button);
        assert!(matches!(
            app.world().resource::<crate::game::Game>().load,
            Some(crate::level::LoadReason::Restart)
        ));
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .get_just_pressed()
                .next()
                .is_none()
        );
    }
}

#[test]
fn held_buttons_do_not_dismiss_game_over_but_unmapped_controller_buttons_do() {
    let mut app = app();
    key(&mut app, KeyCode::Space);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut().resource_mut::<crate::game::Game>().status = crate::game::Status::GameOver;
    app.update();
    assert!(app.world().resource::<crate::game::Game>().load.is_none());
    let mut pad = Gamepad::default();
    pad.digital_mut()
        .press(bevy::input::gamepad::GamepadButton::North);
    app.world_mut().spawn(pad);
    app.update();
    assert!(matches!(
        app.world().resource::<crate::game::Game>().load,
        Some(crate::level::LoadReason::Restart)
    ));
}

#[test]
fn controller_can_pause_choose_restart_and_confirm() {
    use bevy::input::gamepad::GamepadButton;
    let mut app = app();
    crate::gamepad::add_systems(&mut app);
    let pad = app.world_mut().spawn(Gamepad::default()).id();
    for button in [
        GamepadButton::Start,
        GamepadButton::DPadDown,
        GamepadButton::South,
    ] {
        app.world_mut()
            .get_mut::<Gamepad>(pad)
            .unwrap()
            .digital_mut()
            .press(button);
        app.update();
        app.world_mut()
            .get_mut::<Gamepad>(pad)
            .unwrap()
            .digital_mut()
            .reset(button);
        app.update();
    }
    assert!(matches!(
        app.world().resource::<crate::game::Game>().load,
        Some(crate::level::LoadReason::Restart)
    ));
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
}

#[test]
fn pending_audio_is_suspended_without_replacing_its_settings() {
    let mut app = app();
    let audio = app
        .world_mut()
        .spawn(PlaybackSettings::ONCE.with_speed(1.5))
        .id();
    key(&mut app, KeyCode::KeyP);
    let settings = app.world().get::<PlaybackSettings>(audio).unwrap();
    assert!(settings.paused);
    assert_eq!(settings.speed, 1.5);
    key(&mut app, KeyCode::KeyP);
    assert!(!app.world().get::<PlaybackSettings>(audio).unwrap().paused);
}
