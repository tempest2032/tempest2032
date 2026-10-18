//! Fuseballs are multicolored, five-armed enemies that climb lane boundaries
//! and cross toward the glider. They are shootable near the middle of a crossing;
//! contact at the rim fries the glider. Implements movement and point graphics.

use crate::animations::glider_death as shot_death;
use crate::cry;
use crate::enemies::Enemy;
use crate::enemies::flipper_render::draw_approach_dot;
use crate::enemies::tankers;
use crate::explosion;
use crate::game::Game;
use crate::game::Status;
use crate::glider::Glider;
use crate::level::Level;
use crate::level::LevelActor;
use crate::mesh::colored_mesh;
use crate::powerups;
use crate::weapons::Bullet;
use crate::weapons::ParticleShot;
use crate::weapons::Weapon;
use crate::web::FRONT_RADIUS;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use crate::web::Web;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use std::collections::HashSet;
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub(crate) enum Motion {
    Approach,
    Rail,
    Cross {
        from: usize,
        to: usize,
        lane: usize,
        steps: u8,
        delay: u8,
    },
}

#[derive(Component)]
#[require(LevelActor, Enemy)]
pub(crate) struct Fuseball {
    pub(crate) depth: i32,
    pub(crate) edge: usize,
    pub(crate) motion: Motion,
    pub(crate) rise: u8,
    pub(crate) tick: u16,
}

impl Fuseball {
    /// One enemy tick of the rail wait and the sixteen-step lane crossing.
    fn step(&mut self, web: &Web, player_lane: usize) {
        match self.motion {
            Motion::Approach => {}
            Motion::Rail => {
                if self.rise > 0 {
                    self.rise -= 1;
                    return;
                }
                self.rise = 48;
                let reference = if web.closed {
                    (self.edge + web.lanes() - 1) % web.lanes()
                } else {
                    self.edge.saturating_sub(1)
                };
                let delta =
                    web.lane_delta(reference as i32 * LANE_UNIT, player_lane as i32 * LANE_UNIT);
                let mut direction = if delta > 0 { 1 } else { -1 };
                let edge_count = if web.closed {
                    web.lanes()
                } else {
                    web.lanes() + 1
                };
                if !web.closed
                    && ((direction < 0 && self.edge == 0)
                        || (direction > 0 && self.edge == web.lanes()))
                {
                    direction = -direction;
                }
                let to = (self.edge as i32 + direction).rem_euclid(edge_count as i32) as usize;
                let lane = if direction > 0 { self.edge } else { to };
                self.motion = Motion::Cross {
                    from: self.edge,
                    to,
                    lane,
                    steps: 0,
                    delay: 4,
                };
            }
            Motion::Cross {
                from,
                to,
                lane,
                steps,
                delay,
            } => {
                self.motion = if delay > 0 {
                    Motion::Cross {
                        from,
                        to,
                        lane,
                        steps,
                        delay: delay - 1,
                    }
                } else if steps == 15 {
                    self.edge = to;
                    Motion::Rail
                } else {
                    Motion::Cross {
                        from,
                        to,
                        lane,
                        steps: steps + 1,
                        delay: 4,
                    }
                };
            }
        }
    }

    pub(crate) fn target(&self) -> Option<(usize, i32)> {
        match self.motion {
            Motion::Cross {
                lane,
                steps: 3..=11,
                ..
            } => Some((lane, self.depth)),
            _ => None,
        }
    }
    pub(crate) fn danger(&self) -> Option<usize> {
        if self.depth <= 0 {
            match self.motion {
                Motion::Cross { lane, .. } => Some(lane),
                _ => None,
            }
        } else {
            None
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct FuseAssets {
    pub(crate) frames: Vec<Handle<Mesh>>,
    pub(crate) tanker: Handle<Mesh>,
    pub(crate) point: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) shades: Vec<Handle<StandardMaterial>>,
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mut frames = Vec::new();
    let mut tanker = Vec::new();
    for pattern in 0..32 {
        let mut triangles = Vec::new();
        for (arm, hue) in [240, 143, 128, 255, 15].into_iter().enumerate() {
            let points = if pattern & (1 << arm) == 0 {
                [(1., 9.), (5., 6.), (5., 8.), (9., 9.)]
            } else {
                [(1., 9.), (5., 12.), (5., 10.), (9., 9.)]
            };
            let rotation = Quat::from_rotation_z(-(arm as f32) * 51. * TAU / 256.);
            let points =
                points.map(|(x, y)| rotation * Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.));
            for (face, intensity) in [([0, 1, 2], [64, 128, 96]), ([3, 1, 2], [255, 128, 96])] {
                triangles.push(std::array::from_fn(|i| {
                    (points[face[i]], cry::color(hue, intensity[i]))
                }));
            }
        }
        if pattern == 0 || pattern == 31 {
            tanker.extend(triangles.iter().copied());
        }
        frames.push(meshes.add(colored_mesh(triangles)));
    }
    commands.insert_resource(FuseAssets {
        frames,
        tanker: meshes.add(colored_mesh(tanker)),
        point: meshes.add(Rectangle::new(1., 1.)),
        material: materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        shades: (0..=255)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: cry::color(0x80, i),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<FuseAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<tankers::Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(g) = &level.fuseballs else {
        return;
    };
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    if !crate::difficulty::due(&mut game.fuse_timer, clock.steps()) {
        return;
    }
    if crate::enemies::occupancy(&living, &tankers) >= level.flippers.max_active {
        return;
    }
    game.advance_random();
    let lane = (game.random >> 16) as usize % level.web.lanes();
    let edge = if level.web.closed {
        (lane + 1) % level.web.lanes()
    } else {
        lane + 1
    };
    let fuse = Fuseball {
        edge,
        depth: level.web.rear() + 300 * LANE_UNIT,
        motion: Motion::Approach,
        rise: 48,
        tick: 0,
    };
    commands.spawn((
        fuse,
        Transform::from_translation(level.web.vertex(
            edge as f32,
            FRONT_RADIUS,
            -(level.web.depth as f32 + 300.) * WORLD_UNIT,
        )),
        RenderLayers::layer(1),
        Mesh3d(assets.point.clone()),
        MeshMaterial3d(assets.shades.first().cloned().unwrap_or_default()),
    ));
    game.remaining -= 1;
    game.fuse_timer = g.spawn_interval_ticks - 1;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn advance(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut upgrades: ResMut<powerups::Upgrades>,
    mut player: Single<(&mut Glider, &mut Weapon, &Transform, &mut Visibility)>,
    assets: Res<FuseAssets>,
    bursts: Res<explosion::BurstAssets>,
    bullets: Query<(Entity, &Bullet, &Transform, Option<&ParticleShot>)>,
    mut fuses: Query<
        (
            Entity,
            &mut Fuseball,
            &mut Transform,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (Without<Glider>, Without<Bullet>),
    >,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing {
        return;
    }
    let Some(g) = &level.fuseballs else {
        return;
    };
    let (glider, weapon, player_pose, visibility) = &mut *player;
    let player_lane = level.web.selected_lane(glider.progress) as usize;
    let mut consumed = HashSet::new();
    for (entity, mut fuse, mut pose, mut mesh, mut material) in &mut fuses {
        if let Some((lane, depth)) = fuse.target()
            && let Some((shot, _, _, particle)) = bullets.iter().find(|(shot, b, p, _)| {
                !consumed.contains(shot)
                    && b.lane == lane
                    && (p.translation.z - (-depth as f32 / LANE_UNIT as f32 * WORLD_UNIT + 0.06))
                        .abs()
                        < 6. * WORLD_UNIT
            })
        {
            if particle.is_none() {
                commands.entity(shot).despawn();
                consumed.insert(shot);
            }
            let points = game.bonus_points();
            game.add_score(points);
            game.score_popup(points, pose.translation);
            commands.entity(entity).despawn();
            explosion::spawn_explosion(&mut commands, &bursts, pose.translation);
            continue;
        }
        if game.invulnerable == 0 && upgrades.jump_height == 0 && fuse.danger() == Some(player_lane)
        {
            game.status = Status::Fried;
            game.announce(crate::messages::Id::FriedYou);
            game.death = Some(shot_death::Death::new(player_pose.translation, &level.web));
            glider.velocity = 0;
            **weapon = Weapon::default();
            **visibility = Visibility::Hidden;
            upgrades.lose_life();
            return;
        }
        fuse.tick = fuse.tick.wrapping_add(clock.steps() as u16);
        let approaching = matches!(fuse.motion, Motion::Approach);
        match fuse.motion {
            Motion::Approach => {
                fuse.depth -= clock.scaled(2 * LANE_UNIT);
                if fuse.depth <= level.web.rear() {
                    fuse.depth = level.web.rear();
                    fuse.motion = Motion::Rail;
                    material.0 = assets.material.clone();
                }
            }
            Motion::Rail if fuse.depth >= 0 => fuse.depth -= clock.scaled(g.climb_speed),
            _ => {}
        }
        // The rail wait and lane crossing advance in whole enemy ticks, from the next tick
        // after the approach ends.
        for _ in 0..if approaching { 0 } else { clock.steps() } {
            fuse.step(&level.web, player_lane);
        }
        let z = -fuse.depth as f32 / LANE_UNIT as f32 * WORLD_UNIT + 0.06;
        *pose = Transform::from_translation(match fuse.motion {
            Motion::Cross {
                from, to, steps, ..
            } => level.web.vertex(from as f32, FRONT_RADIUS, z).lerp(
                level.web.vertex(to as f32, FRONT_RADIUS, z),
                f32::from(steps) / 16.,
            ),
            _ => level.web.vertex(fuse.edge as f32, FRONT_RADIUS, z),
        });
        if !matches!(fuse.motion, Motion::Approach) {
            pose.rotation =
                Quat::from_rotation_z(-f32::from((fuse.tick as u8).wrapping_mul(2)) * TAU / 256.);
            let pattern = (u32::from(fuse.tick / 4)
                .wrapping_mul(1103515245)
                .wrapping_add(12345)
                >> 16)
                & 31;
            if let Some(frame) = assets.frames.get(pattern as usize) {
                mesh.set_if_neq(Mesh3d(frame.clone()));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_points(
    assets: Res<FuseAssets>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<Fuseball>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut fuses: Query<(
        &Fuseball,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    for (fuse, mut pose, mut material) in &mut fuses {
        if matches!(fuse.motion, Motion::Approach) {
            draw_approach_dot(&mut pose, &mut material, &assets.shades, camera, projection);
        }
    }
}

#[cfg(test)]
#[path = "tests/fuseballs.rs"]
mod tests;
