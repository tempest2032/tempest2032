//! Easy and Hard run enemy and warp timings a fifth slower or faster while
//! keeping their relationships; Normal stays tick for tick.

use super::{Difficulty, advance_enemy_clock};
use crate::enemies::flippers::{Flipper, Motion, move_enemies};
use crate::enemies::shots::{EnemyShot, fire_enemy_shots};
use crate::game::Game;
use crate::test_support::{REAR, app, enemy, ticks};
use bevy::prelude::*;

fn tempo_app(difficulty: Difficulty) -> App {
    let mut app = app();
    app.insert_resource(difficulty).add_systems(
        FixedUpdate,
        (
            advance_enemy_clock,
            (move_enemies, fire_enemy_shots).chain(),
        )
            .chain(),
    );
    app
}

/// Simulation ticks until `done` holds, at each setting.
fn ticks_until(setup: impl Fn(&mut App), done: impl Fn(&mut App) -> bool) -> [usize; 3] {
    Difficulty::ALL.map(|difficulty| {
        let mut app = tempo_app(difficulty);
        setup(&mut app);
        (1..2000)
            .find(|_| {
                ticks(&mut app, 1);
                done(&mut app)
            })
            .expect("the event should happen")
    })
}

fn near(actual: usize, expected: f32) {
    assert!(
        (actual as f32 - expected).abs() <= 1.,
        "{actual} ticks, expected about {expected}"
    );
}

#[test]
fn climbing_covers_a_fifth_less_or_more_depth_per_second() {
    let climbed = Difficulty::ALL.map(|difficulty| {
        let mut app = tempo_app(difficulty);
        let id = enemy(&mut app, 3, REAR, Motion::Climb);
        app.world_mut().resource_mut::<Game>().fire_timer = u8::MAX;
        ticks(&mut app, 60);
        REAR - app.world().get::<Flipper>(id).unwrap().depth
    });
    assert_eq!(climbed[0] * 5, climbed[1] * 4);
    assert_eq!(climbed[2] * 5, climbed[1] * 6);
}

#[test]
fn flips_and_the_shared_fire_timer_scale_with_the_same_tempo() {
    let [easy, normal, hard] = ticks_until(
        |app| {
            enemy(app, 3, 0, Motion::Climb);
        },
        |app| {
            let mut flippers = app.world_mut().query::<&Flipper>();
            matches!(
                flippers.single(app.world()).unwrap().motion,
                Motion::Pause(_)
            )
        },
    );
    near(easy, normal as f32 * 1.25);
    near(hard, normal as f32 / 1.2);
    let [easy, normal, hard] = ticks_until(
        |app| {
            enemy(app, 3, REAR, Motion::Climb);
        },
        |app| {
            let mut shots = app.world_mut().query::<&EnemyShot>();
            shots.iter(app.world()).next().is_some()
        },
    );
    assert_eq!(normal, 113, "the original first shot after 112 ticks");
    near(easy, normal as f32 * 1.25);
    near(hard, normal as f32 / 1.2);
}

#[test]
fn spawn_timers_and_the_pulsar_pulse_follow_the_tempo() {
    use crate::enemies::flippers::spawn_enemies;
    use crate::enemies::pulsars::{Pulse, pulse};
    let [easy, normal, hard] = Difficulty::ALL.map(|difficulty| {
        let mut app = app();
        app.insert_resource(difficulty)
            .add_systems(FixedUpdate, (advance_enemy_clock, spawn_enemies).chain());
        let level = app.world().resource::<crate::level::Level>().clone();
        crate::game::reset_wave(&mut app.world_mut().resource_mut::<Game>(), &level);
        app.world_mut().resource_mut::<Game>().status = crate::game::Status::Playing;
        (1..1000)
            .find(|_| {
                ticks(&mut app, 1);
                let mut flippers = app.world_mut().query::<&Flipper>();
                flippers.iter(app.world()).next().is_some()
            })
            .unwrap()
    });
    near(easy, normal as f32 * 1.25);
    near(hard, normal as f32 / 1.2);
    let level = crate::level::Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/17.toml")),
        "level17",
    )
    .unwrap();
    let phases = Difficulty::ALL.map(|difficulty| {
        let mut app = app();
        app.insert_resource(difficulty)
            .insert_resource(level.clone())
            .add_systems(FixedUpdate, (advance_enemy_clock, pulse).chain());
        app.world_mut().resource_mut::<Game>().status = crate::game::Status::Playing;
        ticks(&mut app, 600);
        app.world().resource::<Pulse>().frame
    });
    assert_eq!(phases, [480, 600, 720]);
}

#[test]
fn warp_zones_travel_at_the_tempo_while_steering_keeps_its_rates() {
    use crate::warp::{Session, ZoneKind, advance};
    for kind in [ZoneKind::GateFlight, ZoneKind::Tunnel] {
        let progress = Difficulty::ALL.map(|difficulty| {
            let mut app = app();
            app.insert_resource(difficulty)
                .add_systems(FixedUpdate, (advance_enemy_clock, advance).chain());
            let mut game = app.world_mut().resource_mut::<Game>();
            game.status = crate::game::Status::Bonus;
            game.bonus = Some(Session::new(kind, 0, None, 0));
            ticks(&mut app, 20);
            let session = app.world().resource::<Game>().bonus.clone().unwrap();
            session
                .tunnel
                .map_or(session.distance, |tunnel| i64::from(tunnel.segments))
        });
        assert_eq!(progress[0] * 5, progress[1] * 4, "{kind:?}");
        assert_eq!(progress[2] * 5, progress[1] * 6, "{kind:?}");
    }
}
