//! Gate course fidelity, fixed-tick controls, scoring, and bonus routing invariants.

use super::*;
use crate::{
    game,
    glider::Glider,
    level::{Level, LevelActor},
    powerups::Upgrades,
    test_support::ticks,
    weapons::Weapon,
};

fn app(number: usize) -> App {
    let mut app = crate::test_support::app();
    let catalog = (1..=100)
        .map(|number| Level {
            number,
            ..default()
        })
        .collect::<Vec<_>>();
    app.insert_resource(catalog[number - 1].clone())
        .insert_resource(Levels(catalog));
    app.world_mut().resource_mut::<Game>().level_index = number - 1;
    app.add_systems(FixedUpdate, advance.before(game::load_level));
    app
}

fn enter(app: &mut App, destination: usize, course: Option<u8>) {
    app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
        kind: ZoneKind::GateFlight,
        course,
        gate: None,
        destination,
    });
    ticks(app, 1);
}

fn aim(session: &mut Session, index: usize, dx: i32, dy: i32) {
    let (x, y, _) = session.spec(index);
    session.x = Axis {
        position: (160 + i32::from(x) + dx) * UNIT,
        velocity: 0,
    };
    session.y = Axis {
        position: (i32::from(y) + dy) * 8 * UNIT,
        velocity: 0,
    };
}

#[test]
fn all_original_courses_have_fifty_three_gates_and_a_final_pyramid() {
    assert_eq!(courses::ORDER, [0, 5, 2, 1, 6, 4, 3, 7]);
    for course in courses::COURSES {
        assert_eq!(course[52] as u8, 4);
        assert_eq!(course[53] as u8, 255);
        assert!(course[..53].iter().all(|gate| *gate as u8 <= 4));
    }
    let session = Session::new(ZoneKind::GateFlight, 0, None, 1);
    assert_eq!(session.spec(12), (-16, 17, 0));
    assert_eq!(Session::new(ZoneKind::GateFlight, 1, None, 1).course, 5);
    assert_eq!(Session::new(ZoneKind::GateFlight, 1, Some(8), 1).course, 7);
}

#[test]
fn selected_gate_is_next_and_inherits_prior_boosts_without_score_announcements() {
    for kind in [ZoneKind::GateFlight, ZoneKind::Rings] {
        for course in 1..=8 {
            let mut played = Session::new(kind, 0, Some(course), 0);
            for gate in 1..=53 {
                let mut selected = Session::new(kind, 0, Some(course), 0);
                selected.start_at_gate(gate);
                assert_eq!(selected.speed, played.speed);
                assert_eq!(selected.passed, usize::from(gate - 1));
                assert_eq!(selected.step_flight(0, 0, selected.speed), 0);
                assert_eq!(selected.gates.front().unwrap().index, usize::from(gate - 1));
                let (x, y, _) = selected.spec(usize::from(gate - 1));
                assert_eq!(selected.position(), Vec2::new(f32::from(x), f32::from(y)));
                assert_eq!((selected.x.velocity, selected.y.velocity), (0, 0));
                assert!(selected.announcements.is_empty());
                aim(&mut played, usize::from(gate - 1), 0, 0);
                played.pass(usize::from(gate - 1));
            }
        }
    }
}

#[test]
fn final_gate_start_reaches_completion_without_input_and_only_awards_the_final_gate() {
    for kind in [ZoneKind::GateFlight, ZoneKind::Rings] {
        for course in 1..=8 {
            let mut app = app(1);
            app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
                kind,
                course: Some(course),
                gate: Some(53),
                destination: 0,
            });
            ticks(&mut app, 1);
            assert_eq!(app.world().resource::<Game>().score, 0);
            for _ in 0..1000 {
                if matches!(
                    app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
                    Phase::Success(_)
                ) {
                    break;
                }
                ticks(&mut app, 1);
            }
            let game = app.world().resource::<Game>();
            let session = game.bonus.as_ref().unwrap();
            assert!(matches!(session.phase, Phase::Success(_)));
            assert_eq!(session.passed, GATES);
            assert_eq!(
                game.score,
                if kind == ZoneKind::GateFlight {
                    750
                } else {
                    250
                }
            );
            ticks(&mut app, 151);
            assert!(matches!(
                app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
                Phase::Victory(_)
            ));
            assert_eq!(
                app.world().resource::<Game>().score,
                20_000
                    + if kind == ZoneKind::GateFlight {
                        750
                    } else {
                        250
                    }
            );
        }
    }
}

#[test]
fn rings_mirror_courses_use_fixed_spacing_and_keep_original_scoring() {
    for course in 1..=8 {
        let gates = Session::new(ZoneKind::GateFlight, 0, Some(course), 0);
        let rings = Session::new(ZoneKind::Rings, 0, Some(course), 0);
        assert_eq!(rings.speed, 0xa000);
        for index in 0..GATES {
            let (x, y, kind) = gates.spec(index);
            assert_eq!(rings.spec(index), (-x, y, kind));
        }
    }
    for (dx, dy, score) in [(0, 0, 250), (4, 0, 500), (7, 1, 750), (7, 7, 1)] {
        let mut session = Session::new(ZoneKind::Rings, 0, None, 0);
        aim(&mut session, 0, dx, dy);
        assert_eq!(session.pass(0), Some(score));
        assert_eq!(session.rings.as_ref().unwrap().rotation, 1);
    }
    let mut session = Session::new(ZoneKind::Rings, 0, Some(8), 0);
    session.step_flight(0, 0, session.speed);
    assert_eq!(session.generation_wait, 8);
    for _ in 0..171 {
        session.step_flight(0, 0, session.speed);
    }
    assert_eq!(session.cursor, 1);
    session.step_flight(0, 0, session.speed);
    assert_eq!(session.cursor, 2);
}

#[test]
fn all_ring_courses_complete_and_settle_the_shared_completion_reward_once() {
    for course in 1..=8 {
        let mut session = Session::new(ZoneKind::Rings, 0, Some(course), 0);
        for _ in 0..100_000 {
            if let Some(index) = session.gates.front().map(|g| g.index) {
                aim(&mut session, index, 0, 0);
            }
            session.step_flight(0, 0, session.speed);
            if !matches!(session.phase, Phase::Flying) {
                break;
            }
        }
        assert!(
            matches!(session.phase, Phase::Success(_)),
            "course {course}"
        );
        assert_eq!(session.passed, GATES);
    }
    let mut app = app(33);
    app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
        kind: ZoneKind::Rings,
        course: Some(1),
        gate: None,
        destination: 32,
    });
    ticks(&mut app, 1);
    let mut game = app.world_mut().resource_mut::<Game>();
    game.score = 1234;
    game.bonus.as_mut().unwrap().phase = Phase::Success(1);
    ticks(&mut app, 3);
    let game = app.world().resource::<Game>();
    assert_eq!(game.score, 21_234);
    assert_eq!(game.bonus.as_ref().unwrap().destination, 36);
}

#[test]
fn generator_spacing_counts_distance_blocks_and_counter_underflow() {
    let mut session = Session::new(ZoneKind::GateFlight, 0, None, 1);
    session.step_flight(0, 0, session.speed);
    assert_eq!(session.cursor, 1);
    for _ in 0..215 {
        session.step_flight(0, 0, session.speed);
    }
    assert_eq!(session.cursor, 1);
    session.step_flight(0, 0, session.speed);
    assert_eq!(session.cursor, 2);
}

#[test]
fn collision_uses_integer_rectangular_tolerances_and_precision_scores() {
    for (dx, dy, score) in [(0, 0, 750), (3, 0, 750), (4, 0, 500), (7, 7, 250)] {
        let mut session = Session::new(ZoneKind::GateFlight, 0, None, 1);
        aim(&mut session, 0, dx, dy);
        assert_eq!(session.pass(0), Some(score));
    }
    let mut session = Session::new(ZoneKind::GateFlight, 0, None, 1);
    aim(&mut session, 0, 7, 0);
    session.x.position += UNIT - 1;
    assert_eq!(session.pass(0), Some(500));
    aim(&mut session, 0, 8, 0);
    assert_eq!(session.pass(0), None);
    assert!(matches!(session.phase, Phase::Failure(151)));
}

#[test]
fn gate_tolerance_matches_source_integer_words_on_both_axes() {
    for axis in 0..2 {
        for (offset, passes) in [
            (-7 * UNIT - 1, false),
            (-7 * UNIT, true),
            (8 * UNIT - 1, true),
            (8 * UNIT, false),
        ] {
            let mut session = Session::new(ZoneKind::GateFlight, 0, None, 1);
            aim(&mut session, 0, 0, 0);
            if axis == 0 {
                session.x.position += offset;
            } else {
                session.y.position += offset * 8;
            }
            assert_eq!(
                session.pass(0).is_some(),
                passes,
                "axis {axis}, offset {offset}"
            );
        }
    }
}

#[test]
fn gate_effects_change_vertical_velocity_speed_and_final_state() {
    let mut session = Session::new(ZoneKind::GateFlight, 0, None, 1);
    aim(&mut session, 24, 0, 0);
    session.pass(24);
    assert_eq!(session.y.velocity, -0x80000);
    aim(&mut session, 41, 0, 0);
    session.pass(41);
    assert_eq!(session.y.velocity, 0x80000);
    let speed = session.speed;
    aim(&mut session, 5, 0, 0);
    session.pass(5);
    assert_eq!(session.speed, speed + speed / 8);
    aim(&mut session, 52, 0, 0);
    session.pass(52);
    assert!(matches!(session.phase, Phase::Success(151)));
}

#[test]
fn inertial_controls_have_source_acceleration_friction_and_nonclamping_coast() {
    let mut axis = Axis {
        position: 160 * UNIT,
        velocity: 0,
    };
    axis.step(1, 0x1000, 0x800, 0x20000, 0, 320 * UNIT);
    assert_eq!(axis.velocity, 0x1000);
    axis.step(0, 0x1000, 0x800, 0x20000, 0, 320 * UNIT);
    assert_eq!(axis.velocity, 0x800);
    axis.position = 321 * UNIT;
    axis.velocity = UNIT;
    axis.step(0, 0x1000, 0x800, 0x20000, 0, 320 * UNIT);
    assert!(axis.position > 321 * UNIT);
    axis.step(1, 0x1000, 0x800, 0x20000, 0, 320 * UNIT);
    assert_eq!(axis.velocity, 0);
}

fn steer(axis: Axis, target: i32, acceleration: i32, min: i32, max: i32) -> i32 {
    let stopping = i64::from(axis.position)
        + i64::from(axis.velocity) * i64::from(axis.velocity.abs()) / (2 * i64::from(acceleration));
    let direction = (i64::from(target) - stopping).signum() as i32;
    if direction == axis.velocity.signum()
        && ((direction < 0 && axis.position < min) || (direction > 0 && axis.position >= max))
    {
        0
    } else {
        direction
    }
}

#[test]
fn steering_inputs_reach_every_gate_on_all_eight_courses() {
    for course in 1..=8 {
        let mut app = app(1);
        app.world_mut().resource_mut::<Game>().bonus_attempts[0] = courses::ORDER
            .iter()
            .position(|index| *index == usize::from(course - 1))
            .unwrap() as u32;
        enter(&mut app, 1, Some(course));
        for _ in 0..100_000 {
            let session = app.world().resource::<Game>().bonus.as_ref().unwrap();
            if !matches!(session.phase, Phase::Flying) {
                break;
            }
            let index = session
                .gates
                .front()
                .map_or(session.passed, |gate| gate.index);
            let (x, y, _) = session.spec(index);
            let horizontal = steer(
                session.x,
                (160 + i32::from(x)) * UNIT,
                0x1000,
                0,
                320 * UNIT,
            );
            let vertical = steer(
                session.y,
                i32::from(y) * 8 * UNIT,
                0x8000,
                -320 * UNIT,
                320 * UNIT,
            );
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            for (direction, negative, positive) in [
                (horizontal, KeyCode::ArrowLeft, KeyCode::ArrowRight),
                (vertical, KeyCode::ArrowUp, KeyCode::ArrowDown),
            ] {
                if direction != 0 {
                    keys.press(if direction < 0 { negative } else { positive });
                }
            }
            ticks(&mut app, 1);
        }
        let session = app.world().resource::<Game>().bonus.as_ref().unwrap();
        assert!(
            matches!(session.phase, Phase::Success(_)),
            "course {course} failed at gate {}, position {:?}, target {:?}",
            session.passed + 1,
            session.position(),
            session.spec(session.passed)
        );
        assert_eq!(session.passed, GATES);
    }
}

#[test]
fn vertical_boosts_need_released_input_to_coast_beyond_the_powered_limit() {
    let mut session = Session::new(ZoneKind::GateFlight, 0, Some(1), 1);
    aim(&mut session, 35, 0, 0);
    session.pass(35);
    let mut held = session.y;
    held.step(-1, 0x8000, 0x2000, 0x80000, -320 * UNIT, 320 * UNIT);
    assert_eq!(held.velocity, 0);
    assert_eq!(held.position, -48 * 8 * UNIT);
    for _ in 0..32 {
        session
            .y
            .step(0, 0x8000, 0x2000, 0x80000, -320 * UNIT, 320 * UNIT);
    }
    assert!(session.position().y < -64.);
    assert!(session.y.velocity < 0);
}

#[test]
fn natural_entry_spends_tokens_once_only_in_an_implemented_level_band() {
    for (number, expected) in [
        (1, true),
        (16, true),
        (64, true),
        (32, true),
        (48, true),
        (96, true),
        (99, false),
        (100, false),
    ] {
        let mut app = app(number);
        app.world_mut()
            .resource_mut::<Upgrades>()
            .seed_warp_tokens(3);
        app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Advance);
        ticks(&mut app, 1);
        assert_eq!(
            app.world().resource::<Game>().status == Status::Bonus,
            expected
        );
        assert_eq!(
            app.world().resource::<Upgrades>().warp_tokens(),
            if expected { 0 } else { 3 }
        );
        if expected {
            let zone = ZoneKind::for_web((number % 100) + 1).index();
            assert_eq!(app.world().resource::<Game>().bonus_attempts[zone], 1);
            ticks(&mut app, 5);
            assert_eq!(app.world().resource::<Game>().bonus_attempts[zone], 1);
        }
    }
}

#[test]
fn failure_costs_no_life_and_returns_to_the_pending_web_exactly_once() {
    let mut app = app(1);
    app.world_mut().resource_mut::<Game>().score = 1234;
    app.world_mut().resource_mut::<Game>().lives = 2;
    app.world_mut().spawn(LevelActor);
    enter(&mut app, 1, None);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<LevelActor>>()
            .iter(app.world())
            .count(),
        0
    );
    app.world_mut()
        .resource_mut::<Game>()
        .bonus
        .as_mut()
        .unwrap()
        .phase = Phase::Failure(1);
    ticks(&mut app, 1);
    assert_eq!(app.world().resource::<Level>().number, 2);
    assert_eq!(
        (
            app.world().resource::<Game>().score,
            app.world().resource::<Game>().lives
        ),
        (1234, 2)
    );
    assert_eq!(app.world().resource::<Game>().entry, Some(0));
    ticks(&mut app, 1);
    assert_eq!(app.world().resource::<Level>().number, 2);
}

#[test]
fn tunnel_returns_preserving_lives_and_points_and_success_has_no_gate_completion_award() {
    for (success, destination, expected) in [(false, 16, 17), (true, 16, 21), (true, 90, 91)] {
        let mut app = app(17);
        app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
            kind: ZoneKind::Tunnel,
            course: Some(1),
            gate: None,
            destination,
        });
        ticks(&mut app, 1);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.score = 1234;
        game.lives = 2;
        game.bonus.as_mut().unwrap().phase = if success {
            Phase::Success(1)
        } else {
            Phase::Failure(1)
        };
        if success {
            ticks(&mut app, 302);
            assert_eq!(
                app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
                Phase::Fade(151)
            );
            assert_eq!(app.world().resource::<Level>().number, 17);
            ticks(&mut app, 150);
            assert_eq!(
                app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
                Phase::Fade(1)
            );
        }
        ticks(&mut app, 1);
        let game = app.world().resource::<Game>();
        assert_eq!((game.score, game.lives), (1234, 2));
        assert!(game.bonus.is_none());
        assert_eq!(app.world().resource::<Level>().number, expected);
    }
}

#[test]
fn success_awards_once_and_skips_four_extra_webs_except_in_the_late_game() {
    for (destination, expected) in [(1, 6), (89, 94), (90, 91), (99, 100)] {
        let mut app = app(1);
        enter(&mut app, destination, None);
        app.world_mut()
            .resource_mut::<Game>()
            .bonus
            .as_mut()
            .unwrap()
            .phase = Phase::Success(1);
        ticks(&mut app, 1);
        assert_eq!(app.world().resource::<Game>().score, 20_000);
        assert_eq!(
            app.world()
                .resource::<Game>()
                .bonus
                .as_ref()
                .unwrap()
                .warp_five,
            destination < 90
        );
        assert_eq!(
            app.world()
                .resource::<Game>()
                .announcements
                .iter()
                .filter(|event| event.id == crate::messages::Id::WarpFive)
                .count(),
            usize::from(destination < 90)
        );
        assert_eq!(app.world().resource::<Game>().lives, 5);
        ticks(&mut app, 10);
        assert_eq!(app.world().resource::<Game>().score, 20_000);
        assert_eq!(app.world().resource::<Game>().lives, 5);
        app.world_mut()
            .resource_mut::<Game>()
            .bonus
            .as_mut()
            .unwrap()
            .phase = Phase::Victory(1);
        ticks(&mut app, 1);
        assert!(matches!(
            app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
            Phase::Fade(151)
        ));
        ticks(&mut app, 151);
        assert_eq!(app.world().resource::<Level>().number, expected);
    }
}

#[test]
fn completion_runs_the_source_scene_fade_victory_and_final_melt_timers() {
    for kind in [ZoneKind::GateFlight, ZoneKind::Rings] {
        let mut app = app(1);
        app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
            kind,
            course: Some(1),
            gate: None,
            destination: 1,
        });
        ticks(&mut app, 1);
        app.world_mut()
            .resource_mut::<Game>()
            .bonus
            .as_mut()
            .unwrap()
            .phase = Phase::Success(151);
        for (duration, expected) in [(151, Phase::Victory(401)), (401, Phase::Fade(151))] {
            ticks(&mut app, duration - 1);
            assert!(app.world().resource::<Game>().bonus.is_some());
            ticks(&mut app, 1);
            assert_eq!(
                app.world().resource::<Game>().bonus.as_ref().unwrap().phase,
                expected
            );
        }
        ticks(&mut app, 150);
        assert_eq!(app.world().resource::<Game>().score, 20_000);
        assert!(app.world().resource::<Game>().bonus.is_some());
        ticks(&mut app, 1);
        assert!(app.world().resource::<Game>().bonus.is_none());
        assert_eq!(app.world().resource::<Level>().number, 6);
    }
}

#[test]
fn restart_cancels_bonus_state_counters_and_pending_rewards() {
    let mut app = app(5);
    enter(&mut app, 4, Some(8));
    app.world_mut()
        .resource_mut::<Game>()
        .bonus
        .as_mut()
        .unwrap()
        .phase = Phase::Success(1);
    app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Restart);
    // The loader also runs in Update so restart can take precedence over fixed bonus ticks.
    app.world_mut().run_system_cached(game::load_level).unwrap();
    assert!(app.world().resource::<Game>().bonus.is_none());
    assert_eq!(app.world().resource::<Game>().bonus_attempts, [0; 3]);
    assert_eq!(app.world().resource::<Level>().number, 1);
    assert_eq!(app.world().resource::<Game>().score, 0);
}

#[test]
fn held_normal_controls_cannot_move_or_fire_the_hidden_glider_during_bonus() {
    let mut app = app(1);
    enter(&mut app, 0, None);
    let pose = *app
        .world_mut()
        .query_filtered::<&Transform, With<Glider>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowRight);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.add_systems(
        FixedUpdate,
        crate::glider::move_glider.run_if(game::controlling),
    );
    ticks(&mut app, 20);
    assert_eq!(
        *app.world_mut()
            .query_filtered::<&Transform, With<Glider>>()
            .single(app.world())
            .unwrap(),
        pose
    );
    assert!(
        !app.world_mut()
            .query::<&Weapon>()
            .single(app.world())
            .unwrap()
            .firing
    );
}

#[test]
fn returning_restores_camera_rotation_projection_and_clears_powerups() {
    let mut app = app(1);
    let original = Projection::Perspective(PerspectiveProjection {
        fov: 0.8,
        near: 0.2,
        far: 250.,
        ..default()
    });
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            original.clone(),
            Transform::from_xyz(1., 2., 3.).with_rotation(Quat::from_rotation_z(0.2)),
        ))
        .id();
    let rotation = app.world().get::<Transform>(camera).unwrap().rotation;
    app.world_mut().resource_mut::<Upgrades>().laser = true;
    enter(&mut app, 1, None);
    assert!(app.world().get::<render::SavedCamera>(camera).is_some());
    app.world_mut()
        .get_mut::<Transform>(camera)
        .unwrap()
        .rotation = Quat::from_rotation_x(1.);
    if let Projection::Perspective(p) = &mut *app.world_mut().get_mut::<Projection>(camera).unwrap()
    {
        p.fov = 1.2;
        p.near = 0.01;
    }
    app.world_mut()
        .resource_mut::<Game>()
        .bonus
        .as_mut()
        .unwrap()
        .phase = Phase::Failure(1);
    ticks(&mut app, 1);
    assert_eq!(
        app.world().get::<Transform>(camera).unwrap().rotation,
        rotation
    );
    let Projection::Perspective(p) = app.world().get::<Projection>(camera).unwrap() else {
        panic!()
    };
    assert_eq!((p.fov, p.near, p.far), (0.8, 0.2, 250.));
    assert!(app.world().get::<render::SavedCamera>(camera).is_none());
    assert!(!app.world().resource::<Upgrades>().laser);
}

#[test]
fn pending_restart_prevents_bonus_reward_settlement() {
    let mut app = app(1);
    enter(&mut app, 0, None);
    app.world_mut()
        .resource_mut::<Game>()
        .bonus
        .as_mut()
        .unwrap()
        .phase = Phase::Success(1);
    app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Restart);
    ticks(&mut app, 1);
    assert_eq!(app.world().resource::<Game>().score, 0);
    assert!(app.world().resource::<Game>().status == Status::Entering);
}

#[test]
fn ready_tokens_wait_until_the_exit_flash_finishes() {
    use crate::animations::level_exit::{self, Exit};
    let mut app = app(1);
    app.world_mut()
        .resource_mut::<Upgrades>()
        .seed_warp_tokens(3);
    let position = app
        .world_mut()
        .query_filtered::<&Transform, With<Glider>>()
        .single(app.world())
        .unwrap()
        .translation;
    let web = app.world().resource::<Level>().web.clone();
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::Exiting;
    game.exit = Some(Exit::new(position, &web));
    app.add_systems(FixedUpdate, level_exit::animate.before(advance));
    for _ in 0..1000 {
        if app
            .world()
            .resource::<Game>()
            .exit
            .is_some_and(|exit| exit.flash == 1)
        {
            break;
        }
        ticks(&mut app, 1);
        assert!(app.world().resource::<Game>().status == Status::Exiting);
        assert_eq!(app.world().resource::<Upgrades>().warp_tokens(), 3);
        assert_eq!(app.world().resource::<Game>().completed_level, 0);
    }
    assert!(app.world().resource::<Game>().status == Status::Exiting);
    assert_eq!(app.world().resource::<Game>().exit.unwrap().flash, 1);
    assert_eq!(app.world().resource::<Upgrades>().warp_tokens(), 3);
    ticks(&mut app, 1);
    assert!(app.world().resource::<Game>().status == Status::Bonus);
    assert_eq!(app.world().resource::<Upgrades>().warp_tokens(), 0);
    assert_eq!(app.world().resource::<Game>().completed_level, 1);
    assert_eq!(
        app.world().resource::<Game>().bonus.as_ref().unwrap().tick,
        0
    );
}

#[test]
fn spike_death_during_exit_does_not_spend_ready_tokens() {
    use crate::animations::level_exit::{self, Exit};
    use crate::enemies::spikers::Spike;
    let mut app = app(1);
    app.world_mut()
        .resource_mut::<Upgrades>()
        .seed_warp_tokens(3);
    let position = app
        .world_mut()
        .query_filtered::<&Transform, With<Glider>>()
        .single(app.world())
        .unwrap()
        .translation;
    let web = app.world().resource::<Level>().web.clone();
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::Exiting;
    game.exit = Some(Exit::new(position, &web));
    app.world_mut().spawn(Spike {
        lane: 7,
        length: 50 * 65536,
    });
    app.add_systems(FixedUpdate, level_exit::animate.before(advance));
    ticks(&mut app, 150);
    assert!(app.world().resource::<Game>().bonus.is_none());
    assert_eq!(app.world().resource::<Upgrades>().warp_tokens(), 3);
    assert_eq!(app.world().resource::<Game>().completed_level, 0);
}
#[test]
fn warp_announcements_follow_original_triggers_without_repeated_frame_messages() {
    use crate::messages::Id;
    let mut gate = Session::new(ZoneKind::GateFlight, 0, Some(1), 0);
    aim(&mut gate, 5, 0, 0);
    gate.pass(5);
    assert!(
        gate.announcements
            .iter()
            .any(|event| event.id == Id::SpeedBoost)
    );
    assert!(
        gate.announcements
            .iter()
            .any(|event| { event.id == Id::Points750 && event.origin.is_none() })
    );
    let mut rings = Session::new(ZoneKind::Rings, 0, Some(1), 0);
    aim(&mut rings, 5, 0, 0);
    rings.pass(5);
    assert!(
        rings.announcements.is_empty(),
        "psycho suppresses the speed-boost text and score graphic"
    );
    let mut app = app(17);
    app.world_mut().resource_mut::<Game>().load = Some(LoadReason::Warp {
        kind: ZoneKind::Tunnel,
        course: Some(1),
        gate: None,
        destination: 16,
    });
    ticks(&mut app, 1);
    assert_eq!(
        app.world()
            .resource::<Game>()
            .announcements
            .iter()
            .filter(|event| event.id == Id::GreenTrack)
            .count(),
        1
    );
    ticks(&mut app, 10);
    assert_eq!(
        app.world()
            .resource::<Game>()
            .announcements
            .iter()
            .filter(|event| event.id == Id::GreenTrack)
            .count(),
        1
    );
}
