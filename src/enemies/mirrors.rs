//! Mirrors spin and stop forty units from the rim. Surviving hits push them
//! backward and reflect the player's shot; five hits or rearward escape destroy them.

use bevy::{camera::visibility::RenderLayers, prelude::*};
use std::f32::consts::TAU;

use crate::{
    cry,
    enemies::{Enemy, flipper_render::draw_approach_dot, tankers::Tanker},
    game::{Game, Status},
    level::{Level, LevelActor},
    mesh::colored_mesh,
    weapons::ShotSlot,
    web::{LANE_UNIT, WORLD_UNIT, Web},
};

#[derive(Component)]
#[require(LevelActor, Enemy)]
pub(crate) struct Mirror {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) approach: bool,
    pub(crate) hits: u8,
    pub(crate) angle: u8,
}

impl Mirror {
    pub(crate) fn target(&self) -> Option<(usize, i32)> {
        (!self.approach).then_some((self.lane, self.depth))
    }

    pub(crate) fn hit(&mut self, speed: i32, rear: i32) -> bool {
        self.hits -= 1;
        self.depth += speed * 16;
        self.hits == 0 || self.depth > rear
    }

    pub(crate) fn transform(&self, web: &Web) -> Transform {
        let mut pose = web.glider_transform(self.lane as i32 * LANE_UNIT);
        pose.translation.z = -self.depth as f32 / LANE_UNIT as f32 * WORLD_UNIT + 0.06;
        pose.rotation =
            web.orientation(self.lane) * Quat::from_rotation_z(-f32::from(self.angle) * TAU / 256.);
        pose
    }
}

#[derive(Component)]
#[require(ShotSlot)]
pub(crate) struct ReturnedShot {
    pub(crate) speed: i32,
}

#[derive(Resource, Default)]
pub(crate) struct MirrorAssets {
    pub(crate) body: Handle<Mesh>,
    pub(crate) reflection: Handle<Mesh>,
    pub(crate) point: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) shades: Vec<Handle<StandardMaterial>>,
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let points = [
        (1., 5.),
        (9., 1.),
        (15., 5.),
        (15., 13.),
        (9., 17.),
        (1., 13.),
        (9., 9.),
    ]
    .map(|(x, y)| Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.));
    let faces = [
        (0x88, [0, 1, 6], [255, 255, 255]),
        (0x87, [1, 2, 6], [255, 192, 255]),
        (0x86, [2, 3, 6], [192, 128, 255]),
        (0x85, [3, 4, 6], [128, 128, 255]),
        (0x86, [4, 5, 6], [128, 192, 255]),
        (0x87, [5, 0, 6], [192, 255, 255]),
    ];
    let ring = colored_mesh((0..16).flat_map(|segment| {
        let point = |radius: f32, index: usize| {
            let angle = index as f32 * TAU / 16.;
            (
                Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.),
                cry::color(0x8a, 255),
            )
        };
        let a = point(0.12, segment);
        let b = point(0.12, segment + 1);
        let c = point(0.2, segment + 1);
        let d = point(0.2, segment);
        [[a, b, c], [a, c, d]]
    }));
    commands.insert_resource(MirrorAssets {
        body: meshes.add(colored_mesh(faces.map(|(hue, face, intensity)| {
            std::array::from_fn(|i| (points[face[i]], cry::color(hue, intensity[i])))
        }))),
        reflection: meshes.add(ring),
        point: meshes.add(Rectangle::new(1., 1.)),
        material: materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        shades: (0..=255)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: cry::color(0x88, i),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });
}

pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<MirrorAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(generator) = &level.mirrors else {
        return;
    };
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    if !crate::difficulty::due(&mut game.mirror_timer, clock.steps()) {
        return;
    }
    if crate::enemies::occupancy(&living, &tankers) >= level.flippers.max_active {
        return;
    }
    game.advance_random();
    let mirror = Mirror {
        lane: (game.random >> 16) as usize % level.web.lanes(),
        depth: level.web.rear() + 300 * LANE_UNIT,
        approach: true,
        hits: 5,
        angle: 0,
    };
    commands.spawn((
        mirror.transform(&level.web),
        mirror,
        RenderLayers::layer(1),
        Mesh3d(assets.point.clone()),
        MeshMaterial3d(assets.shades.first().cloned().unwrap_or_default()),
    ));
    game.remaining -= 1;
    game.mirror_timer = generator.spawn_interval_ticks - 1;
}

pub(crate) fn advance(
    level: Res<Level>,
    game: Res<Game>,
    assets: Res<MirrorAssets>,
    mut mirrors: Query<(
        &mut Mirror,
        &mut Transform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(generator) = &level.mirrors else {
        return;
    };
    if game.status != Status::Playing {
        return;
    }
    for (mut mirror, mut pose, mut mesh, mut material) in &mut mirrors {
        mirror.angle = mirror.angle.wrapping_add(clock.steps() as u8);
        if mirror.approach {
            mirror.depth -= clock.scaled(2 * LANE_UNIT);
            if mirror.depth <= level.web.rear() {
                mirror.depth = level.web.rear();
                mirror.approach = false;
                mesh.0 = assets.body.clone();
                material.0 = assets.material.clone();
            }
        } else {
            mirror.depth = (mirror.depth - clock.scaled(generator.climb_speed)).max(40 * LANE_UNIT);
        }
        *pose = mirror.transform(&level.web);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_points(
    assets: Res<MirrorAssets>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<Mirror>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut mirrors: Query<(
        &Mirror,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    for (mirror, mut pose, mut material) in &mut mirrors {
        if mirror.approach {
            draw_approach_dot(&mut pose, &mut material, &assets.shades, camera, projection);
        }
    }
}

#[cfg(test)]
#[path = "tests/mirrors.rs"]
mod tests;
