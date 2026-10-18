//! Player weapons and lane-bound shots. The particle laser fires faster,
//! pierces enemies, and replaces solid bolts with colored particle rings.

use crate::cry;
use crate::explosion;
use crate::glider::Glider;
use crate::level::Level;
use crate::level::LevelActor;
use crate::mesh::colored_mesh;
use crate::powerups::Upgrades;
use crate::web::FRONT_RADIUS;
use crate::web::LANE_UNIT;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

pub(crate) const FIRE_INTERVAL: u8 = 4;

pub(crate) const MAX_BULLETS: usize = 8;

pub(crate) const BULLET_STEP: f32 = 3.5 * FRONT_RADIUS / 24.0;

#[derive(Component, Default)]
pub(crate) struct Weapon {
    pub(crate) cooldown: u8,
    pub(crate) firing: bool,
}

#[derive(Component)]
#[require(LevelActor, ShotSlot)]
pub(crate) struct Bullet {
    pub(crate) lane: usize,
}

#[derive(Component, Default)]
pub(crate) struct ShotSlot;

/// A player shot reads as a white-hot core inside a cyan-green glow. The
/// original draws a small rotating vector shape and lets bloom spread the
/// colour outwards, so these layers stand in for that glow. The shades are
/// sampled from a reference screenshot of the Jaguar original.
pub(crate) fn shot_mesh() -> Mesh {
    const SEGMENTS: usize = 8;
    const THICKNESS: f32 = 0.3;
    let shades = [
        Color::srgb_u8(236, 250, 254),
        Color::srgb_u8(14, 250, 155),
        Color::srgb_u8(0, 154, 53),
    ];
    let radii = [0.037, 0.08, 0.132];
    let point = |layer: usize, index: usize, z: f32| {
        let angle = index as f32 * std::f32::consts::TAU / SEGMENTS as f32;
        Vec3::new(
            angle.cos() * radii[layer],
            angle.sin() * radii[layer] * 1.05,
            z,
        )
    };

    let mut triangles = Vec::new();
    for z in [THICKNESS, -THICKNESS] {
        for index in 0..SEGMENTS {
            let next = (index + 1) % SEGMENTS;
            let (a, b) = (point(0, index, z), point(0, next, z));
            triangles.push([
                (Vec3::new(0.0, 0.0, z), shades[0]),
                (a, shades[0]),
                (b, shades[0]),
            ]);
            for layer in 0..radii.len() - 1 {
                let (near, far) = (shades[layer], shades[layer + 1]);
                let (a, b) = (point(layer, index, z), point(layer, next, z));
                let (c, d) = (point(layer + 1, index, z), point(layer + 1, next, z));
                triangles.push([(a, near), (b, near), (c, far)]);
                triangles.push([(a, near), (c, far), (d, far)]);
            }
        }
    }
    // Close the rim so the bolt stays solid when a shot is seen from the side.
    for index in 0..SEGMENTS {
        let next = (index + 1) % SEGMENTS;
        let (a, b) = (point(2, index, THICKNESS), point(2, next, THICKNESS));
        let (c, d) = (point(2, index, -THICKNESS), point(2, next, -THICKNESS));
        triangles.push([(a, shades[2]), (c, shades[2]), (d, shades[2])]);
        triangles.push([(a, shades[2]), (d, shades[2]), (b, shades[2])]);
    }
    colored_mesh(triangles)
}

#[derive(Resource, Default)]
pub(crate) struct ShotAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Component, Default)]
pub(crate) struct ParticleShot(pub(crate) Option<Handle<Mesh>>);

pub(crate) fn particle_shot_mesh(
    position: Vec3,
    camera: &Transform,
    projection: &PerspectiveProjection,
) -> Mesh {
    let relative = camera.rotation.inverse() * (position - camera.translation);
    let distance = (-relative.z).max(0.01);
    let depth = (camera.translation.z - position.z) / (FRONT_RADIUS / 24.0);
    let phase = depth as i32 as u8;
    let color = cry::color(
        explosion::pulse_hue(phase),
        (255.0 * (1.0 - depth / 325.0).clamp(0.0, 1.0)) as u8,
    );
    let dot = 4.0 * distance * (projection.fov * 0.5).tan() * projection.aspect_ratio / 384.0;
    // pring truncates the projected radius before multiplying its byte sines.
    let pixel = dot * 0.5;
    let radius = (8.0 * FRONT_RADIUS / 24.0 / pixel).floor();
    colored_mesh((0..4).flat_map(|index| {
        let angle = phase.wrapping_add(index * 64);
        let center = Vec3::new(
            (f32::from(crate::sines::SINES[angle as usize]) * radius / 256.0).floor(),
            -(f32::from(crate::sines::SINES[angle.wrapping_add(64) as usize]) * radius / 256.0)
                .floor(),
            0.0,
        ) * pixel;
        let quad = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| {
            (
                camera.rotation * (center + Vec3::new(x * dot * 0.5, y * dot * 0.5, 0.0)),
                color,
            )
        });
        [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
    }))
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_particle_shots(
    mut meshes: ResMut<Assets<Mesh>>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<ParticleShot>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut shots: Query<(&mut ParticleShot, &mut Mesh3d, &Transform)>,
) {
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    for (mut particle, mut mesh, transform) in &mut shots {
        let geometry = particle_shot_mesh(transform.translation, camera, projection);
        if let Some(handle) = &particle.0 {
            if let Some(existing) = meshes.get_mut(handle) {
                *existing = geometry;
            }
        } else {
            let handle = meshes.add(geometry);
            mesh.0 = handle.clone();
            particle.0 = Some(handle);
        }
    }
}

pub(crate) fn fire_bullets(
    level: Res<Level>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<ShotAssets>,
    mut glider: Single<(&Glider, &mut Weapon, &Transform)>,
    bullets: Query<Entity, With<ShotSlot>>,
    upgrades: Option<Res<Upgrades>>,
) {
    let (glider, weapon, transform) = &mut *glider;
    if !keys.any_pressed([KeyCode::KeyZ, KeyCode::Space]) {
        weapon.firing = false;
        return;
    }
    weapon.cooldown = weapon.cooldown.saturating_sub(1);
    if weapon.cooldown != 0 {
        return;
    }
    weapon.cooldown = if upgrades.as_ref().is_some_and(|u| u.laser) {
        3
    } else {
        FIRE_INTERVAL
    };
    if bullets.iter().count()
        >= MAX_BULLETS
            + if upgrades.as_ref().is_some_and(|u| u.droid) {
                8
            } else {
                0
            }
    {
        weapon.firing = false;
        return;
    }
    weapon.firing = true;
    crate::sound::play(
        &mut commands,
        if upgrades.as_ref().is_some_and(|u| u.laser) {
            crate::sound::Effect::Laser
        } else {
            crate::sound::Effect::Shot
        },
    );
    let lane = level.web.selected_lane(glider.progress) as usize;
    let position = transform.translation - Vec3::Z * 0.18;
    let mut shot = commands.spawn((
        Bullet { lane },
        RenderLayers::layer(1),
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_translation(position),
    ));
    if upgrades.as_ref().is_some_and(|u| u.laser) {
        shot.insert(ParticleShot::default());
    }
}

pub(crate) fn move_bullets(
    level: Res<Level>,
    mut commands: Commands,
    mut bullets: Query<(Entity, &Bullet, &mut Transform)>,
    upgrades: Option<Res<Upgrades>>,
) {
    for (entity, bullet, mut transform) in &mut bullets {
        let center = level
            .web
            .glider_transform(bullet.lane as i32 * LANE_UNIT)
            .translation;
        transform.translation.x = center.x;
        transform.translation.y = center.y;
        transform.translation.z -= if upgrades.as_ref().is_some_and(|u| u.laser) {
            4.5 * FRONT_RADIUS / 24.0
        } else {
            BULLET_STEP
        };
        if transform.translation.z <= -level.web.world_depth() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
#[path = "tests/weapons.rs"]
mod tests;
