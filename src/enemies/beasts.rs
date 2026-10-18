//! Beasties are armored, layered enemies that move as Flippers. Shooting their
//! armor detaches horns that return toward the glider as reflected shots.

use crate::cry;
use crate::enemies::Enemy;
use crate::enemies::flipper_render::FlipperAssets;
use crate::enemies::flippers::Flipper;
use crate::enemies::flippers::Motion;
use crate::enemies::flippers::flipper_transform;
use crate::enemies::tankers;
use crate::game::Game;
use crate::game::Status;
use crate::level::Level;
use crate::mesh::colored_mesh;
use crate::weapons::ShotSlot;
use crate::web::LANE_UNIT;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Beast {
    pub(crate) stage: usize,
}

#[derive(Component)]
#[require(ShotSlot)]
pub(crate) struct Reflected {
    pub(crate) speed: i32,
}

#[derive(Resource, Default)]
pub(crate) struct BeastAssets {
    pub(crate) frames: [Handle<Mesh>; 3],
    pub(crate) parts: [Handle<Mesh>; 2],
}

pub(crate) fn part(
    points: &[(f32, f32)],
    faces: &[(u8, [usize; 3], [u8; 3])],
    scale: f32,
    z: f32,
) -> Vec<[(Vec3, Color); 3]> {
    faces
        .iter()
        .map(|(hue, face, intensity)| {
            std::array::from_fn(|i| {
                let (x, y) = points[face[i]];
                (
                    Vec3::new((x - 9.) * 0.075 * scale, (9. - y) * 0.075 * scale, z),
                    cry::color(*hue, intensity[i]),
                )
            })
        })
        .collect()
}

pub(crate) fn setup(commands: &mut Commands, meshes: &mut Assets<Mesh>) {
    // Double the core so its upper corners meet the horn roots at (5, 1) and (13, 1).
    let body = part(
        &[
            (3., 7.),
            (7., 5.),
            (11., 5.),
            (15., 7.),
            (15., 11.),
            (11., 13.),
            (7., 13.),
            (3., 11.),
            (5., 7.),
            (7., 7.),
            (7., 9.),
            (13., 7.),
            (11., 7.),
            (11., 9.),
        ],
        &[
            (0x88, [0, 1, 2], [128, 64, 64]),
            (0x88, [0, 2, 3], [128, 64, 128]),
            (0x88, [0, 7, 4], [128, 96, 96]),
            (0x88, [0, 3, 4], [128, 128, 96]),
            (0x88, [7, 6, 4], [96, 64, 96]),
            (0x88, [6, 5, 4], [64, 64, 96]),
            (0xf0, [8, 9, 10], [192, 240, 128]),
            (0xf0, [11, 12, 13], [192, 240, 128]),
        ],
        2.,
        0.,
    );
    let faces = [
        (0xbb, [0, 1, 2], [240, 240, 192]),
        (0xbc, [1, 2, 3], [240, 192, 160]),
        (0xcc, [2, 3, 4], [192, 160, 160]),
        (0x66, [5, 6, 7], [160, 192, 255]),
    ];
    let left = part(
        &[
            (-7., -11.),
            (-7., 1.),
            (-3., -3.),
            (-3., 5.),
            (5., 1.),
            (17., 13.),
            (13., 13.),
            (13., 25.),
        ],
        &faces,
        1.,
        0.01,
    );
    let right = part(
        &[
            (25., -11.),
            (25., 1.),
            (21., -3.),
            (21., 5.),
            (13., 1.),
            (1., 13.),
            (5., 13.),
            (5., 25.),
        ],
        &faces,
        1.,
        0.02,
    );
    let frames = [
        meshes.add(colored_mesh(body.clone())),
        meshes.add(colored_mesh(body.iter().chain(&left).copied())),
        meshes.add(colored_mesh(
            body.into_iter().chain(left.clone()).chain(right.clone()),
        )),
    ];
    commands.insert_resource(BeastAssets {
        frames,
        parts: [
            meshes.add(colored_mesh(left)),
            meshes.add(colored_mesh(right)),
        ],
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<FlipperAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<tankers::Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(generator) = &level.beasts else {
        return;
    };
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    if !crate::difficulty::due(&mut game.beast_timer, clock.steps()) {
        return;
    }
    if crate::enemies::occupancy(&living, &tankers) >= level.flippers.max_active {
        return;
    }
    game.advance_random();
    let flipper = Flipper {
        lane: (game.random >> 16) as usize % level.web.lanes(),
        depth: level.web.rear() + 300 * LANE_UNIT,
        motion: Motion::Approach,
        pause: 0,
    };
    commands.spawn((
        flipper_transform(&flipper, &level.web),
        flipper,
        Beast { stage: 2 },
        RenderLayers::layer(1),
        Mesh3d(assets.point.clone()),
        MeshMaterial3d(assets.point_shades.first().cloned().unwrap_or_default()),
    ));
    game.remaining -= 1;
    game.beast_timer = generator.spawn_interval_ticks - 1;
}

#[cfg(test)]
#[path = "tests/beasts.rs"]
mod tests;
