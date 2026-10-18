//! Headless Bevy test fixtures, fixed-tick helpers, the bundled level catalog,
//! and default-web geometry.
//! The gameplay fixture starts in Playing rather than the app's entry animation.

use crate::animations::flipper_capture::animate_capture;
use crate::animations::glider_death as shot_death;
use crate::animations::level_entry as entry;
use crate::enemies::beasts;
use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::flippers::Flipper;
use crate::enemies::flippers::Motion;
use crate::enemies::flippers::flipper_transform;
use crate::enemies::tankers;
use crate::explosion;
use crate::game::Game;
use crate::game::Status;
use crate::game::check_game;
use crate::game::load_level;
use crate::glider::Glider;
use crate::glider::move_glider;
use crate::level::Level;
use crate::level::Levels;
use crate::powerups;
use crate::weapons::Bullet;
use crate::weapons::ShotAssets;
use crate::weapons::Weapon;
use crate::weapons::fire_bullets;
use crate::weapons::move_bullets;
use crate::web::FRONT_RADIUS;
use crate::web::LANE_UNIT;
use bevy::prelude::*;

pub(crate) fn movement_app(progress: i32, keys: &[KeyCode]) -> App {
    let mut app = App::new();
    let mut input = ButtonInput::<KeyCode>::default();
    for &key in keys {
        input.press(key);
    }
    app.init_resource::<crate::difficulty::Difficulty>()
        .init_resource::<Level>()
        .insert_resource(input)
        .add_systems(FixedUpdate, move_glider);
    app.world_mut().spawn((
        Glider {
            progress,
            velocity: 0,
        },
        glider_transform(progress),
    ));
    app
}

pub(crate) fn state(app: &mut App) -> (i32, i32) {
    let glider = app
        .world_mut()
        .query::<&Glider>()
        .single(app.world())
        .unwrap();
    (glider.progress, glider.velocity)
}

pub(crate) fn ticks(app: &mut App, count: usize) {
    for _ in 0..count {
        app.world_mut().run_schedule(FixedUpdate);
    }
}

pub(crate) fn firing_app(progress: i32, keys: &[KeyCode]) -> App {
    let mut app = movement_app(progress, keys);
    let glider = app
        .world_mut()
        .query_filtered::<Entity, With<Glider>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(glider).insert(Weapon::default());
    app.insert_resource(ShotAssets::default()).add_systems(
        FixedUpdate,
        (move_bullets, fire_bullets).chain().after(move_glider),
    );
    app
}

pub(crate) fn bullet_count(app: &mut App) -> usize {
    app.world_mut().query::<&Bullet>().iter(app.world()).count()
}

pub(crate) fn pickup(depth: i32) -> powerups::pickups::PowerUp {
    powerups::pickups::PowerUp {
        lane: 3,
        depth,
        rez: 300,
        radius: 10,
        captured: false,
        mesh: None,
    }
}

pub(crate) fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<Level>()
        .init_resource::<Levels>()
        .init_resource::<crate::difficulty::Difficulty>()
        .init_resource::<powerups::Upgrades>()
        .init_resource::<powerups::PowerAssets>()
        .init_resource::<FlipperAssets>()
        .init_resource::<tankers::TankerAssets>()
        .init_resource::<crate::enemies::fuseballs::FuseAssets>()
        .init_resource::<crate::enemies::pulsar_render::PulsarAssets>()
        .init_resource::<crate::enemies::pulsars::Pulse>()
        .init_resource::<beasts::BeastAssets>()
        .init_resource::<crate::enemies::mirrors::MirrorAssets>()
        .init_resource::<crate::enemies::aliens::AlienAssets>()
        .init_resource::<explosion::BurstAssets>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(
            FixedUpdate,
            (entry::advance, animate_capture, shot_death::animate)
                .chain()
                .before(check_game),
        )
        .add_systems(FixedUpdate, load_level.after(check_game));
    app.world_mut().spawn((
        Glider {
            progress: START_PROGRESS,
            velocity: 0,
        },
        Weapon::default(),
        glider_transform(START_PROGRESS),
        Visibility::Visible,
    ));
    app
}

pub(crate) fn finish_capture(app: &mut App) {
    assert!(app.world().resource::<Game>().status == Status::Captured);
    ticks(app, 130);
    assert!(app.world().resource::<Game>().capture.is_none());
}

pub(crate) fn enemy(app: &mut App, lane: usize, depth: i32, motion: Motion) -> Entity {
    let flipper = Flipper {
        lane,
        depth,
        motion,
        pause: 0,
    };
    let transform = flipper_transform(&flipper, &app.world().resource::<Level>().web);
    app.world_mut()
        .spawn((
            transform,
            flipper,
            Mesh3d::default(),
            MeshMaterial3d::<StandardMaterial>::default(),
        ))
        .id()
}

pub(crate) const LANES: usize = 16;

pub(crate) const DEPTH: f32 = 40.0 * FRONT_RADIUS / 6.0;

pub(crate) const WEB_PERIOD: i32 = LANES as i32 * LANE_UNIT;

pub(crate) const START_PROGRESS: i32 = 7 * LANE_UNIT;

pub(crate) fn rim_vertex(index: f32, radius: f32, z: f32) -> Vec3 {
    Level::default().web.vertex(index, radius, z)
}

pub(crate) fn selected_lane(progress: i32) -> f32 {
    Level::default().web.selected_lane(progress)
}

pub(crate) fn glider_transform(progress: i32) -> Transform {
    Level::default().web.glider_transform(progress)
}

pub(crate) fn camera_position(glider: Vec3) -> Vec3 {
    Level::default().web.camera_position(glider)
}

pub(crate) const REAR: i32 = 160 * LANE_UNIT;

pub(crate) const MAX_FLIPPERS: usize = 11;

pub(crate) const BRIGHTEST_RED: u8 = 240;

/// A copy of the bundled catalog, parsed once per test run.
pub(crate) fn levels() -> crate::level::Levels {
    static LEVELS: std::sync::LazyLock<Vec<Level>> = std::sync::LazyLock::new(|| {
        crate::level::Levels::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("levels"))
            .unwrap()
            .0
    });
    crate::level::Levels(LEVELS.clone())
}
