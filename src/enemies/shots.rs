//! Rotating enemy shots fired by climbing Flippers and Spikers. Also advances
//! detached Beastie horns, which return toward the glider as reflected shots.

use crate::enemies::SpawnOrder;
use crate::enemies::beasts;
use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::flippers::Flipper;
use crate::enemies::flippers::Motion;
use crate::enemies::mirrors;
use crate::enemies::spikers;
use crate::game::Game;
use crate::game::Status;
use crate::level::Level;
use crate::level::LevelActor;
use crate::mesh::colored_mesh;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Component)]
#[require(LevelActor, crate::enemies::SpawnOrder)]
pub(crate) struct EnemyShot {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) angle: u8,
}

pub(crate) fn enemy_shot_mesh() -> Mesh {
    // Keep the larger darts, but move each one inward by 1.5 source pixels.
    let vertices = [(1., 9.), (4., 9.), (5., 8.), (5., 10.)]
        .map(|(x, y)| Vec3::new((x - 7.5) * 0.15, (9. - y) * 0.15, 0.0));
    let faces = [
        ([135, 104, 255], [(2, 192), (1, 255), (0, 64)]),
        ([224, 255, 71], [(3, 192), (1, 255), (0, 64)]),
    ];
    let mut triangles = Vec::new();
    for turn in 0..3 {
        let rotation = Quat::from_rotation_z(-(turn * 85) as f32 * TAU / 256.0);
        for (rgb, face) in faces {
            triangles.push(face.map(|(index, intensity)| {
                (
                    rotation * vertices[index],
                    Color::srgb_u8(
                        ((rgb[0] * intensity) >> 8) as u8,
                        ((rgb[1] * intensity) >> 8) as u8,
                        ((rgb[2] * intensity) >> 8) as u8,
                    ),
                )
            }));
        }
    }
    colored_mesh(triangles)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_enemy_shots(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<FlipperAssets>,
    enemies: Query<(&Flipper, &SpawnOrder)>,
    shots: Query<&EnemyShot, (Without<beasts::Reflected>, Without<mirrors::ReturnedShot>)>,
    spikers: Query<(&spikers::Spiker, &SpawnOrder)>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing {
        return;
    }
    // `alienfire` runs from each climbing object while `run_objects` walks the
    // newest-first active list, and every call decrements the shared timer.
    let mut sources: Vec<_> = enemies
        .iter()
        .filter(|(f, _)| matches!(f.motion, Motion::Climb))
        .map(|(f, order)| (order.0, f.lane, f.depth))
        .chain(
            spikers
                .iter()
                .filter(|(s, _)| s.firing())
                .map(|(s, order)| (order.0, s.lane, s.depth)),
        )
        .collect();
    sources.sort_unstable_by_key(|&(order, ..)| std::cmp::Reverse(order));
    for (_, lane, depth) in (0..clock.steps()).flat_map(|_| sources.iter().copied()) {
        if game.fire_timer > 0 {
            game.fire_timer -= 1;
            continue;
        }
        game.fire_timer = 112;
        if depth <= 60 * LANE_UNIT || shots.iter().count() >= level.enemy_shots {
            continue;
        }
        let shot = EnemyShot {
            lane,
            depth,
            angle: 0,
        };
        crate::sound::play(&mut commands, crate::sound::Effect::Shot);
        commands.spawn((
            Transform::from_translation({
                let mut p = level
                    .web
                    .glider_transform(lane as i32 * LANE_UNIT)
                    .translation;
                p.z = -(depth as f32 / LANE_UNIT as f32) * WORLD_UNIT + 0.06;
                p
            }),
            shot,
            RenderLayers::layer(1),
            Mesh3d(assets.shot.clone()),
            MeshMaterial3d(assets.shaded.clone()),
        ));
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn move_enemy_shots(
    mut commands: Commands,
    game: Res<Game>,
    mut shots: Query<(
        Entity,
        &mut EnemyShot,
        &mut Transform,
        Option<&beasts::Reflected>,
        Option<&mirrors::ReturnedShot>,
    )>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Playing {
        return;
    }
    for (entity, mut shot, mut transform, reflection, returned) in &mut shots {
        shot.depth -= clock
            .scaled(returned.map_or_else(|| reflection.map_or(0x18000, |r| r.speed), |r| r.speed));
        shot.angle = shot.angle.wrapping_add(4 * clock.steps() as u8);
        transform.translation.z = -(shot.depth as f32 / LANE_UNIT as f32) * WORLD_UNIT + 0.06;
        transform.rotation = Quat::from_rotation_z(-(shot.angle as f32) * TAU / 256.0);
        if shot.depth < 0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
#[path = "tests/shots.rs"]
mod tests;
