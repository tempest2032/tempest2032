//! Checks that the AI droid seeks the nearest enemy and maintains its firing cadence.

use bevy::prelude::*;

use crate::{
    enemies::{
        flipper_render::FlipperAssets,
        flippers::{Flipper, Motion},
    },
    game::Game,
    level::Level,
    powerups::{
        PowerAssets, Upgrades,
        droid::{Droid, move_droid},
    },
    weapons::{self, Bullet, ShotAssets},
    web::LANE_UNIT,
};

#[test]
fn droid_seeks_the_closest_enemy_and_fires_every_six_ticks() {
    let mut app = App::new();
    let mut meshes = Assets::<Mesh>::default();
    let player_shot = meshes.add(weapons::shot_mesh());
    let enemy_shot = meshes.add(crate::enemies::shots::enemy_shot_mesh());
    app.insert_resource(FlipperAssets {
        shot: enemy_shot.clone(),
        ..default()
    })
    .insert_resource(ShotAssets {
        mesh: player_shot.clone(),
        ..default()
    })
    .init_resource::<Game>()
    .init_resource::<crate::difficulty::EnemyClock>()
    .init_resource::<Level>()
    .init_resource::<Upgrades>()
    .init_resource::<PowerAssets>()
    .init_resource::<Assets<StandardMaterial>>()
    .add_systems(FixedUpdate, move_droid);
    app.world_mut().spawn(Flipper {
        lane: 3,
        depth: 80 * LANE_UNIT,
        motion: Motion::Climb,
        pause: 0,
    });
    app.world_mut().spawn(Flipper {
        lane: 10,
        depth: 140 * LANE_UNIT,
        motion: Motion::Climb,
        pause: 0,
    });
    let droid = app
        .world_mut()
        .spawn((
            Droid {
                progress: 2 * LANE_UNIT,
                cooldown: 0,
            },
            Transform::default(),
        ))
        .id();
    for _ in 0..16 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    assert_eq!(
        app.world().get::<Droid>(droid).unwrap().progress,
        3 * LANE_UNIT
    );
    assert_eq!(
        app.world_mut().query::<&Bullet>().iter(app.world()).count(),
        3
    );
    for bullet in app.world_mut().query::<&Mesh3d>().iter(app.world()) {
        assert_eq!(bullet.0, player_shot);
        assert_ne!(bullet.0, enemy_shot);
    }
}
