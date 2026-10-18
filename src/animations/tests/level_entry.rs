//! Checks web entry transforms, contracting claw trails, control timing, and restart cancellation.

use crate::animations::level_entry::ClawTrail;
use crate::animations::level_entry::EntryAssets;
use crate::animations::level_entry::WebRoot;
use crate::animations::level_entry::web_transform;
use crate::animations::level_entry::{advance, draw, setup};
use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::flippers::spawn_enemies;
use crate::game::Game;
use crate::game::Status;
use crate::game::load_level;
use crate::game::playing;
use crate::glider::ACCELERATION;
use crate::glider::Glider;
use crate::glider::move_glider;
use crate::level::Level;
use crate::level::Levels;
use crate::powerups;
use crate::test_support::DEPTH;
use crate::test_support::START_PROGRESS;
use crate::test_support::camera_position;
use crate::test_support::glider_transform;
use crate::weapons::Bullet;
use crate::weapons::ShotAssets;
use crate::weapons::Weapon;
use crate::weapons::fire_bullets;
use crate::web::WORLD_UNIT;
use bevy::prelude::*;
use std::f32::consts::TAU;

fn app() -> App {
    let mut app = App::new();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    setup(
        &mut app.world_mut().commands(),
        &mut meshes,
        &mut materials,
        &Level::default().web,
    );
    app.world_mut().flush();
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::KeyA);
    keys.press(KeyCode::KeyZ);
    app.insert_resource(Game {
        status: Status::Entering,
        entry: Some(0),
        ..default()
    })
    .insert_resource(keys)
    .init_resource::<Level>()
    .init_resource::<Levels>()
    .insert_resource(meshes)
    .insert_resource(materials)
    .init_resource::<powerups::Upgrades>()
    .init_resource::<crate::difficulty::EnemyClock>()
    .init_resource::<ShotAssets>()
    .init_resource::<FlipperAssets>()
    .add_systems(
        FixedUpdate,
        (
            move_glider.run_if(playing),
            fire_bullets.run_if(playing),
            spawn_enemies,
            advance,
        )
            .chain(),
    )
    .add_systems(Update, (load_level, draw).chain());
    app.world_mut().spawn((
        Glider {
            progress: START_PROGRESS,
            velocity: 0,
        },
        Weapon::default(),
        glider_transform(START_PROGRESS),
        Visibility::Hidden,
    ));
    for _ in 0..2 {
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_translation(camera_position(
                glider_transform(START_PROGRESS).translation,
            )),
        ));
    }
    app.world_mut().run_schedule(Update);
    app
}

fn ticks(app: &mut App, count: usize) {
    for _ in 0..count {
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(Update);
    }
}

#[test]
fn web_approaches_and_rotates_about_its_center_without_moving_the_camera() {
    let mut app = app();
    let center = Vec3::NEG_Z * (DEPTH * 0.5);
    for tick in [0u16, 64, 128, 192, 256] {
        if tick > 0 {
            ticks(&mut app, 64);
        }
        let web = app
            .world_mut()
            .query_filtered::<&Transform, With<WebRoot>>()
            .single(app.world())
            .unwrap();
        assert!(
            (web.translation + web.rotation * center)
                .distance(center - Vec3::Z * f32::from(256 - tick) * WORLD_UNIT)
                < 1e-4
        );
        if tick == 128 {
            let expected =
                Quat::from_rotation_x(3.0 * TAU / 4.0) * Quat::from_rotation_y(TAU / 2.0);
            assert!(web.rotation.dot(expected).abs() > 1.0 - 1e-6);
        }
        for camera in app
            .world_mut()
            .query_filtered::<&Transform, With<Camera3d>>()
            .iter(app.world())
        {
            assert_eq!(
                camera.translation,
                camera_position(glider_transform(START_PROGRESS).translation)
            );
        }
    }
    let web = app
        .world_mut()
        .query_filtered::<&Transform, With<WebRoot>>()
        .single(app.world())
        .unwrap();
    assert_eq!(*web, Transform::default());
    assert_eq!(app.world().resource::<Game>().remaining, 25);
    assert_eq!(
        app.world_mut().query::<&Bullet>().iter(app.world()).count(),
        0
    );
}

#[test]
fn four_wire_claws_contract_while_controls_and_the_wave_resume() {
    let mut app = app();
    ticks(&mut app, 256);
    assert!(app.world().resource::<Game>().status == Status::Playing);
    assert_eq!(app.world().resource::<Game>().spawn_timer, 60);
    ticks(&mut app, 1);
    let (glider, transform, visibility) = app
        .world_mut()
        .query::<(&Glider, &Transform, &Visibility)>()
        .single(app.world())
        .unwrap();
    assert_eq!(glider.velocity, ACCELERATION);
    assert_eq!(*visibility, Visibility::Hidden);
    let position = transform.translation;
    for (trail, transform, visibility) in app
        .world_mut()
        .query::<(&ClawTrail, &Transform, &Visibility)>()
        .iter(app.world())
    {
        assert_eq!(*visibility, Visibility::Visible);
        assert!(
            (transform.translation.z - position.z - trail.0 as f32 * 15.75 * WORLD_UNIT).abs()
                < 1e-5
        );
    }
    assert_eq!(
        app.world_mut().query::<&Bullet>().iter(app.world()).count(),
        1
    );
    ticks(&mut app, 30);
    let assets = app.world().resource::<EntryAssets>();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&assets.surface)
            .unwrap()
            .base_color
            .alpha(),
        1.0
    );
    ticks(&mut app, 30);
    assert_eq!(app.world().resource::<Game>().remaining, 24);
    ticks(&mut app, 3);
    assert!(app.world().resource::<Game>().entry.is_none());
    assert_eq!(
        *app.world_mut()
            .query_filtered::<&Visibility, With<Glider>>()
            .single(app.world())
            .unwrap(),
        Visibility::Visible
    );
    assert!(
        app.world_mut()
            .query_filtered::<&Visibility, With<ClawTrail>>()
            .iter(app.world())
            .all(|visibility| *visibility == Visibility::Hidden)
    );
}

#[test]
fn restart_rewinds_the_approach_and_hides_the_claw() {
    let mut app = app();
    ticks(&mut app, 300);
    app.world_mut().resource_mut::<Game>().load = Some(crate::level::LoadReason::Restart);
    app.world_mut().run_schedule(Update);
    assert!(app.world().resource::<Game>().status == Status::Entering);
    assert_eq!(app.world().resource::<Game>().entry, Some(0));
    assert_eq!(
        *app.world_mut()
            .query_filtered::<&Transform, With<WebRoot>>()
            .single(app.world())
            .unwrap(),
        web_transform(0, &Level::default().web)
    );
    assert_eq!(
        *app.world_mut()
            .query_filtered::<&Visibility, With<Glider>>()
            .single(app.world())
            .unwrap(),
        Visibility::Hidden
    );
    assert!(
        app.world_mut()
            .query_filtered::<&Visibility, With<ClawTrail>>()
            .iter(app.world())
            .all(|visibility| *visibility == Visibility::Hidden)
    );
}
