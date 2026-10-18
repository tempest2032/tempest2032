//! Checks the frame-rate label samples at a steady pace, keeps a two-second
//! window of per-frame times, and appends the processor and memory readings
//! once they arrive.

use crate::hud::{FpsDisplay, FpsLabel, FpsMeter, spawn_fps_label, update_fps};
use bevy::diagnostic::{
    Diagnostic, DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore,
    SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use std::time::{Duration, Instant};

fn vector_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::messages::geometry::Glyphs>()
        .init_resource::<crate::game::Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<crate::powerups::Upgrades>()
        .init_resource::<Time<Virtual>>()
        .add_systems(Startup, crate::hud::setup)
        .add_systems(Update, crate::hud::update_hud);
    app.world_mut()
        .spawn((Window::default(), bevy::window::PrimaryWindow));
    app.update();
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(250));
    app
}

fn hud_vertices(app: &App) -> usize {
    app.world()
        .resource::<Assets<Mesh>>()
        .get(&app.world().resource::<super::HudAssets>().mesh)
        .unwrap()
        .count_vertices()
}

#[test]
fn icons_fade_in_and_out_and_reverse_without_popping() {
    use bevy::mesh::VertexAttributeValues;
    let mut app = vector_app();
    app.world_mut()
        .resource_mut::<crate::powerups::Upgrades>()
        .seed_warp_tokens(1);
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(100));
    app.update();
    let icons = app.world().resource::<super::HudAssets>().icons;
    assert!((icons[8] - 0.4).abs() < 1e-6);
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&app.world().resource::<super::HudAssets>().mesh)
        .unwrap();
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("HUD must contain vertex colors")
    };
    assert!(colors.iter().any(|c| c[3] > 0. && c[3] < 1.));
    app.world_mut()
        .resource_mut::<crate::powerups::Upgrades>()
        .seed_warp_tokens(0);
    app.world_mut().resource_mut::<crate::game::Game>().lives = 1;
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(50));
    app.update();
    let icons = app.world().resource::<super::HudAssets>().icons;
    assert!((icons[8] - 0.2).abs() < 1e-6);
    assert!((icons[0] - 0.8).abs() < 1e-6);
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::ZERO);
    app.update();
    assert_eq!(app.world().resource::<super::HudAssets>().icons, icons);
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(250));
    app.update();
    assert_eq!(app.world().resource::<super::HudAssets>().icons, [0.; 11]);
}

#[test]
fn vector_hud_tracks_lives_and_warp_tokens_without_text_or_new_assets() {
    let mut app = vector_app();
    let initial = hud_vertices(&app);
    assert!(initial > 0);
    assert_eq!(
        app.world_mut().query::<&Text>().iter(app.world()).count(),
        0
    );
    app.world_mut()
        .resource_mut::<crate::powerups::Upgrades>()
        .seed_warp_tokens(3);
    app.update();
    assert_eq!(hud_vertices(&app), initial + 27);
    app.world_mut().resource_mut::<crate::game::Game>().lives = 1;
    app.update();
    let last_life = hud_vertices(&app);
    let life_vertices = app.world().resource::<super::HudAssets>().life.len() * 3;
    assert_eq!(initial + 27 - last_life, 3 * life_vertices);
    app.world_mut().resource_mut::<crate::game::Game>().lives = 2;
    app.update();
    assert_eq!(hud_vertices(&app), last_life + life_vertices);
    app.world_mut().resource_mut::<crate::game::Game>().lives = 0;
    app.update();
    assert_eq!(hud_vertices(&app), last_life);
    app.world_mut().resource_mut::<crate::game::Game>().score = 1234567890;
    app.update();
    assert!(hud_vertices(&app) > initial);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
}

#[test]
fn vector_hud_refits_on_resize_and_updates_when_paused() {
    use bevy::mesh::VertexAttributeValues;
    let mut app = vector_app();
    app.world_mut()
        .query::<&mut Window>()
        .single_mut(app.world_mut())
        .unwrap()
        .resolution
        .set(600., 1200.);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&app.world().resource::<super::HudAssets>().mesh)
        .unwrap();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("HUD must contain vector positions")
    };
    assert!(
        positions
            .iter()
            .all(|p| p[0].abs() <= 60. && p[1].abs() <= 120.)
    );
    assert!(positions.iter().any(|p| p[2] < 20.));
    assert!(positions.iter().any(|p| p[2] > 20.));
    let paused_positions = positions.clone();
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    app.update();
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&app.world().resource::<super::HudAssets>().mesh)
        .unwrap();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!()
    };
    assert_eq!(*positions, paused_positions);
}

fn fps_app() -> App {
    let mut app = App::new();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
        16_666_667,
    )))
    .add_plugins(TimePlugin)
    .insert_resource(FpsDisplay(true))
    .init_resource::<FpsMeter>()
    .init_resource::<crate::antialiasing::Antialiasing>()
    .init_resource::<DiagnosticsStore>()
    .add_systems(Startup, spawn_fps_label)
    .add_systems(Update, update_fps);
    let mut store = app.world_mut().resource_mut::<DiagnosticsStore>();
    for path in [
        SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
        SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
    ] {
        store.add(Diagnostic::new(path));
    }
    app.update();
    app
}

fn run(app: &mut App, frame: Duration, count: u32) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(frame));
    for _ in 0..count {
        app.update();
    }
}

fn report(app: &mut App, path: &DiagnosticPath, readings: &[f64]) {
    let mut store = app.world_mut().resource_mut::<DiagnosticsStore>();
    let diagnostic = store.get_mut(path).unwrap();
    for reading in readings {
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: Instant::now(),
            value: *reading,
        });
    }
}

fn label(app: &mut App) -> String {
    app.world_mut()
        .query_filtered::<&Text, With<FpsLabel>>()
        .single(app.world())
        .unwrap()
        .0
        .clone()
}

#[test]
fn the_label_waits_for_a_half_second_sample_and_then_reports_the_frame_times() {
    let mut app = fps_app();
    run(&mut app, Duration::from_nanos(16_666_667), 10);
    assert_eq!(label(&mut app), "");
    run(&mut app, Duration::from_nanos(16_666_667), 20);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full");
}

#[test]
fn one_slow_frame_shows_up_in_the_range_at_the_next_refresh() {
    let mut app = fps_app();
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full");
    run(&mut app, Duration::from_millis(100), 1);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full");
    run(&mut app, Duration::from_nanos(16_666_667), 25);
    assert_eq!(label(&mut app), "50 FPS  16.7-100.0 ms  AA full");
}

#[test]
fn the_frame_times_span_the_last_two_seconds_and_forget_older_frames() {
    let mut app = fps_app();
    run(&mut app, Duration::from_nanos(10_000_000), 60);
    assert_eq!(label(&mut app), "100 FPS  10.0-10.0 ms  AA full");
    run(&mut app, Duration::from_nanos(33_333_333), 50);
    assert_eq!(label(&mut app), "30 FPS  10.0-33.3 ms  AA full");
    run(&mut app, Duration::from_nanos(33_333_333), 50);
    assert_eq!(label(&mut app), "30 FPS  33.3-33.3 ms  AA full");
}

#[test]
fn the_processor_and_memory_readings_are_added_once_the_diagnostics_have_values() {
    let mut app = fps_app();
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full");
    report(
        &mut app,
        &SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
        &[10.0, 30.0],
    );
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full  CPU 20%");
    report(
        &mut app,
        &SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
        &[1.24, 1.28],
    );
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(
        label(&mut app),
        "60 FPS  16.7-16.7 ms  AA full  CPU 20%  MEM 1290 MiB"
    );
}

#[test]
fn the_antialiasing_state_follows_the_frame_rate_fallback() {
    let mut app = fps_app();
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA full");
    app.world_mut()
        .resource_mut::<crate::antialiasing::Antialiasing>()
        .quality = crate::antialiasing::Quality::Off;
    run(&mut app, Duration::from_nanos(16_666_667), 30);
    assert_eq!(label(&mut app), "60 FPS  16.7-16.7 ms  AA off");
}

#[test]
fn refresh_deadlines_keep_the_remainder_from_previous_frames() {
    let mut app = fps_app();
    run(&mut app, Duration::from_millis(300), 2);
    assert_eq!(label(&mut app), "3 FPS  300.0-300.0 ms  AA full");
    run(&mut app, Duration::from_millis(200), 1);
    assert_eq!(label(&mut app), "3 FPS  300.0-300.0 ms  AA full");
    run(&mut app, Duration::from_millis(200), 1);
    assert_eq!(label(&mut app), "5 FPS  200.0-300.0 ms  AA full");
}

#[test]
fn the_rolling_window_keeps_its_precision_after_a_long_session() {
    let mut app = fps_app();
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs(7 * 24 * 60 * 60));
    run(&mut app, Duration::from_millis(100), 1);
    run(&mut app, Duration::from_millis(10), 40);
    assert_eq!(label(&mut app), "82 FPS  10.0-100.0 ms  AA full");
    run(&mut app, Duration::from_millis(10), 160);
    assert_eq!(label(&mut app), "100 FPS  10.0-100.0 ms  AA full");
    run(&mut app, Duration::from_millis(10), 50);
    assert_eq!(label(&mut app), "100 FPS  10.0-10.0 ms  AA full");
}
