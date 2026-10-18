//! Checks original level geometry and wave settings, open-web topology, and TOML validation.

use crate::level::Level;
use crate::web::FRONT_RADIUS;
use crate::web::LANE_UNIT;
use bevy::prelude::*;

#[test]
fn all_levels_follow_the_rom_initialized_tempest_2000_web_order() {
    // orig/t2k.jag, table at ROM offset 0x19cf0, entries 16..47. Hashes cover
    // topology, clockwise XY vertices, and lane orientations modulo 256.
    let retail: [(u8, u64, usize); 32] = [
        (5, 0xb1e4b244e977932c, 8),
        (11, 0xa48c85087e67811c, 10),
        (1, 0x0f75cb467d0be385, 5),
        (2, 0x90f54b1bf2b0ead3, 7),
        (9, 0x9983a9dd093d715e, 9),
        (3, 0x5f04ceb554acd934, 7),
        (18, 0x817db7efa850223c, 5),
        (19, 0xbf575411dc88ff57, 8),
        (20, 0x476a59f9f07cf7d3, 6),
        (4, 0x66e3eb617d4e5adc, 8),
        (14, 0xd1e046a29b46fd3a, 5),
        (10, 0x8c751dbfb31246ec, 8),
        (17, 0x596f67446496e6fc, 5),
        (6, 0x6f7cd2566934920d, 8),
        (16, 0x0ec604f5dd0664c5, 7),
        (22, 0x202dccfb3c2f629c, 8),
        (24, 0xb2743b2b40e88417, 13),
        (23, 0xd2547928ccff2e9a, 11),
        (27, 0xc8df29e211f4caf4, 7),
        (25, 0x7a6e63ee9fc6e94f, 2),
        (21, 0x052af197df72280c, 4),
        (28, 0x0c5e4b7d14292509, 6),
        (29, 0xbf60f4850e766b75, 12),
        (30, 0xb9a3b3779fb5d88d, 4),
        (32, 0xccf1daf8c5550edf, 7),
        (33, 0x1ab2530b7ba3833c, 8),
        (34, 0xb5ba01ef5488a70a, 7),
        (35, 0x904a506841ecf8b7, 3),
        (36, 0x72b20fcb1aef4c7d, 9),
        (37, 0x163ec736505dc555, 12),
        (26, 0x9403e5dd51a49c1c, 9),
        (31, 0xdd629e8ffa6b44ca, 13),
    ];
    let levels = crate::test_support::levels();
    assert_eq!(levels.0.len(), 100);
    for level in &levels.0 {
        let index = (level.number - 1) % 32;
        let (web, expected, start) = retail[index];
        let bytes = std::iter::once(u8::from(level.web.closed))
            .chain(
                level
                    .web
                    .vertices
                    .iter()
                    .flatten()
                    .flat_map(|value| value.to_bits().to_le_bytes()),
            )
            .chain(
                level
                    .web
                    .orientations
                    .iter()
                    .map(|angle| angle.rem_euclid(256) as u8),
            );
        let actual = bytes.fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        assert_eq!(
            actual, expected,
            "level {} should use retail web{web}",
            level.number
        );
        assert_eq!(
            level.web.start_lane,
            if level.number == 1 { 7 } else { start }
        );
        if !(49..=64).contains(&level.number) {
            assert_eq!(
                level.appearance.surface_hues,
                levels.0[index].appearance.surface_hues
            );
        }
    }
}

#[test]
fn surface_hues_require_one_byte_per_lane_or_an_explicit_empty_palette() {
    let source = include_str!("../../levels/05.toml");
    for palette in ["[]", "[1]", "[256]", "[\"purple\"]"] {
        let source = source
            .lines()
            .map(|line| {
                if line.starts_with("surface_hues =") {
                    format!("surface_hues = {palette}")
                } else {
                    line.into()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            Level::parse(&source, "palette-test").is_ok(),
            palette == "[]"
        );
    }
    let source = source
        .lines()
        .filter(|line| !line.starts_with("surface_hues ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        Level::parse(&source, "rgb-fallback")
            .unwrap()
            .appearance
            .surface_hues
            .is_none()
    );
}

#[test]
fn original_level_twenty_has_the_open_hook_and_a_thirty_three_spawn_budget() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/20.toml")),
        "level20",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 12);
    assert_eq!(level.web.start_lane, 2);
    assert_eq!(level.web.vertices[0], [13., 11.]);
    assert_eq!(level.web.vertices[12], [8., -3.]);
    assert_eq!(level.web.neighbor(0, -1), None);
    assert_eq!(level.web.neighbor(11, 1), None);
    assert_eq!(
        level.web.orientations,
        [-96, -112, -128, 112, 96, 80, 64, 48, 32, 32, 48, 64]
    );
    assert_eq!(level.flippers.count, 33);
    assert_eq!(level.flippers.first_spawn_ticks, 41);
    assert_eq!(level.flippers.spawn_interval_ticks, 81);
    assert_eq!(level.flippers.climb_speed, 0xaa00);
    let tankers = level.tankers.unwrap();
    assert_eq!(tankers.first_spawn_ticks, 167);
    assert_eq!(tankers.spawn_interval_ticks, 334);
    assert_eq!(tankers.climb_speed, 0x8000);
    assert_eq!(tankers.super_chance, 76);
    let pulsars = level.pulsars.unwrap();
    assert_eq!(pulsars.generator.first_spawn_ticks, 251);
    assert_eq!(pulsars.generator.spawn_interval_ticks, 501);
    assert_eq!(pulsars.phase_ticks, 9);
    assert_eq!(pulsars.propagation_ticks, 5);
    assert_eq!(level.spikers.unwrap().climb_speed, 0xd000);
    assert_eq!(level.beasts.unwrap().climb_speed, 0xaa00);
    assert_eq!(level.fuseballs.unwrap().climb_speed, 0x9800);
    assert_eq!(level.enemy_shots, 3);
    assert_eq!(level.powerups.kill_interval, 7);
}

#[test]
fn original_level_nineteen_has_the_pentagon_and_restores_flipper_tankers() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/19.toml")),
        "level19",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 15);
    assert_eq!(level.web.start_lane, 7);
    assert_eq!(level.web.vertices[0], [8., 2.]);
    assert_eq!(level.web.vertices[7], [9., 14.]);
    assert_eq!(level.web.neighbor(14, 1), Some(0));
    assert_eq!(level.web.flip_angle(2, 3, 1), 80);
    assert_eq!(level.flippers.count, 31);
    assert_eq!(level.flippers.first_spawn_ticks, 61);
    assert_eq!(level.flippers.spawn_interval_ticks, 121);
    assert_eq!(level.flippers.climb_speed, 0xaa00);
    let tankers = level.tankers.unwrap();
    assert_eq!(tankers.first_spawn_ticks, 223);
    assert_eq!(tankers.spawn_interval_ticks, 445);
    assert_eq!(tankers.climb_speed, 0x8000);
    assert_eq!(tankers.super_chance, 72);
    let pulsars = level.pulsars.unwrap();
    assert_eq!(pulsars.generator.first_spawn_ticks, 301);
    assert_eq!(pulsars.generator.spawn_interval_ticks, 601);
    assert_eq!(pulsars.phase_ticks, 9);
    assert_eq!(level.spikers.unwrap().climb_speed, 0xd000);
    assert_eq!(level.beasts.unwrap().climb_speed, 0xaa00);
    assert_eq!(level.fuseballs.unwrap().climb_speed, 0x9800);
    assert_eq!(level.enemy_shots, 3);
    assert_eq!(level.powerups.kill_interval, 7);
}

#[test]
fn original_level_eighteen_has_the_open_spiraloid_and_a_denser_pulsar_wave() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/18.toml")),
        "level18",
    )
    .unwrap();
    assert_eq!(level.web.lanes(), 15);
    assert!(!level.web.closed);
    assert_eq!(level.web.start_lane, 11);
    assert_eq!(level.web.vertices[0], [7., 6.]);
    assert_eq!(level.web.vertices[15], [1., 9.]);
    assert_eq!(level.web.neighbor(0, -1), None);
    assert_eq!(level.web.neighbor(14, 1), None);
    assert_eq!(
        level.web.orientations,
        [
            112, 64, 32, 0, -16, -48, -48, -64, -80, -96, -112, -128, 112, 112, 80
        ]
    );
    assert_eq!(level.flippers.count, 31);
    assert_eq!(level.flippers.first_spawn_ticks, 51);
    assert_eq!(level.flippers.spawn_interval_ticks, 101);
    assert_eq!(level.flippers.climb_speed, 0xa800);
    assert_eq!(level.enemy_shots, 3);
    assert!(level.tankers.is_none());
    let pulsars = level.pulsars.unwrap();
    assert_eq!(pulsars.generator.first_spawn_ticks, 251);
    assert_eq!(pulsars.generator.spawn_interval_ticks, 501);
    assert_eq!(pulsars.generator.climb_speed, 0x8000);
    assert_eq!(pulsars.phase_ticks, 9);
    assert_eq!(pulsars.propagation_ticks, 5);
    assert_eq!(level.spikers.unwrap().climb_speed, 0xd000);
    assert_eq!(level.beasts.unwrap().spawn_interval_ticks, 1001);
    assert_eq!(level.fuseballs.unwrap().spawn_interval_ticks, 901);
    assert_eq!(level.appearance.flipper, [135, 208, 255]);
    assert_eq!(level.powerups.kill_interval, 7);
}

#[test]
fn original_level_seventeen_starts_the_red_oval_and_pulsar_wave() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/17.toml")),
        "level17",
    )
    .unwrap();
    assert_eq!(level.web.lanes(), 18);
    assert!(level.web.closed);
    assert_eq!(level.web.start_lane, 13);
    assert_eq!(level.web.vertices[0], [1., 8.]);
    assert_eq!(level.web.vertices[9], [15., 10.]);
    assert_eq!(
        level.web.orientations,
        [
            48, 16, 16, 0, 0, -16, -16, -48, -64, -80, -112, -112, -128, -128, 112, 112, 80, 64
        ]
    );
    assert_eq!(level.flippers.count, 25);
    assert_eq!(level.flippers.climb_speed, 0xa800);
    assert_eq!(level.flippers.spawn_interval_ticks, 121);
    assert_eq!(level.enemy_shots, 3);
    assert!(level.tankers.is_none());
    let pulsars = level.pulsars.unwrap();
    assert_eq!(pulsars.generator.first_spawn_ticks, 301);
    assert_eq!(pulsars.generator.spawn_interval_ticks, 601);
    assert_eq!(pulsars.generator.climb_speed, 0x8000);
    assert_eq!(pulsars.phase_ticks, 9);
    assert_eq!(pulsars.propagation_ticks, 5);
    assert_eq!(level.spikers.unwrap().climb_speed, 0xd000);
    assert_eq!(level.beasts.unwrap().spawn_interval_ticks, 1001);
    assert_eq!(level.fuseballs.unwrap().spawn_interval_ticks, 901);
    assert_eq!(level.appearance.flipper, [135, 208, 255]);
    assert_eq!(level.powerups.kill_interval, 7);
    for bad in ["phase_ticks = 0", "propagation_ticks = 0"] {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/17.toml"));
        let field = bad.split(" = ").next().unwrap();
        let valid = if field == "phase_ticks" {
            "phase_ticks = 9"
        } else {
            "propagation_ticks = 5"
        };
        assert!(
            Level::parse(&source.replace(valid, bad), "bad-pulsar")
                .err()
                .unwrap()
                .contains("must be positive")
        );
    }
}

#[test]
fn original_level_five_uses_the_triangle_and_faster_climbers() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/05.toml")),
        "level5",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 18);
    assert_eq!(level.web.start_lane, 9);
    assert_eq!(level.web.flip_angle(5, 6, 1), 48);
    assert_eq!(level.web.flip_angle(17, 0, 1), 32);
    assert_eq!(level.flippers.climb_speed, 0xa400);
    assert_eq!(level.tankers.as_ref().unwrap().climb_speed, 0x8800);
    assert_eq!(level.spikers.as_ref().unwrap().climb_speed, 0xe000);
    assert_eq!(level.powerups.kill_interval, 6);
}

#[test]
fn original_level_six_has_an_open_u_and_the_thirty_one_spawn_budget() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/06.toml")),
        "level6",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 15);
    assert_eq!(level.flippers.count, 31);
    assert_eq!(
        (
            level.flippers.first_spawn_ticks,
            level.flippers.spawn_interval_ticks
        ),
        (48, 96)
    );
    assert_eq!(level.tankers.as_ref().unwrap().spawn_interval_ticks, 261);
    assert_eq!(level.web.neighbor(14, 1), None);
    assert_eq!(level.web.neighbor(0, -1), None);
    assert_eq!(level.web.flip_angle(6, 7, 1), 128);
    assert_eq!(level.web.flip_angle(2, 3, 1), 112);
}

#[test]
fn original_level_seven_selects_the_tempest_2000_lemon_and_its_speed_group() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/07.toml")),
        "level7",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 14);
    assert_eq!(level.web.start_lane, 5);
    assert_eq!(level.web.vertices[0], [12., 3.]);
    assert_eq!(level.web.vertices[7], [3., 12.]);
    assert_eq!(level.flippers.climb_speed, 0xa600);
    assert_eq!(level.spikers.as_ref().unwrap().climb_speed, 0xf000);
    assert_eq!(
        (
            level.tankers.as_ref().unwrap().first_spawn_ticks,
            level.tankers.as_ref().unwrap().spawn_interval_ticks
        ),
        (126, 251)
    );
    assert_eq!(level.web.neighbor(13, 1), Some(0));
    assert_eq!(level.web.flip_angle(13, 0, 1), 64);
}

#[test]
fn original_level_eight_uses_the_retail_wide_v_with_denser_tankers_and_spikers() {
    let seven = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/07.toml")),
        "level7",
    )
    .unwrap();
    let eight = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/08.toml")),
        "level8",
    )
    .unwrap();
    assert_ne!(seven.web.vertices, eight.web.vertices);
    assert!(!eight.web.closed);
    assert_eq!(eight.web.lanes(), 14);
    assert_eq!(eight.web.start_lane, 8);
    assert_eq!(eight.web.vertices[0], [16., 4.]);
    assert_eq!(eight.web.vertices[14], [-3., 3.]);
    assert_eq!(eight.flippers.count, 31);
    assert_eq!(
        (
            eight.tankers.as_ref().unwrap().first_spawn_ticks,
            eight.tankers.as_ref().unwrap().spawn_interval_ticks
        ),
        (121, 241)
    );
    assert_eq!(
        (
            eight.spikers.as_ref().unwrap().first_spawn_ticks,
            eight.spikers.as_ref().unwrap().spawn_interval_ticks
        ),
        (326, 651)
    );
    assert_eq!(eight.tankers.as_ref().unwrap().super_chance, 28);
    assert!(crate::test_support::levels().0.len() >= 8);
}

#[test]
fn original_level_nine_has_the_retail_giraffe_neck_and_two_enemy_shots() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/09.toml")),
        "level9",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 13);
    assert_eq!(level.web.vertices[0], [14., -3.]);
    assert_eq!(level.web.vertices[13], [-1., 16.]);
    assert_eq!(level.enemy_shots, 2);
    assert_eq!(level.flippers.climb_speed, 0xa800);
    assert_eq!(level.spikers.as_ref().unwrap().climb_speed, 0x10000);
    assert_eq!(level.tankers.as_ref().unwrap().spawn_interval_ticks, 231);
    assert_eq!(level.powerups.kill_interval, 7);
}

#[test]
fn original_level_ten_has_the_clover_and_beast_generator() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/10.toml")),
        "level10",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 16);
    assert_eq!(level.web.vertices[0], [8., 6.]);
    assert_eq!(level.web.orientations[4], -16);
    assert_eq!(
        (
            level.beasts.as_ref().unwrap().first_spawn_ticks,
            level.beasts.as_ref().unwrap().spawn_interval_ticks
        ),
        (1001, 2001)
    );
}

#[test]
fn original_level_twelve_has_the_cross_and_faster_fuseball_generator() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/12.toml")),
        "level12",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 16);
    assert_eq!(level.web.vertices[0], [7., 3.]);
    assert_eq!(level.flippers.climb_speed, 0xaa00);
    assert_eq!(level.fuseballs.as_ref().unwrap().spawn_interval_ticks, 701);
    assert_eq!(level.web.flip_angle(1, 2, 1), 160);
}

#[test]
fn original_level_thirteen_selects_the_retail_wobbly_u_and_next_speed_group() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/13.toml")),
        "level13",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 12);
    assert_eq!(level.web.vertices[0], [15., 1.]);
    assert_eq!(level.web.vertices[12], [6., 1.]);
    assert_eq!(level.flippers.climb_speed, 0xac00);
    assert_eq!(level.spikers.as_ref().unwrap().climb_speed, 0x12000);
    assert_eq!(level.fuseballs.as_ref().unwrap().climb_speed, 0x8800);
}

#[test]
fn original_level_fourteen_has_the_distorted_w_and_shorter_flipper_interval() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/14.toml")),
        "level14",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 15);
    assert_eq!(level.web.start_lane, 8);
    assert_eq!(level.flippers.spawn_interval_ticks, 76);
    assert_eq!(level.web.vertices[5], [13., 11.]);
    assert_eq!(level.tankers.as_ref().unwrap().super_chance, 52);
}

#[test]
fn original_level_fifteen_uses_web16_and_the_last_early_speed_group() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/15.toml")),
        "level15",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 15);
    assert_eq!(level.flippers.climb_speed, 0xae00);
    assert_eq!(level.spikers.as_ref().unwrap().climb_speed, 0x13000);
    assert_eq!(level.fuseballs.as_ref().unwrap().climb_speed, 0x9000);
    assert_eq!(level.fuseballs.as_ref().unwrap().spawn_interval_ticks, 501);
}

#[test]
fn original_level_sixteen_selects_the_retail_twelve_lane_star_and_forty_one_spawns() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/16.toml")),
        "level16",
    )
    .unwrap();
    assert!(level.web.closed);
    assert_eq!(level.web.lanes(), 12);
    assert_eq!(level.web.start_lane, 8);
    assert_eq!(level.flippers.count, 41);
    assert_eq!(level.flippers.spawn_interval_ticks, 71);
    assert_eq!(level.tankers.as_ref().unwrap().super_chance, 60);
    assert_eq!(level.enemy_shots, 2);
    assert_eq!(level.web.flip_angle(11, 0, 1), 144);
}

#[test]
fn open_plane_has_eleven_lanes_and_never_wraps_at_its_ends() {
    let level = Level::parse(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/03.toml")),
        "level3",
    )
    .unwrap();
    assert!(!level.web.closed);
    assert_eq!(level.web.lanes(), 11);
    assert_eq!(level.web.neighbor(0, -1), None);
    assert_eq!(level.web.neighbor(10, 1), None);
    assert_eq!(level.web.normalize(-LANE_UNIT), 0);
    assert_eq!(level.web.normalize(20 * LANE_UNIT), 10 * LANE_UNIT);
    assert_eq!(level.web.lane_delta(0, 10 * LANE_UNIT), 10 * LANE_UNIT);
    assert_eq!(level.web.vertex(11., FRONT_RADIUS, 0.).x, -22.0 / 3.0);
}

#[test]
fn original_levels_have_the_expected_geometry_and_wave_settings() {
    let levels = crate::test_support::levels();
    assert!(levels.0.len() >= 2);
    let first = &levels.0[0];
    let second = &levels.0[1];
    assert_eq!(
        (
            first.flippers.count,
            first.flippers.first_spawn_ticks,
            first.flippers.spawn_interval_ticks
        ),
        (25, 61, 121)
    );
    assert_eq!(
        (
            second.flippers.count,
            second.flippers.first_spawn_ticks,
            second.flippers.spawn_interval_ticks
        ),
        (25, 51, 101)
    );
    assert_eq!(second.flippers.climb_speed, 0xa000);
    assert_eq!(second.web.lanes(), 16);
    assert_eq!(
        second.web.glider_transform(second.web.start()).translation,
        Vec3::new(-2.0 / 3.0, -8.0 / 3.0, 0.12)
    );
    for lane in 0..16 {
        let next = (lane + 1) % 16;
        assert_eq!(
            second.web.flip_angle(lane, next, 1),
            if lane % 4 == 3 { 64 } else { 128 }
        );
        assert_eq!(
            second.web.flip_angle(next, lane, -1),
            if lane % 4 == 3 { 64 } else { 128 }
        );
        assert_eq!(first.web.flip_angle(lane, next, 1), 112);
    }
}

#[test]
fn invalid_tweaks_report_the_file_and_reject_unknown_fields() {
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/01.toml"));
    for (old, new) in [
        ("start_lane = 7", "start_lane = 99"),
        ("depth = 160", "depth = 0"),
        ("first_spawn_ticks = 61", "first_spawn_ticks = 0"),
        ("climb_speed = 0.625", "climb_speed = nan"),
        ("flip_speed = 4", "flip_speed = 0"),
        ("kill_interval = 5", "kill_interval = 0"),
        ("spawn_interval_ticks = 121", "spawn_interva_ticks = 121"),
        ("[9, 2]", "[7, 2]"),
    ] {
        let error = Level::parse(&source.replace(old, new), "bad-level.toml")
            .err()
            .expect("invalid definition must fail");
        assert!(error.starts_with("bad-level.toml:"), "{error}");
    }
}

#[test]
fn normal_difficulty_is_exactly_the_identity_on_a_parsed_level() {
    use crate::difficulty::Difficulty;
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/04.toml"));
    let original = Level::parse(source, "level4").unwrap();
    let mut scaled = original.clone();
    scaled.scale_difficulty(Difficulty::Normal.scale());
    assert_eq!(scaled.flippers.climb_speed, original.flippers.climb_speed);
    assert_eq!(
        scaled.flippers.spawn_interval_ticks,
        original.flippers.spawn_interval_ticks
    );
    assert_eq!(scaled.enemy_shots, original.enemy_shots);
    let spiker = original.spikers.unwrap().clone();
    let scaled_spiker = scaled.spikers.unwrap();
    assert_eq!(scaled_spiker.climb_speed, spiker.climb_speed);
    assert_eq!(
        scaled_spiker.spawn_interval_ticks,
        spiker.spawn_interval_ticks
    );
}

#[test]
fn low_and_high_scale_enemy_tempo_in_opposite_directions() {
    use crate::difficulty::Difficulty;
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/levels/04.toml"));
    let mut low = Level::parse(source, "level4").unwrap();
    let mut high = Level::parse(source, "level4").unwrap();
    low.scale_difficulty(Difficulty::Easy.scale());
    high.scale_difficulty(Difficulty::Hard.scale());
    for (level, name) in [(&low, "low"), (&high, "high")] {
        assert!(
            level.flippers.climb_speed > 0,
            "{name} zeroed the climb speed"
        );
        assert!(
            level.flippers.spawn_interval_ticks >= 1,
            "{name} zeroed the interval"
        );
        for generator in [
            level.spikers.as_ref(),
            level.beasts.as_ref(),
            level.aliens.as_ref(),
            level.tankers.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(generator.climb_speed > 0, "{name} zeroed a climb speed");
            assert!(
                generator.spawn_interval_ticks >= 1,
                "{name} produced an illegal zero interval"
            );
        }
        assert!(
            (1..=16).contains(&level.enemy_shots),
            "{name} broke the shot range"
        );
    }
    assert!(low.flippers.climb_speed < high.flippers.climb_speed);
    assert!(low.flippers.spawn_interval_ticks > high.flippers.spawn_interval_ticks);
    assert!(low.enemy_shots < high.enemy_shots);
}
