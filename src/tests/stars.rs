//! Checks star projection, depth wrapping, brightness, population, and warp trail samples.

use crate::mesh::colored_mesh;
use crate::stars::COUNT;
use crate::stars::Star;
use crate::stars::StarField;
use crate::stars::Stars;
use crate::stars::Warp;
use crate::stars::draw_stars;
use crate::web::LANE_UNIT;
use bevy::prelude::*;

#[test]
fn original_level_patterns_have_source_coordinates_palette_and_level32_exception() {
    use crate::stars::Pattern;
    for (level, point) in [
        (1, Vec2::new(6., -27.)),
        (2, Vec2::new(12., 126.)),
        (32, Vec2::new(124., 126.)),
    ] {
        let stars = Pattern::Level.stars(level);
        assert_eq!(stars.len(), 256);
        assert_eq!(stars[0].position, Vec2::new(0., -126.));
        assert_eq!(stars[1].position, point);
        assert_eq!((stars[0].depth, stars[255].depth), (510 * LANE_UNIT, 0));
        assert_eq!(stars[0].color, crate::cry::color(0x8f, 255));
    }
    let special = Pattern::Level.stars(32);
    let reference = Pattern::Level.stars(164);
    assert!(
        special
            .iter()
            .zip(reference)
            .all(|(a, b)| a.position == b.position && a.depth == b.depth && a.color == b.color)
    );
    let first = Pattern::Level.stars(1);
    for level in [2, 3, 8, 16, 32, 48, 64, 80, 100] {
        assert!(
            first
                .iter()
                .zip(Pattern::Level.stars(level))
                .any(|(a, b)| a.position != b.position),
            "level {level} lost its distinct pattern"
        );
    }
}

#[test]
fn original_preview_variants_have_their_own_population_motion_and_trails() {
    use crate::stars::Pattern;
    let rings = Pattern::Rings.stars(1);
    assert_eq!(rings.len(), 256);
    assert!(rings.iter().all(|star| star.position.length() < 50.));
    assert_eq!(Pattern::Rings.step(), 0x1c000);
    assert_eq!(Pattern::Rings.trails(4), (6, 2 * LANE_UNIT));
    assert_eq!(Pattern::Selection.step(), 6 * LANE_UNIT);
    assert_eq!(Pattern::Selection.trails(4), (4, 26 * LANE_UNIT));
    let selection = Pattern::Selection.stars(2);
    assert_eq!(
        selection[4].position.x, 73.,
        "fractional quarter-angle steps must accumulate before sine lookup"
    );
}

#[test]
fn web_loading_changes_the_pattern_preserves_motion_and_restart_resets_phase() {
    use crate::stars::{Animation, advance_stars};
    use crate::{game::Game, level::Level};
    let mut app = App::new();
    app.init_resource::<Stars>()
        .init_resource::<Animation>()
        .init_resource::<Level>()
        .init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .add_systems(Update, advance_stars);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Animation>().offset, 6 * LANE_UNIT);
    app.world_mut().resource_mut::<Level>().number = 2;
    app.update();
    assert_eq!(
        app.world().resource::<Stars>().0[1].position,
        Vec2::new(12., 126.)
    );
    assert_eq!(app.world().resource::<Stars>().0[0].depth, 502 * LANE_UNIT);
    let level = app.world().resource::<Level>().clone();
    app.insert_resource(level);
    app.update();
    assert_eq!(app.world().resource::<Stars>().0[0].depth, 500 * LANE_UNIT);
    app.world_mut().resource_mut::<Game>().message_epoch += 1;
    app.update();
    assert_eq!(app.world().resource::<Stars>().0[0].depth, 508 * LANE_UNIT);
    assert_eq!(app.world().resource::<Animation>().offset, 2 * LANE_UNIT);
}

#[test]
fn entry_replays_exit_speed_and_warp_streaks_in_reverse_on_every_web() {
    use crate::{
        animations::{level_entry::APPROACH_TICKS, level_exit::Exit},
        game::{Game, Status},
        level::Level,
        stars::{Animation, DEPTH_WRAP, STEP, advance_stars},
    };
    let mut app = App::new();
    app.init_resource::<Stars>()
        .init_resource::<Animation>()
        .init_resource::<Level>()
        .init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .add_systems(Update, advance_stars);
    for level in [1, 2, 32] {
        app.world_mut().resource_mut::<Level>().number = level;
        let web = &app.world().resource::<Level>().web;
        let mut exit = Exit::new(web.glider_transform(web.start()).translation, web);
        let mut exit_frames = Vec::new();
        while exit.flash == 0 {
            exit.advance();
            exit_frames.push((exit.speed, i32::from(exit.ticks) * 0x4000));
        }
        for (tick, (speed, spacing)) in exit_frames.into_iter().rev().enumerate() {
            let before = app.world().resource::<Animation>().offset;
            {
                let mut game = app.world_mut().resource_mut::<Game>();
                game.status = if tick < usize::from(APPROACH_TICKS) {
                    Status::Entering
                } else {
                    Status::Playing
                };
                game.entry = (tick < usize::from(APPROACH_TICKS + 64)).then_some(tick as u16);
            }
            app.update();
            let step = (app.world().resource::<Animation>().offset - before).rem_euclid(DEPTH_WRAP);
            assert_eq!(step, STEP + speed);
            let animation = app.world().resource::<Animation>();
            assert_eq!(animation.entry_warp.copies, 3);
            assert_eq!(animation.entry_warp.spacing, spacing);
            let expected_depth = (510 * LANE_UNIT - app.world().resource::<Animation>().offset)
                .rem_euclid(DEPTH_WRAP);
            assert_eq!(app.world().resource::<Stars>().0[0].depth, expected_depth);
        }
        let before = app.world().resource::<Animation>().offset;
        app.update();
        let animation = app.world().resource::<Animation>();
        assert_eq!((animation.offset - before).rem_euclid(DEPTH_WRAP), STEP);
        assert_eq!(animation.entry_warp.copies, 0);
    }
}

#[test]
fn bonus_and_menu_starfields_do_not_get_the_web_entry_boost() {
    use crate::{
        game::{Game, Status},
        level::Level,
        stars::{Animation, Pattern, STEP, advance_stars},
    };
    for (pattern, status, expected) in [
        (Pattern::Level, Status::Bonus, STEP),
        (Pattern::Rings, Status::Entering, 0x1c000),
        (Pattern::Selection, Status::Entering, 6 * LANE_UNIT),
    ] {
        let mut app = App::new();
        app.init_resource::<Stars>()
            .insert_resource(Animation::new(pattern))
            .init_resource::<Level>()
            .insert_resource(Game {
                status,
                entry: Some(0),
                ..default()
            })
            .add_systems(Update, advance_stars);
        app.update();
        assert_eq!(app.world().resource::<Animation>().offset, expected);
    }
}

#[test]
fn stars_web_lanes_and_exit_flight_share_the_same_vanishing_point() {
    use crate::{
        level::Level,
        stars::BACKGROUND_DISTANCE,
        web::{FRONT_RADIUS, WORLD_UNIT},
    };
    use bevy::{camera::CameraProjection, mesh::VertexAttributeValues};

    for source in [
        include_str!("../../levels/01.toml"),
        include_str!("../../levels/03.toml"),
    ] {
        let level = Level::parse(source, "shared perspective").unwrap();
        let lane = level.web.start_lane;
        let glider = level
            .web
            .glider_transform(lane as i32 * LANE_UNIT)
            .translation;
        for flight_distance in [0., 100., 500.] {
            let mut pose = Transform::from_translation(level.web.camera_position(glider))
                .with_rotation(level.web.camera_rotation());
            pose.translation.z -= flight_distance * WORLD_UNIT;
            let projection = PerspectiveProjection {
                fov: 1.5,
                aspect_ratio: 1.6,
                ..default()
            };
            let project = |point| {
                projection
                    .get_clip_from_view()
                    .project_point3(pose.compute_affine().inverse().transform_point3(point))
                    .truncate()
            };
            let vanishing = project(pose.translation + Vec3::NEG_Z * 1_000_000.);
            let mut app = App::new();
            let mut meshes = Assets::<Mesh>::default();
            let mesh = meshes.add(colored_mesh([]));
            app.insert_resource(meshes)
                .insert_resource(Stars(vec![Star {
                    position: pose.translation.truncate() / WORLD_UNIT,
                    depth: 200 * LANE_UNIT,
                    color: Color::WHITE,
                }]))
                .init_resource::<Warp>()
                .add_systems(Update, draw_stars);
            app.world_mut().spawn((
                Camera3d::default(),
                pose,
                Projection::Perspective(projection.clone()),
            ));
            let field = app
                .world_mut()
                .spawn((StarField, Mesh3d(mesh.clone()), Transform::default()))
                .id();
            app.update();
            let field_pose = app.world().get::<Transform>(field).unwrap();
            let geometry = app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap();
            let VertexAttributeValues::Float32x3(vertices) =
                geometry.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!()
            };
            assert_eq!(vertices.len(), 6);
            let center = (Vec3::from_array(vertices[0]) + Vec3::from_array(vertices[2])) / 2.;
            assert!((center.z).abs() < 1e-6);
            assert!(
                ((field_pose.translation - pose.translation).length() - BACKGROUND_DISTANCE).abs()
                    < 1e-4
            );
            assert!(
                project(field_pose.transform_point(center)).distance(vanishing) < 1e-6,
                "star origin must match the world depth axis"
            );
            assert!(
                vanishing.length() > 0.01,
                "the tilted camera must not force vanishing to the screen center"
            );
            for vertex in 0..level.web.vertices.len() {
                let a = project(level.web.vertex(
                    vertex as f32,
                    FRONT_RADIUS,
                    -(flight_distance + 30.) * WORLD_UNIT,
                ));
                let b = project(level.web.vertex(
                    vertex as f32,
                    FRONT_RADIUS,
                    -(flight_distance + 200.) * WORLD_UNIT,
                ));
                assert!(
                    (b - a).perp_dot(vanishing - a).abs() < 1e-5,
                    "web rail does not converge with stars"
                );
            }
            let flight_a = project(Vec3::new(
                glider.x,
                glider.y,
                -(flight_distance + 100.) * WORLD_UNIT,
            ));
            let flight_b = project(Vec3::new(
                glider.x,
                glider.y,
                -(flight_distance + 200.) * WORLD_UNIT,
            ));
            assert!(
                (flight_b - flight_a).perp_dot(vanishing - flight_a).abs() < 1e-5,
                "exit flight does not converge with stars"
            );
        }
    }
}

#[test]
fn rotated_star_rays_follow_camera_projection_and_skip_points_behind_the_view() {
    use crate::stars::BACKGROUND_DISTANCE;
    use crate::web::WORLD_UNIT;
    let star = star(100 * LANE_UNIT);
    let camera = Transform::from_xyz(0.3, -0.7, 5.3);
    let legacy =
        star.projected(camera.translation.truncate() / WORLD_UNIT) * (BACKGROUND_DISTANCE / 128.);
    assert!(star.background_position(&camera).unwrap().distance(legacy) < 1e-5);
    assert!(
        star.background_position(
            &camera.with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
        )
        .is_none()
    );
}

#[test]
fn bonus_uses_original_ring_population_and_six_samples_without_steering_parallax() {
    use crate::game::{Game, Status};
    use crate::stars::BonusStars;
    use crate::warp::{Session, ZoneKind};

    let mut app = App::new();
    let mut meshes = Assets::<Mesh>::default();
    let mesh = meshes.add(colored_mesh([]));
    let mut session = Session::new(ZoneKind::GateFlight, 0, Some(1), 0);
    session.tick = 60;
    app.insert_resource(meshes)
        .insert_resource(Stars::default())
        .insert_resource(Warp::default())
        .insert_resource(BonusStars::default())
        .insert_resource(Game {
            status: Status::Bonus,
            bonus: Some(session),
            ..default()
        })
        .add_systems(Update, draw_stars);
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ))
        .id();
    app.world_mut()
        .spawn((StarField, Mesh3d(mesh.clone()), Transform::default()));
    app.update();
    let before = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .unwrap()
        .clone();
    assert_eq!(app.world().resource::<BonusStars>().0.len(), 256);
    let stars = &app.world().resource::<BonusStars>().0;
    for axis in 0..2 {
        assert!(stars.iter().any(|star| star.position[axis] > 90.));
        assert!(stars.iter().any(|star| star.position[axis] < -90.));
    }
    assert!(before.count_vertices() > COUNT * 6);
    app.world_mut()
        .get_mut::<Transform>(camera)
        .unwrap()
        .translation = Vec3::new(12., -4., 0.);
    app.update();
    assert_eq!(
        before.attribute(Mesh::ATTRIBUTE_POSITION),
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_POSITION)
    );
    app.world_mut().resource_mut::<Game>().status = Status::Playing;
    app.update();
    assert!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .count_vertices()
            <= COUNT * 6
    );
}

#[test]
fn warp_draws_four_depth_spaced_samples_and_returns_to_single_dots() {
    use crate::stars::Animation;
    for entering in [false, true] {
        let mut app = App::new();
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(colored_mesh([]));
        app.insert_resource(meshes)
            .insert_resource(Stars(vec![Star {
                position: Vec2::ZERO,
                depth: 32 * LANE_UNIT,
                color: Color::WHITE,
            }]))
            .insert_resource(Warp {
                distance: 0,
                spacing: 16 * LANE_UNIT,
                copies: 3,
            })
            .add_systems(Update, draw_stars);
        if entering {
            let warp = std::mem::take(&mut *app.world_mut().resource_mut::<Warp>());
            app.insert_resource(Animation {
                entry_warp: warp,
                ..default()
            });
        }
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        app.world_mut()
            .spawn((StarField, Mesh3d(mesh.clone()), Transform::default()));
        app.world_mut().run_schedule(Update);
        let colors = match app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_COLOR)
            .unwrap()
        {
            bevy::mesh::VertexAttributeValues::Float32x4(colors) => colors,
            _ => panic!("star mesh needs colors"),
        };
        assert_eq!(colors.len(), 24);
        assert!(
            colors
                .as_chunks::<6>()
                .0
                .iter()
                .map(|dot| dot[0][0])
                .collect::<Vec<_>>()
                .windows(2)
                .all(|pair| pair[0] > pair[1])
        );
        *app.world_mut().resource_mut::<Warp>() = Warp::default();
        if entering {
            app.world_mut().resource_mut::<Animation>().entry_warp = Warp::default();
        }
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&mesh)
                .unwrap()
                .count_vertices(),
            6
        );
    }
}

fn star(depth: i32) -> Star {
    Star {
        position: Vec2::new(30.0, -20.0),
        depth,
        color: Color::WHITE,
    }
}

#[test]
fn stars_accelerate_outward_and_brighten_as_the_viewpoint_advances() {
    let mut star = star(100 * LANE_UNIT);
    let before = star.projected(Vec2::ZERO);
    let brightness = star.color().to_srgba().red;
    star.advance(crate::stars::STEP);
    let first_step = star.projected(Vec2::ZERO) - before;
    assert!(star.projected(Vec2::ZERO).length() > before.length());
    assert!(star.color().to_srgba().red > brightness);
    let before = star.projected(Vec2::ZERO);
    star.advance(crate::stars::STEP);
    assert!((star.projected(Vec2::ZERO) - before).length() > first_step.length());
}

#[test]
fn near_stars_wrap_back_into_the_distance_without_changing_their_ray() {
    let mut star = star(LANE_UNIT);
    let direction = star.projected(Vec2::ZERO).normalize();
    star.advance(crate::stars::STEP);
    assert_eq!(star.depth, 511 * LANE_UNIT);
    assert!(star.projected(Vec2::ZERO).normalize().distance(direction) < 1e-6);
    assert!(star.projected(Vec2::ZERO).is_finite());
}

#[test]
fn motion_repeats_after_256_ticks_without_changing_the_population() {
    let mut stars = Stars::default();
    let initial: Vec<_> = stars.0.iter().map(|star| star.depth).collect();
    for _ in 0..256 {
        for star in &mut stars.0 {
            star.advance(crate::stars::STEP);
        }
    }
    assert_eq!(stars.0.len(), COUNT);
    assert_eq!(
        stars.0.iter().map(|star| star.depth).collect::<Vec<_>>(),
        initial
    );
}
