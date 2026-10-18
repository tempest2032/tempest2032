//! Checks death fragments, camera retreat, control lockout, final-life timing, and restart cancellation.

use crate::animations::glider_death::ShotBurst;
use crate::animations::glider_death::{animate, draw, setup};
use crate::animations::level_entry as entry;
use crate::cry;
use crate::enemies::shots::EnemyShot;
use crate::enemies::shots::move_enemy_shots;
use crate::explosion;
use crate::game::Game;
use crate::game::Status;
use crate::game::check_game;
use crate::game::controlling;
use crate::game::load_level;
use crate::glider::Glider;
use crate::glider::move_glider;
use crate::level::Level;
use crate::level::Levels;
use crate::powerups;
use crate::test_support::START_PROGRESS;
use crate::test_support::camera_position;
use crate::test_support::glider_transform;
use crate::test_support::selected_lane;
use crate::weapons::Bullet;
use crate::weapons::ShotAssets;
use crate::weapons::Weapon;
use crate::weapons::fire_bullets;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use bevy::prelude::*;
use std::f32::consts::TAU;

#[test]
fn enemy_shot_reaching_the_player_lane_costs_a_life() {
    let mut app = crate::test_support::app();
    app.add_systems(FixedUpdate, check_game);
    app.world_mut().spawn(EnemyShot {
        lane: 7,
        depth: LANE_UNIT,
        angle: 0,
    });
    crate::test_support::ticks(&mut app, 1);
    assert!(app.world().resource::<Game>().status == Status::ShotHit);
    assert_eq!(app.world().resource::<Game>().lives, 4);
    assert!(
        app.world_mut()
            .query::<&EnemyShot>()
            .iter(app.world())
            .next()
            .is_none()
    );
    crate::test_support::ticks(&mut app, 163);
    assert_eq!(app.world().resource::<Game>().lives, 3);
    assert!(app.world().resource::<Game>().status == Status::Entering);
}

fn app(lives: u16) -> App {
    let mut app = App::new();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    entry::setup(
        &mut app.world_mut().commands(),
        &mut meshes,
        &mut materials,
        &Level::default().web,
    );
    setup(&mut app.world_mut().commands(), &mut meshes, &mut materials);
    app.world_mut().flush();
    app.insert_resource(Game {
        lives,
        score: 9000,
        ..default()
    })
    .insert_resource(meshes)
    .init_resource::<Level>()
    .init_resource::<Levels>()
    .insert_resource(materials)
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<powerups::Upgrades>()
    .init_resource::<crate::difficulty::EnemyClock>()
    .init_resource::<ShotAssets>()
    .init_resource::<explosion::BurstAssets>()
    .add_systems(
        FixedUpdate,
        (
            move_glider.run_if(controlling),
            powerups::update_jump,
            fire_bullets.run_if(controlling),
            move_enemy_shots,
            animate,
            check_game,
            load_level,
        )
            .chain(),
    )
    .add_systems(Update, (load_level, entry::draw, draw).chain());
    app.world_mut().spawn((
        Glider {
            progress: START_PROGRESS,
            velocity: 0,
        },
        Weapon::default(),
        glider_transform(START_PROGRESS),
        Visibility::Visible,
    ));
    for _ in 0..2 {
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_translation(camera_position(
                glider_transform(START_PROGRESS).translation,
            )),
        ));
    }
    app.world_mut().spawn((
        EnemyShot {
            lane: selected_lane(START_PROGRESS) as usize,
            depth: 2 * LANE_UNIT + LANE_UNIT / 2,
            angle: 0,
        },
        Transform::default(),
    ));
    app
}

fn ticks(app: &mut App, count: usize) {
    for _ in 0..count {
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(Update);
    }
}

#[test]
fn shot_hit_uses_four_zap_outlines_and_locks_controls_until_the_retreat_finishes() {
    let mut app = app(3);
    app.world_mut().resource_mut::<powerups::Upgrades>().laser = true;
    app.world_mut()
        .resource_mut::<powerups::Upgrades>()
        .jump_enabled = true;
    app.world_mut().resource_mut::<powerups::Upgrades>().droid = true;
    ticks(&mut app, 1);
    assert!(app.world().resource::<Game>().status == Status::ShotHit);
    assert_eq!(app.world().resource::<Game>().lives, 3);
    assert!(!app.world().resource::<powerups::Upgrades>().laser);
    assert!(!app.world().resource::<powerups::Upgrades>().jump_enabled);
    assert!(!app.world().resource::<powerups::Upgrades>().droid);
    assert_eq!(
        app.world_mut()
            .query::<&EnemyShot>()
            .iter(app.world())
            .count(),
        0
    );
    let original = glider_transform(START_PROGRESS);
    for key in [KeyCode::KeyA, KeyCode::KeyZ, KeyCode::KeyX] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }
    ticks(&mut app, 10);
    let rotation = original.rotation * Quat::from_rotation_z(120.0 * TAU / 256.0);
    let mut count = 0;
    for (burst, transform, visibility, mesh, material) in app
        .world_mut()
        .query::<(
            &ShotBurst,
            &Transform,
            &Visibility,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        )>()
        .iter(app.world())
    {
        count += 1;
        assert_eq!(*visibility, Visibility::Visible);
        assert_eq!(
            transform.translation.truncate(),
            original.translation.truncate()
        );
        assert!(
            (transform.translation.z
                - original.translation.z
                - (2.5 + burst.0 as f32 * 5.0) * WORLD_UNIT)
                .abs()
                < 1e-5
        );
        assert!(transform.rotation.dot(rotation).abs() > 1.0 - 1e-6);
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&mesh.0)
                .unwrap()
                .count_vertices(),
            60
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
                .unwrap()
                .base_color,
            cry::color(255 - burst.0 as u8 * 17, 255)
        );
    }
    assert_eq!(count, 4);
    assert_eq!(
        app.world_mut().query::<&Bullet>().iter(app.world()).count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&Glider>()
            .single(app.world())
            .unwrap()
            .progress,
        START_PROGRESS
    );
    ticks(&mut app, 110);
    assert!(app.world().resource::<Game>().death.unwrap().visible);
    assert_eq!(app.world().resource::<Game>().lives, 3);
    for camera in app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>()
        .iter(app.world())
    {
        assert!((camera.translation.z - 16.55).abs() < 1e-4);
        assert!(
            camera
                .translation
                .truncate()
                .distance(original.translation.truncate())
                < 1e-5
        );
    }
    ticks(&mut app, 1);
    assert!(!app.world().resource::<Game>().death.unwrap().visible);
    assert!(
        app.world_mut()
            .query_filtered::<&Visibility, With<ShotBurst>>()
            .iter(app.world())
            .all(|visibility| *visibility == Visibility::Hidden)
    );
    ticks(&mut app, 41);
    assert_eq!(app.world().resource::<Game>().lives, 3);
    ticks(&mut app, 1);
    let game = app.world().resource::<Game>();
    assert!(game.status == Status::Entering);
    assert!(game.death.is_none());
    assert_eq!((game.lives, game.score, game.invulnerable), (2, 9000, 120));
    for camera in app
        .world_mut()
        .query_filtered::<&Transform, With<Camera3d>>()
        .iter(app.world())
    {
        assert_eq!(camera.translation, camera_position(original.translation));
    }
}

#[test]
fn the_last_life_is_lost_only_after_the_full_animation() {
    let mut app = app(1);
    ticks(&mut app, 163);
    assert_eq!(app.world().resource::<Game>().lives, 1);
    ticks(&mut app, 1);
    assert!(app.world().resource::<Game>().status == Status::GameOver);
    assert_eq!(app.world().resource::<Game>().lives, 0);
    assert!(app.world().resource::<Game>().entry.is_none());
}

#[test]
fn restart_cancels_the_explosion_and_restores_the_camera() {
    let mut app = app(3);
    ticks(&mut app, 80);
    app.world_mut().resource_mut::<Game>().load = Some(crate::level::LoadReason::Restart);
    app.world_mut().run_schedule(Update);
    assert!(app.world().resource::<Game>().death.is_none());
    assert!(app.world().resource::<Game>().status == Status::Entering);
    assert_eq!(app.world().resource::<Game>().lives, 4);
    assert!(
        app.world_mut()
            .query_filtered::<&Visibility, With<ShotBurst>>()
            .iter(app.world())
            .all(|visibility| *visibility == Visibility::Hidden)
    );
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

#[test]
fn jump_or_invulnerability_prevents_the_hit_animation() {
    for jump in [false, true] {
        let mut app = app(3);
        if jump {
            app.world_mut()
                .resource_mut::<powerups::Upgrades>()
                .jump_enabled = true;
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyX);
        } else {
            app.world_mut().resource_mut::<Game>().invulnerable = 60;
        }
        ticks(&mut app, 2);
        assert!(app.world().resource::<Game>().death.is_none());
        assert_eq!(app.world().resource::<Game>().lives, 3);
    }
}
