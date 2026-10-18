//! Plays whole waves headless with a scripted, invulnerable player, to compare
//! the settings: enemy pressure must rise from Easy to Hard while the rewards
//! of a wave stay the same.

use super::{Difficulty, advance_enemy_clock};
use crate::collisions::resolve_hits;
use crate::enemies::{
    Enemy, aliens, beasts, flippers, fuseballs, mirrors, pulsars, shots, spikers, tankers,
};
use crate::game::{Game, Status, check_game, playing, reset_wave};
use crate::glider::Glider;
use crate::level::Level;
use crate::powerups::{self, pickups::PowerUp};
use crate::test_support::{app, ticks};
use crate::weapons::{ShotAssets, fire_bullets, move_bullets};
use crate::web::LANE_UNIT;
use bevy::prelude::*;

#[derive(Debug, Default, Clone, Copy)]
struct Wave {
    ticks: usize,
    shots: usize,
    peak: usize,
    drops: usize,
}

impl Wave {
    fn shots_per_minute(self) -> f32 {
        self.shots as f32 * 3600. / self.ticks as f32
    }
}

/// Steers toward the lane of the enemy closest to the rim at the claw's top
/// speed, and fires only once it is in the front half of the web, so enemies
/// climb and shoot as they would against a human player.
fn steer(
    level: Res<Level>,
    mut glider: Single<&mut Glider>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    enemies: Query<&Transform, With<Enemy>>,
) {
    let web = &level.web;
    let nearest = enemies
        .iter()
        .max_by(|a, b| a.translation.z.total_cmp(&b.translation.z));
    let front = -web.world_depth() / 2.;
    if nearest.is_some_and(|pose| pose.translation.z > front) {
        keys.press(KeyCode::KeyZ);
    } else {
        keys.release(KeyCode::KeyZ);
    }
    let Some(target) = nearest.map(|pose| {
        (0..web.lanes())
            .min_by(|&a, &b| {
                let distance = |lane: usize| {
                    web.glider_transform(lane as i32 * LANE_UNIT)
                        .translation
                        .truncate()
                        .distance(pose.translation.truncate())
                };
                distance(a).total_cmp(&distance(b))
            })
            .unwrap()
    }) else {
        return;
    };
    let delta = web.lane_delta(glider.progress, target as i32 * LANE_UNIT);
    glider.progress = web.normalize(glider.progress + delta.clamp(-0x6000, 0x6000));
}

/// The wave's measurements, or `None` when the scripted player cannot clear it.
fn play(number: usize, difficulty: Difficulty) -> Option<Wave> {
    let mut app = app();
    let mut level = crate::test_support::levels().0.remove(number - 1);
    level.apply_difficulty(difficulty);
    let mut game = Game::default();
    reset_wave(&mut game, &level);
    game.status = Status::Playing;
    game.entry = None;
    app.insert_resource(level)
        .insert_resource(game)
        .insert_resource(difficulty)
        .init_resource::<ShotAssets>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(
            Startup,
            |mut commands: Commands,
             mut meshes: ResMut<Assets<Mesh>>,
             mut materials: ResMut<Assets<StandardMaterial>>,
             level: Res<Level>| {
                crate::enemies::setup(&mut commands, &mut meshes, &mut materials, &level);
            },
        )
        .add_systems(
            FixedUpdate,
            (
                (
                    advance_enemy_clock,
                    steer,
                    move_bullets,
                    fire_bullets.run_if(playing),
                    pulsars::pulse,
                    flippers::spawn_enemies,
                    tankers::spawn,
                    spikers::spawn,
                    beasts::spawn,
                    fuseballs::spawn,
                    pulsars::spawn,
                    mirrors::spawn,
                    aliens::spawn,
                )
                    .chain(),
                (
                    flippers::move_enemies,
                    mirrors::advance,
                    aliens::advance,
                    fuseballs::advance,
                    shots::move_enemy_shots,
                    pulsars::check_sparks,
                    pulsars::advance,
                    resolve_hits,
                    tankers::move_tankers,
                    spikers::advance,
                    shots::fire_enemy_shots,
                    powerups::advance_powerups,
                    check_game,
                )
                    .chain(),
            )
                .chain(),
        );
    app.world_mut().run_schedule(Startup);
    let mut wave = Wave::default();
    let mut new_shots = app
        .world_mut()
        .query_filtered::<(), Added<shots::EnemyShot>>();
    let mut new_drops = app.world_mut().query_filtered::<(), Added<PowerUp>>();
    let mut living = app.world_mut().query_filtered::<(), With<Enemy>>();
    for tick in 1..60 * 60 * 10 {
        // Nothing may end the run early: only the wave's own pace is measured.
        app.world_mut().resource_mut::<Game>().invulnerable = 2;
        ticks(&mut app, 1);
        let world = app.world_mut();
        wave.shots += new_shots.iter(world).count();
        wave.drops += new_drops.iter(world).count();
        wave.peak = wave.peak.max(living.iter(world).count());
        // Running the schedule directly never resets change ticks, so do it here.
        world.clear_trackers();
        if world.resource::<Game>().status == Status::Exiting {
            wave.ticks = tick;
            return Some(wave);
        }
    }
    None
}

fn compare(number: usize) -> Option<[Wave; 3]> {
    let waves = Difficulty::ALL.map(|difficulty| play(number, difficulty));
    let Some([easy, normal, hard]) = waves
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .map(|w| [w[0], w[1], w[2]])
    else {
        println!("level {number:3}: not cleared by the scripted player");
        return None;
    };
    println!(
        "level {number:3}: clear {:5}/{:5}/{:5} ticks, shots/min {:5.1}/{:5.1}/{:5.1}, peak {:2}/{:2}/{:2}, drops {}/{}/{}",
        easy.ticks,
        normal.ticks,
        hard.ticks,
        easy.shots_per_minute(),
        normal.shots_per_minute(),
        hard.shots_per_minute(),
        easy.peak,
        normal.peak,
        hard.peak,
        easy.drops,
        normal.drops,
        hard.drops,
    );
    Some([easy, normal, hard])
}

#[test]
fn harder_settings_raise_enemy_pressure_on_representative_waves() {
    for number in [1, 9, 33] {
        let [easy, normal, hard] = compare(number).expect("the scripted player clears these waves");
        assert!(
            easy.ticks > normal.ticks && normal.ticks > hard.ticks,
            "level {number} pace"
        );
        assert!(
            easy.shots_per_minute() < normal.shots_per_minute()
                && normal.shots_per_minute() < hard.shots_per_minute(),
            "level {number} enemy fire"
        );
        assert!(
            easy.peak <= normal.peak && normal.peak <= hard.peak,
            "level {number} crowding"
        );
    }
}

#[test]
#[ignore = "manual balance report across the catalog"]
fn balance_report() {
    let mut totals = [Wave::default(); 3];
    for waves in (1..=100).filter_map(compare) {
        for (total, wave) in totals.iter_mut().zip(waves) {
            total.ticks += wave.ticks;
            total.shots += wave.shots;
            total.drops += wave.drops;
            total.peak += wave.peak;
        }
    }
    for (difficulty, total) in Difficulty::ALL.iter().zip(totals) {
        println!(
            "{difficulty:?}: {} ticks, {:.1} shots/min, {} drops, peak sum {}",
            total.ticks,
            total.shots_per_minute(),
            total.drops,
            total.peak
        );
    }
}
