//! Checks pickup formation, lane and height requirements, rewards, and collection-burst cleanup.

use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::tankers;
use crate::explosion;
use crate::game::Game;
use crate::game::Status;
use crate::glider::Glider;
use crate::level::Level;
use crate::powerups::PowerAssets;
use crate::powerups::Upgrades;
use crate::powerups::pickups::PowerUp;
use crate::powerups::pickups::advance_powerups;
use crate::test_support::{app, pickup, ticks};
use crate::web::LANE_UNIT;
use bevy::prelude::*;

#[test]
fn a_drop_forms_for_76_ticks_then_moves_at_the_original_speed() {
    let mut pickup = pickup(100 * LANE_UNIT);
    for _ in 0..76 {
        assert!(!pickup.tick(3, Some(0)));
    }
    assert_eq!(pickup.depth, 100 * LANE_UNIT);
    assert_eq!(pickup.rez, -4);
    pickup.tick(3, Some(0));
    assert_eq!(pickup.depth, 100 * LANE_UNIT - 0x18000);
}

#[test]
fn pickup_requires_the_right_lane_range_and_a_grounded_claw() {
    let mut pickup = pickup(32 * LANE_UNIT);
    pickup.rez = -4;
    assert!(!pickup.tick(2, Some(0)));
    assert!(!pickup.tick(3, None));
    assert!(pickup.tick(3, Some(0)));
    assert_eq!(pickup.radius, 11);
    assert!(pickup.captured);
    let depth = pickup.depth;
    for _ in 0..49 {
        assert!(!pickup.tick(3, Some(0)));
    }
    assert_eq!(
        pickup.depth, depth,
        "the pickup burst stays at its collection point"
    );
    assert!(pickup.radius >= 255);
    let mut passed = crate::test_support::pickup(-LANE_UNIT);
    passed.rez = -4;
    assert!(
        !passed.tick(3, Some(0)),
        "a passed pickup cannot be collected"
    );
}

#[test]
fn exit_collection_uses_the_claws_depth_instead_of_the_rim() {
    let mut pickup = pickup(64 * LANE_UNIT);
    pickup.rez = -4;
    assert!(pickup.tick(3, Some(40 * LANE_UNIT)));
    let mut passed = crate::test_support::pickup(39 * LANE_UNIT);
    passed.rez = -4;
    assert!(!passed.tick(3, Some(40 * LANE_UNIT)));
}

#[test]
fn collecting_during_exit_grants_a_droid_on_the_next_webs_first_pickup() {
    use crate::animations::level_exit;
    use crate::game::{check_game, load_level};
    use crate::messages::Id;
    let mut app = app();
    app.add_systems(
        FixedUpdate,
        (advance_powerups, level_exit::animate, check_game)
            .chain()
            .before(load_level),
    );
    let second = Level {
        number: 2,
        ..default()
    };
    app.world_mut()
        .resource_mut::<crate::level::Levels>()
        .0
        .push(second);
    app.world_mut().resource_mut::<Game>().remaining = 0;
    let mut bonus = pickup(100 * LANE_UNIT);
    bonus.lane = (crate::test_support::START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    let entity = app.world_mut().spawn(bonus).id();
    ticks(&mut app, 1);
    assert!(app.world().resource::<Game>().status == Status::Exiting);
    for _ in 0..100 {
        ticks(&mut app, 1);
        if app.world().get::<PowerUp>(entity).unwrap().captured {
            break;
        }
    }
    assert!(app.world().get::<PowerUp>(entity).unwrap().captured);
    let game = app.world().resource::<Game>();
    assert_eq!(game.score, 0);
    assert!(game.exit.unwrap().depth > 0);
    assert!(
        game.announcements
            .iter()
            .any(|event| event.id == Id::YesYesYes)
    );
    assert!(!app.world().resource::<Upgrades>().droid);
    ticks(&mut app, 400);
    assert_eq!(app.world().resource::<Level>().number, 2);
    ticks(&mut app, 256);
    assert!(app.world().resource::<Game>().status == Status::Playing);
    for (laser, droids) in [(false, 1), (true, 1)] {
        let mut bonus = pickup(0);
        bonus.lane = app
            .world()
            .resource::<Level>()
            .web
            .selected_lane(crate::test_support::START_PROGRESS) as usize;
        bonus.rez = -4;
        app.world_mut().spawn(bonus);
        ticks(&mut app, 1);
        assert!(app.world().resource::<Upgrades>().droid);
        assert_eq!(app.world().resource::<Upgrades>().laser, laser);
        assert_eq!(
            app.world_mut()
                .query::<&crate::powerups::Droid>()
                .iter(app.world())
                .count(),
            droids
        );
    }
    assert_eq!(app.world().resource::<Game>().score, 0);
}

#[test]
fn a_surprise_pickup_can_end_the_web_with_enemies_and_spawns_remaining() {
    use crate::animations::level_exit;
    use crate::enemies::{flippers::Motion, shots::EnemyShot};
    use crate::game::{check_game, load_level};
    use crate::messages::Id;
    use crate::test_support::{START_PROGRESS, enemy};

    let mut app = app();
    app.add_systems(
        FixedUpdate,
        (advance_powerups, level_exit::animate, check_game)
            .chain()
            .before(load_level),
    );
    let second = Level {
        number: 2,
        ..default()
    };
    app.world_mut()
        .resource_mut::<crate::level::Levels>()
        .0
        .push(second);
    app.world_mut().resource_mut::<Upgrades>().stage = 1;
    // The next shared gameplay roll is zero, within suprise's 0..=8 skip range.
    app.world_mut().resource_mut::<Game>().random = 1972;
    let enemies = [1, 5].map(|lane| enemy(&mut app, lane, 100 * LANE_UNIT, Motion::Climb));
    let shot = app
        .world_mut()
        .spawn(EnemyShot {
            lane: 2,
            depth: 60 * LANE_UNIT,
            angle: 0,
        })
        .id();
    let mut bonus = pickup(0);
    bonus.lane = (START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    app.world_mut().spawn(bonus);
    assert!(app.world().resource::<Game>().remaining > 0);
    ticks(&mut app, 1);
    let game = app.world().resource::<Game>();
    assert!(game.status == Status::Exiting && game.outta_here);
    assert_eq!((game.score, game.lives, game.remaining), (3000, 4, 0));
    assert_eq!(
        game.announcements
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        [Id::OuttaHere, Id::Excellent]
    );
    assert!(
        enemies
            .into_iter()
            .chain([shot])
            .all(|entity| app.world().get_entity(entity).is_err())
    );
    assert!(game.exit.is_some());
    for _ in 0..400 {
        ticks(&mut app, 1);
        if app.world().resource::<Level>().number == 2 {
            break;
        }
    }
    assert_eq!(app.world().resource::<Level>().number, 2);
    assert_eq!(app.world().resource::<Game>().score, 3000);
}

#[test]
fn a_points_pickup_can_earn_a_score_life() {
    let mut app = app();
    app.add_systems(FixedUpdate, advance_powerups);
    app.world_mut().resource_mut::<Upgrades>().stage = 1;
    app.world_mut().resource_mut::<Game>().score = 19_000;
    let mut bonus = pickup(0);
    bonus.lane = (crate::test_support::START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    app.world_mut().spawn(bonus);
    ticks(&mut app, 1);
    let game = app.world().resource::<Game>();
    assert_eq!((game.score, game.lives), (21_000, 5));
    assert_eq!(
        game.announcements
            .iter()
            .filter(|event| event.id == crate::messages::Id::OneUp)
            .count(),
        1
    );
}

#[test]
fn restarting_discards_an_unclaimed_early_droid() {
    use crate::animations::level_exit::Exit;
    let mut app = app();
    app.add_systems(
        FixedUpdate,
        advance_powerups.before(crate::game::load_level),
    );
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::Exiting;
    game.exit = Some(Exit::new(
        crate::test_support::glider_transform(crate::test_support::START_PROGRESS).translation,
        &Level::default().web,
    ));
    let mut bonus = pickup(0);
    bonus.lane = (crate::test_support::START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    app.world_mut().spawn(bonus);
    ticks(&mut app, 1);
    assert!(app.world().resource::<Upgrades>().droid_next);
    app.world_mut().resource_mut::<Game>().load = Some(crate::level::LoadReason::Restart);
    ticks(&mut app, 1);
    assert!(!app.world().resource::<Upgrades>().droid_next);
    app.world_mut().resource_mut::<Game>().status = Status::Playing;
    let mut bonus = pickup(0);
    bonus.lane = (crate::test_support::START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    app.world_mut().spawn(bonus);
    ticks(&mut app, 1);
    assert!(app.world().resource::<Upgrades>().laser);
    assert!(!app.world().resource::<Upgrades>().droid);
}

#[test]
fn the_spinning_exit_claw_cannot_collect_pickups() {
    use crate::animations::level_exit::Exit;
    let mut app = app();
    app.add_systems(FixedUpdate, advance_powerups);
    let mut exit = Exit::new(
        crate::test_support::glider_transform(crate::test_support::START_PROGRESS).translation,
        &Level::default().web,
    );
    exit.depth = exit.rear;
    exit.wireframe = true;
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::Exiting;
    game.exit = Some(exit);
    let mut bonus = pickup(exit.depth);
    bonus.lane = (crate::test_support::START_PROGRESS / LANE_UNIT) as usize;
    bonus.rez = -4;
    let entity = app.world_mut().spawn(bonus).id();
    ticks(&mut app, 1);
    assert!(!app.world().get::<PowerUp>(entity).unwrap().captured);
    assert!(app.world().resource::<Game>().announcements.is_empty());
}

#[test]
fn collection_grants_one_reward_and_the_burst_expires() {
    let mut app = App::new();
    app.init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<Level>()
        .init_resource::<tankers::TankerAssets>()
        .init_resource::<FlipperAssets>()
        .init_resource::<crate::enemies::fuseballs::FuseAssets>()
        .init_resource::<crate::enemies::pulsar_render::PulsarAssets>()
        .init_resource::<Upgrades>()
        .init_resource::<PowerAssets>()
        .init_resource::<explosion::BurstAssets>()
        .add_systems(FixedUpdate, advance_powerups);
    app.world_mut().spawn(Glider {
        progress: 3 * LANE_UNIT,
        velocity: 0,
    });
    let mut pickup = pickup(30 * LANE_UNIT);
    pickup.rez = -4;
    let entity = app.world_mut().spawn(pickup).id();
    app.world_mut().run_schedule(FixedUpdate);
    assert!(app.world().resource::<Upgrades>().laser);
    assert_eq!(app.world().resource::<Upgrades>().stage, 1);
    for _ in 0..48 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    assert!(app.world().get::<PowerUp>(entity).is_some());
    app.world_mut().run_schedule(FixedUpdate);
    assert!(app.world().get::<PowerUp>(entity).is_none());
    assert_eq!(app.world().resource::<Upgrades>().stage, 1);
}
