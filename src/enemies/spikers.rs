//! Spikers are spiral enemies that build persistent spikes along web lanes.
//! Handles spawning, movement, spike growth, and persistent spike graphics.

use crate::cry;
use crate::enemies::Enemy;
use crate::enemies::tankers;
use crate::game::Game;
use crate::game::Status;
use crate::level::Level;
use crate::level::LevelActor;
use crate::mesh::colored_mesh;
use crate::mesh::segment_transform;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Motion {
    Seek,
    Climb,
    Descend,
}

#[derive(Component)]
#[require(LevelActor, Enemy)]
pub(crate) struct Spiker {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) motion: Motion,
    pub(crate) spike: Option<Entity>,
    pub(crate) build: u16,
    pub(crate) angle: u8,
    /// Spike extension per enemy tick: the Spiker's climb speed, as in `climbspike`.
    pub(crate) spike_speed: i32,
}

impl Spiker {
    pub(crate) fn firing(&self) -> bool {
        !matches!(self.motion, Motion::Seek)
    }
}

#[derive(Component)]
#[require(LevelActor)]
pub(crate) struct Spike {
    pub(crate) lane: usize,
    pub(crate) length: i32,
}

#[derive(Resource, Default)]
pub(crate) struct SpikerAssets {
    pub(crate) body: Handle<Mesh>,
    pub(crate) line: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let points = [
        (10., 9.),
        (9., 10.),
        (8., 9.),
        (9., 7.),
        (12., 8.),
        (10., 11.),
        (8., 11.),
        (6., 8.),
        (9., 5.),
        (14., 7.),
        (13., 14.),
        (6., 15.),
    ]
    .map(|(x, y)| Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.));
    let triangles = points.windows(2).flat_map(|edge| {
        let (a, b) = (edge[0], edge[1]);
        let side = (b - a).normalize().cross(Vec3::Z) * 0.012;
        let q = [a - side, a + side, b + side, b - side].map(|p| (p, Color::WHITE));
        [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
    });
    commands.insert_resource(SpikerAssets {
        body: meshes.add(colored_mesh(triangles)),
        line: meshes.add(Cuboid::new(1., 1., 1.)),
        material: materials.add(StandardMaterial {
            base_color: cry::color(0x8f, 255),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<SpikerAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<tankers::Tanker>>,
    spikers: Query<&Spiker>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    let mut occupied = crate::enemies::occupancy(&living, &tankers);
    let mut has_spiker = !spikers.is_empty();
    for (extra, generator) in [(false, &level.spikers), (true, &level.extra_spikers)] {
        let Some(generator) = generator else { continue };
        if game.remaining == 0 {
            break;
        }
        let timer = if extra {
            &mut game.extra_spiker_timer
        } else {
            &mut game.spiker_timer
        };
        if !crate::difficulty::due(timer, clock.steps()) {
            continue;
        }
        if occupied >= level.flippers.max_active {
            continue;
        }
        *timer = generator.spawn_interval_ticks - 1;
        game.remaining -= 1;
        // launch1 spends the budget even when max_spikers refuses the attempt.
        if has_spiker {
            continue;
        }
        game.advance_random();
        let lane = (game.random >> 16) as usize % level.web.lanes();
        let mut pose = level.web.glider_transform(lane as i32 * LANE_UNIT);
        pose.translation.z = -level.web.world_depth() + 0.06;
        commands.spawn((
            Spiker {
                lane,
                depth: level.web.rear(),
                motion: Motion::Seek,
                spike: None,
                build: 50,
                angle: 0,
                spike_speed: generator.climb_speed,
            },
            pose,
            RenderLayers::layer(1),
            Mesh3d(assets.body.clone()),
            MeshMaterial3d(assets.material.clone()),
        ));
        has_spiker = true;
        occupied += 1;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn advance(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<SpikerAssets>,
    mut spikers: Query<(&mut Spiker, &mut Transform), Without<Spike>>,
    mut spikes: Query<(Entity, &mut Spike)>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing {
        return;
    }
    let Some(generator) = &level.spikers else {
        return;
    };
    for (mut spiker, mut pose) in &mut spikers {
        spiker.angle = spiker.angle.wrapping_add(4);
        match spiker.motion {
            Motion::Seek => {
                let free = (0..level.web.lanes())
                    .filter(|lane| !spikes.iter().any(|(_, s)| s.lane == *lane))
                    .collect::<Vec<_>>();
                let (entity, lane) = if free.is_empty() {
                    let (entity, spike) = spikes.iter().min_by_key(|(_, s)| s.length).unwrap();
                    (entity, spike.lane)
                } else {
                    game.advance_random();
                    let lane = free[(game.random >> 16) as usize % free.len()];
                    let mut pose = level.web.glider_transform(lane as i32 * LANE_UNIT);
                    pose.translation.z = -level.web.world_depth();
                    let entity = commands
                        .spawn((
                            Spike {
                                lane,
                                length: 30 * LANE_UNIT,
                            },
                            pose,
                            Visibility::Visible,
                            RenderLayers::layer(1),
                            Mesh3d(assets.line.clone()),
                            MeshMaterial3d(assets.material.clone()),
                        ))
                        .id();
                    (entity, lane)
                };
                spiker.lane = lane;
                spiker.spike = Some(entity);
                spiker.build = 50;
                spiker.motion = Motion::Climb;
            }
            Motion::Climb => {
                spiker.depth -= clock.scaled(generator.climb_speed);
                if spiker.depth < 10 * LANE_UNIT {
                    spiker.motion = Motion::Descend;
                } else if let Some(entity) = spiker.spike
                    && let Ok((_, mut spike)) = spikes.get_mut(entity)
                {
                    // climbspike compares integer Z words, not full fixed-point distances.
                    let height = (level.web.rear() - spiker.depth + LANE_UNIT - 1) / LANE_UNIT;
                    // Builds count enemy ticks, so spikes reach the same length on every setting.
                    if height > spike.length / LANE_UNIT && clock.steps() > 0 {
                        let length = spike.length + clock.scaled(spiker.spike_speed);
                        if length < 150 * LANE_UNIT {
                            spike.length = length;
                        }
                        if spiker.build == 0 {
                            spiker.motion = Motion::Descend;
                        } else {
                            spiker.build = spiker.build.saturating_sub(clock.steps() as u16);
                        }
                    }
                } else {
                    spiker.motion = Motion::Descend;
                }
            }
            Motion::Descend => {
                spiker.depth += clock.scaled(generator.climb_speed);
                if spiker.depth >= level.web.rear() {
                    spiker.depth = level.web.rear();
                    spiker.motion = Motion::Seek;
                }
            }
        }
        *pose = level.web.glider_transform(spiker.lane as i32 * LANE_UNIT);
        pose.translation.z = -(spiker.depth as f32 / LANE_UNIT as f32) * WORLD_UNIT + 0.06;
        pose.rotation = Quat::from_rotation_z(f32::from(spiker.angle) * TAU / 256.);
    }
}

pub(crate) fn draw(
    level: Res<Level>,
    mut spikes: Query<(&Spike, &mut Transform, &mut Visibility)>,
) {
    for (spike, mut pose, mut visible) in &mut spikes {
        visible.set_if_neq(if spike.length > 0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        if spike.length <= 0 {
            continue;
        }
        let mut start = level
            .web
            .glider_transform(spike.lane as i32 * LANE_UNIT)
            .translation;
        start.z = -level.web.world_depth();
        *pose = segment_transform(
            start,
            // draw_spike negates and halves the integer length, then draws at 2× scale.
            start + Vec3::Z * (((spike.length / LANE_UNIT + 1) & !1) as f32) * WORLD_UNIT,
            0.018,
        );
    }
}

#[cfg(test)]
#[path = "tests/spikers.rs"]
mod tests;
