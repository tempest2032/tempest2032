//! Tankers climb lanes and release two Flippers, Fuseballs, or Pulsars when opened.
//! Handles shell graphics, spawning, releases, and fast-child movement.

use crate::cry;
use crate::enemies::Enemy;
use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::flipper_render::draw_approach_dot;
use crate::enemies::flippers::Flipper;
use crate::enemies::flippers::Motion;
use crate::enemies::flippers::SpiralFlipper;
use crate::enemies::flippers::flipper_transform;
use crate::enemies::fuseballs;
use crate::enemies::pulsar_render::{CYCLE, PulsarAssets};
use crate::enemies::pulsars::Pulsar;
use crate::enemies::pulsars::Pulse;
use crate::explosion;
use crate::game::Game;
use crate::game::Status;
use crate::level::Level;
use crate::level::LevelActor;
use crate::mesh::colored_mesh;
use crate::web::FRONT_RADIUS;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use crate::web::Web;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Contents {
    #[default]
    Flippers,
    Fuseballs,
    Pulsars,
}

#[derive(Component)]
#[require(LevelActor, Enemy, Contents)]
pub(crate) struct Tanker {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) approach: bool,
}

#[derive(Component)]
pub(crate) struct Released {
    pub(crate) burst: bool,
    pub(crate) fast: bool,
    pub(crate) remaining: u8,
    pub(crate) direction: i32,
}

#[derive(Resource, Default)]
pub(crate) struct TankerAssets {
    pub(crate) frames: Vec<Handle<Mesh>>,
    pub(crate) fast: Handle<Mesh>,
    pub(crate) spiral: Handle<Mesh>,
    pub(crate) point: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) shades: Vec<Handle<StandardMaterial>>,
}

fn tanker_mesh(phase: u8) -> Mesh {
    let hue = ((crate::sines::sine(i32::from(phase)) + 128) & 0xf0
        | ((crate::sines::sine(i32::from(phase) + 64) + 128) >> 4) & 0x0f) as u8;
    // draw_sfliptank selects s_fliptank2/ft2verts, not the old pink diamond.
    colored_mesh(super::beasts::part(
        &[
            (9., 9.),
            (5., 9.),
            (13., 9.),
            (-7., 5.),
            (25., 5.),
            (-7., 13.),
            (25., 13.),
            (9., 5.),
            (9., 13.),
            (13., 1.),
            (13., 17.),
            (5., 1.),
            (5., 17.),
        ],
        &[
            (hue, [9, 4, 0], [128, 64, 255]),
            (hue, [6, 10, 0], [64, 96, 255]),
            (hue, [12, 5, 0], [64, 128, 255]),
            (hue, [3, 11, 0], [128, 128, 255]),
            (0xf0, [3, 1, 0], [128, 192, 255]),
            (0x30, [4, 2, 0], [64, 96, 143]),
            (0x30, [5, 1, 0], [128, 192, 255]),
            (0xf0, [6, 2, 0], [64, 96, 143]),
            (0xf4, [9, 7, 0], [128, 192, 255]),
            (0x34, [8, 10, 0], [64, 96, 143]),
            (0x34, [11, 7, 0], [128, 192, 255]),
            (0xf4, [12, 8, 0], [64, 96, 143]),
        ],
        1.,
        0.,
    ))
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let fast_vertices = [
        (1., 9.),
        (5., 9.),
        (9., 5.),
        (9., 13.),
        (13., 9.),
        (17., 9.),
    ]
    .map(|(x, y)| Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.0));
    let fast = colored_mesh(
        [
            (0x44, [0, 1, 2]),
            (0x66, [5, 4, 2]),
            (0x44, [0, 1, 3]),
            (0x66, [5, 4, 3]),
        ]
        .map(|(hue, face)| {
            std::array::from_fn(|i| (fast_vertices[face[i]], cry::color(hue, [255, 128, 192][i])))
        }),
    );
    commands.insert_resource(TankerAssets {
        spiral: meshes.add(colored_mesh(
            [
                (0xcc, [6, 1, 4], [255, 64, 64]),
                (0xcc, [7, 1, 4], [255, 64, 64]),
                (0x44, [0, 1, 2], [255, 128, 192]),
                (0x66, [5, 4, 2], [255, 128, 192]),
                (0x44, [0, 1, 3], [255, 128, 192]),
                (0x66, [5, 4, 3], [255, 128, 192]),
            ]
            .map(|(hue, face, intensity)| {
                let points = [
                    (1., 9.),
                    (5., 9.),
                    (9., 5.),
                    (9., 13.),
                    (13., 9.),
                    (17., 9.),
                    (9., 3.),
                    (9., 15.),
                ]
                .map(|(x, y)| Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.));
                std::array::from_fn(|i| (points[face[i]], cry::color(hue, intensity[i])))
            }),
        )),
        frames: (0..64)
            .map(|frame| meshes.add(tanker_mesh(frame * 4)))
            .collect(),
        fast: meshes.add(fast),
        point: meshes.add(Rectangle::new(1., 1.)),
        material: materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        shades: (0..=255)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: cry::color(0x0f, i),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });
}

pub(crate) fn transform(tanker: &Tanker, web: &Web) -> Transform {
    let mut transform = web.glider_transform(tanker.lane as i32 * LANE_UNIT);
    transform.translation.z = -(tanker.depth as f32 / LANE_UNIT as f32) * WORLD_UNIT + 0.06;
    transform.rotation = web.orientation(tanker.lane);
    transform
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<TankerAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    let mut occupied = crate::enemies::occupancy(&living, &tankers);
    for (contents, generator) in [
        (Contents::Flippers, level.tankers.as_ref()),
        (Contents::Fuseballs, level.fuse_tankers.as_ref()),
        (
            Contents::Pulsars,
            level.pulsar_tankers.as_ref().map(|p| &p.generator),
        ),
    ] {
        let Some(generator) = generator else { continue };
        if game.remaining == 0 {
            break;
        }
        let timer = match contents {
            Contents::Flippers => &mut game.tanker_timer,
            Contents::Fuseballs => &mut game.fuse_tanker_timer,
            Contents::Pulsars => &mut game.pulsar_tanker_timer,
        };
        if !crate::difficulty::due(timer, clock.steps()) {
            continue;
        }
        if occupied >= level.flippers.max_active {
            continue;
        }
        *timer = generator.spawn_interval_ticks - 1;
        game.advance_random();
        let tanker = Tanker {
            lane: 1 + (game.random >> 16) as usize % (level.web.lanes() - 2),
            depth: level.web.rear() + 300 * LANE_UNIT,
            approach: true,
        };
        commands.spawn((
            transform(&tanker, &level.web),
            tanker,
            contents,
            RenderLayers::layer(1),
            Mesh3d(assets.point.clone()),
            MeshMaterial3d(assets.shades.first().cloned().unwrap_or_default()),
        ));
        game.remaining -= 1;
        occupied += 3;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open(
    commands: &mut Commands,
    game: &mut Game,
    level: &Level,
    assets: &FlipperAssets,
    shells: &TankerAssets,
    fuses: &fuseballs::FuseAssets,
    pulsars: &PulsarAssets,
    tanker: &Tanker,
    contents: Contents,
) {
    game.add_score(100);
    if contents == Contents::Pulsars {
        for direction in [-1, 1] {
            let pulsar = Pulsar::released(
                level.web.neighbor(tanker.lane, direction).unwrap(),
                tanker.depth,
                tanker.lane,
                direction,
            );
            commands.spawn((
                pulsar.transform(&level.web),
                pulsar,
                RenderLayers::layer(1),
                Mesh3d(pulsars.frames[3].clone()),
                MeshMaterial3d(pulsars.material.clone()),
            ));
        }
        return;
    }
    if contents == Contents::Fuseballs {
        for direction in [-1, 1] {
            let from = tanker.lane + usize::from(direction > 0);
            let to = if level.web.closed {
                (from as i32 + direction).rem_euclid(level.web.lanes() as i32) as usize
            } else {
                (from as i32 + direction) as usize
            };
            let lane = level.web.neighbor(tanker.lane, direction).unwrap();
            commands.spawn((
                fuseballs::Fuseball {
                    edge: from,
                    depth: tanker.depth,
                    motion: fuseballs::Motion::Cross {
                        from,
                        to,
                        lane,
                        steps: 0,
                        delay: 4,
                    },
                    rise: 48,
                    tick: 0,
                },
                Transform::from_translation(level.web.vertex(
                    from as f32,
                    FRONT_RADIUS,
                    -tanker.depth as f32 / LANE_UNIT as f32 * WORLD_UNIT + 0.06,
                )),
                RenderLayers::layer(1),
                Mesh3d(fuses.frames.first().cloned().unwrap_or_default()),
                MeshMaterial3d(fuses.material.clone()),
            ));
        }
        return;
    }
    for direction in [-1, 1] {
        game.advance_random();
        let fast =
            (game.random >> 24) < u32::from(level.tankers.as_ref().map_or(0, |g| g.super_chance));
        let scatter = ((game.random >> 16) & 3) as u8 + 1;
        let chance = level.tankers.as_ref().map_or(0, |g| g.spiral_chance);
        let spiral = if fast && chance > 0 {
            game.advance_random();
            (game.random >> 24) < u32::from(chance)
        } else {
            false
        };
        let lane = level.web.neighbor(tanker.lane, direction).unwrap();
        let flipper = Flipper {
            lane,
            depth: tanker.depth,
            motion: Motion::Flip {
                from: tanker.lane,
                direction,
                angle: 0,
            },
            pause: if fast { 0 } else { level.flippers.pause_ticks },
        };
        let remaining = if fast && !spiral { scatter } else { 0 };
        let mut child = commands.spawn((
            flipper_transform(&flipper, &level.web),
            flipper,
            Released {
                burst: true,
                fast: fast && !spiral,
                remaining,
                direction,
            },
            RenderLayers::layer(1),
            Mesh3d(if spiral {
                shells.spiral.clone()
            } else if fast {
                shells.fast.clone()
            } else {
                assets.body.clone()
            }),
            MeshMaterial3d(assets.shaded.clone()),
        ));
        if spiral {
            child.insert(SpiralFlipper);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn move_tankers(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<FlipperAssets>,
    shells: Res<TankerAssets>,
    bursts: Res<explosion::BurstAssets>,
    (fuses, pulsars, pulse): (Res<fuseballs::FuseAssets>, Res<PulsarAssets>, Res<Pulse>),
    mut tankers: Query<(
        Entity,
        &mut Tanker,
        &Contents,
        &mut Transform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut frame: Local<u8>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing {
        return;
    }
    *frame = frame.wrapping_add(1);
    for (entity, mut tanker, contents, mut pose, mut mesh, mut material) in &mut tankers {
        let Some(generator) = (match contents {
            Contents::Flippers => level.tankers.as_ref(),
            Contents::Fuseballs => level.fuse_tankers.as_ref(),
            Contents::Pulsars => level.pulsar_tankers.as_ref().map(|p| &p.generator),
        }) else {
            continue;
        };
        if tanker.approach {
            tanker.depth -= clock.scaled(2 * LANE_UNIT);
            if tanker.depth <= level.web.rear() {
                tanker.depth = level.web.rear();
                tanker.approach = false;
                mesh.0 = match contents {
                    Contents::Flippers => shells.frames.first().cloned().unwrap_or_default(),
                    Contents::Fuseballs => fuses.tanker.clone(),
                    Contents::Pulsars => pulsars.tankers[CYCLE[pulse.phase as usize]].clone(),
                };
                material.0 = shells.material.clone();
            }
        } else {
            tanker.depth = (tanker.depth - clock.scaled(generator.climb_speed)).max(0);
        }
        *pose = transform(&tanker, &level.web);
        if !tanker.approach && *contents == Contents::Flippers {
            let phase = frame.wrapping_mul(4);
            pose.rotate_local_z(-f32::from(phase) * TAU / 256.);
            mesh.set_if_neq(Mesh3d(
                shells
                    .frames
                    .get(usize::from(phase / 4))
                    .cloned()
                    .unwrap_or_default(),
            ));
        }
        if !tanker.approach && *contents != Contents::Flippers {
            pose.rotate_local_z(-f32::from(*frame) * TAU / 256.);
            if *contents == Contents::Pulsars {
                mesh.set_if_neq(Mesh3d(pulsars.tankers[CYCLE[pulse.phase as usize]].clone()));
            }
        }
        if !tanker.approach && tanker.depth == 0 {
            open(
                &mut commands,
                &mut game,
                &level,
                &assets,
                &shells,
                &fuses,
                &pulsars,
                &tanker,
                *contents,
            );
            explosion::spawn_sphere(&mut commands, &bursts, pose.translation);
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_points(
    assets: Res<TankerAssets>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<Tanker>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut tankers: Query<(
        &Tanker,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    for (tanker, mut transform, mut material) in &mut tankers {
        if tanker.approach {
            draw_approach_dot(
                &mut transform,
                &mut material,
                &assets.shades,
                camera,
                projection,
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/tankers.rs"]
mod tests;
