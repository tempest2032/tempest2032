//! Pulsars climb lanes and electrify them during phase seven of a shared pulse.
//! At the rim they split into two sparks that propagate in opposite directions.
//! Implements spawning, approach gating, movement, and grounded-player contact.

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::{
    animations::glider_death::Death,
    enemies::{
        Enemy,
        flippers::{Flipper, Motion, flipper_transform},
        pulsar_render::PulsarAssets,
        tankers::Tanker,
    },
    game::{Game, Status},
    glider::Glider,
    level::{Level, LevelActor},
    powerups::Upgrades,
    weapons::Weapon,
    web::{LANE_UNIT, Web},
};

#[derive(Resource, Default)]
pub(crate) struct Pulse {
    pub(crate) phase: u8,
    pub(crate) frame: u32,
    ticks: u8,
}

#[derive(Clone, Copy)]
struct Spark {
    direction: i32,
    ticks: u8,
}

#[derive(Component, Clone, Copy)]
#[require(LevelActor, Enemy)]
pub(crate) struct Pulsar {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) approach: bool,
    spark: Option<Spark>,
    flip: Option<Motion>,
}

impl Pulsar {
    pub(crate) fn target(&self, web: &Web) -> Option<(usize, i32)> {
        if self.flipping() {
            self.flipper().hit_lane(web).map(|lane| (lane, self.depth))
        } else {
            (!self.approach).then_some((self.lane, self.depth))
        }
    }

    pub(crate) fn points(&self) -> u32 {
        if self.spark.is_some() || self.flipping() {
            150
        } else {
            200
        }
    }

    pub(crate) fn electrified(&self, phase: u8) -> bool {
        self.climbing() && phase == 7
    }

    pub(crate) fn climbing(&self) -> bool {
        !self.approach && self.spark.is_none() && !self.flipping()
    }

    pub(crate) fn flipping(&self) -> bool {
        self.flip.is_some()
    }

    pub(crate) fn released(lane: usize, depth: i32, from: usize, direction: i32) -> Self {
        Self {
            lane,
            depth,
            approach: false,
            spark: None,
            flip: Some(Motion::Flip {
                from,
                direction,
                angle: 0,
            }),
        }
    }

    fn flipper(&self) -> Flipper {
        Flipper {
            lane: self.lane,
            depth: self.depth,
            motion: self.flip.unwrap_or(Motion::Climb),
            pause: 0,
        }
    }

    pub(crate) fn transform(&self, web: &Web) -> Transform {
        flipper_transform(&self.flipper(), web)
    }
}

pub(crate) fn pulse(
    mut commands: Commands,
    level: Res<Level>,
    game: Res<Game>,
    mut pulse: ResMut<Pulse>,
    pulsars: Query<&Pulsar>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(settings) = level.pulse_settings() else {
        return;
    };
    if !matches!(game.status, Status::Entering | Status::Playing) {
        return;
    }
    for _ in 0..clock.steps() {
        pulse.frame = pulse.frame.wrapping_add(1);
        pulse.ticks += 1;
        if pulse.ticks >= settings.phase_ticks {
            pulse.ticks = 0;
            pulse.phase = (pulse.phase + 1) & 15;
            if pulse.phase == 7 && pulsars.iter().any(Pulsar::climbing) {
                crate::sound::play(&mut commands, crate::sound::Effect::Pulse);
            }
        }
    }
}

pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<PulsarAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(settings) = &level.pulsars else {
        return;
    };
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    if !crate::difficulty::due(&mut game.pulsar_timer, clock.steps()) {
        return;
    }
    if crate::enemies::occupancy(&living, &tankers) >= level.flippers.max_active {
        return;
    }
    game.advance_random();
    let pulsar = Pulsar {
        lane: (game.random >> 16) as usize % level.web.lanes(),
        depth: level.web.rear() + 300 * LANE_UNIT,
        approach: true,
        spark: None,
        flip: None,
    };
    let pose = pulsar.transform(&level.web);
    commands.spawn((
        pulsar,
        pose,
        RenderLayers::layer(1),
        Mesh3d(assets.point.clone()),
        MeshMaterial3d(assets.shades[0].clone()),
    ));
    game.remaining -= 1;
    game.pulsar_timer = settings.generator.spawn_interval_ticks - 1;
}

pub(crate) fn advance(
    level: Res<Level>,
    game: Res<Game>,
    pulse: Res<Pulse>,
    assets: Res<PulsarAssets>,
    mut commands: Commands,
    mut pulsars: Query<(&mut Pulsar, &mut Transform)>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(settings) = level.pulse_settings() else {
        return;
    };
    if game.status != Status::Playing {
        return;
    }
    for (mut pulsar, mut pose) in &mut pulsars {
        if let Some(Motion::Flip {
            from,
            direction,
            angle,
        }) = pulsar.flip
        {
            let angle =
                angle.saturating_add(clock.scaled(i32::from(level.flippers.flip_speed)) as u16);
            pulsar.flip = (angle < level.web.flip_angle(from, pulsar.lane, direction)).then_some(
                Motion::Flip {
                    from,
                    direction,
                    angle,
                },
            );
            *pose = pulsar.transform(&level.web);
            continue;
        }
        let climb_speed = level
            .pulsars
            .as_ref()
            .map_or(0x8000, |p| p.generator.climb_speed);
        if pulsar.approach {
            pulsar.depth -= clock.scaled(2 * LANE_UNIT);
            if pulsar.depth <= level.web.rear() {
                pulsar.depth = level.web.rear();
                // run_objects waits at the rear until the shared pulse is harmless.
                if pulse.phase < 3 {
                    pulsar.approach = false;
                    pulsar.depth -= clock.scaled(climb_speed);
                }
            }
        } else if let Some(mut spark) = pulsar.spark {
            for _ in 0..clock.steps() {
                spark.ticks -= 1;
                if spark.ticks == 0 {
                    spark.ticks = settings.propagation_ticks;
                    if level.web.neighbor(pulsar.lane, spark.direction).is_none() {
                        spark.direction = -spark.direction;
                    }
                    pulsar.lane = level
                        .web
                        .neighbor(pulsar.lane, spark.direction)
                        .unwrap_or(pulsar.lane);
                }
            }
            pulsar.spark = Some(spark);
        } else {
            pulsar.depth -= clock.scaled(climb_speed);
            if pulsar.depth < 0 {
                pulsar.depth = 0;
                pulsar.spark = Some(Spark {
                    direction: -1,
                    ticks: settings.propagation_ticks,
                });
                let other = Pulsar {
                    spark: Some(Spark {
                        direction: 1,
                        ticks: settings.propagation_ticks,
                    }),
                    ..*pulsar
                };
                commands.spawn((
                    other,
                    level.web.glider_transform(other.lane as i32 * LANE_UNIT),
                    RenderLayers::layer(1),
                    Mesh3d(assets.frames[5].clone()),
                    MeshMaterial3d(assets.material.clone()),
                ));
            }
        }
        *pose = pulsar.transform(&level.web);
    }
}

pub(crate) fn check_player(
    level: Res<Level>,
    mut game: ResMut<Game>,
    pulse: Res<Pulse>,
    mut upgrades: ResMut<Upgrades>,
    mut player: Single<(&mut Glider, &mut Weapon, &Transform, &mut Visibility)>,
    pulsars: Query<&Pulsar>,
) {
    if game.status != Status::Playing || game.invulnerable > 0 || upgrades.jump_height > 0 {
        return;
    }
    let lane = level.web.selected_lane(player.0.progress) as usize;
    if pulsars
        .iter()
        .any(|p| p.lane == lane && p.electrified(pulse.phase))
    {
        fry(&level, &mut game, &mut upgrades, &mut player);
    }
}

pub(crate) fn check_sparks(
    level: Res<Level>,
    mut game: ResMut<Game>,
    mut upgrades: ResMut<Upgrades>,
    mut player: Single<(&mut Glider, &mut Weapon, &Transform, &mut Visibility)>,
    pulsars: Query<&Pulsar>,
) {
    if game.status != Status::Playing || game.invulnerable > 0 || upgrades.jump_height > 0 {
        return;
    }
    let lane = level.web.selected_lane(player.0.progress) as usize;
    if pulsars.iter().any(|p| p.lane == lane && p.spark.is_some()) {
        fry(&level, &mut game, &mut upgrades, &mut player);
    }
}

fn fry(
    level: &Level,
    game: &mut Game,
    upgrades: &mut Upgrades,
    player: &mut Single<(&mut Glider, &mut Weapon, &Transform, &mut Visibility)>,
) {
    let (glider, weapon, pose, visibility) = &mut **player;
    game.status = Status::Fried;
    game.announce(crate::messages::Id::FriedYou);
    game.death = Some(Death::new(pose.translation, &level.web));
    glider.velocity = 0;
    **weapon = Weapon::default();
    **visibility = Visibility::Hidden;
    upgrades.lose_life();
}

#[cfg(test)]
#[path = "tests/pulsars.rs"]
mod tests;
