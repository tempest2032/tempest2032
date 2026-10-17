use super::{
    Messages,
    font::{Font, GlyphTriangle},
};
use bevy::{asset::RenderAssetUsages, camera::visibility::RenderLayers, prelude::*};
use std::collections::HashMap;

#[derive(Component)]
struct Graphic;

#[derive(Resource)]
pub(super) struct Assets {
    mesh: Handle<Mesh>,
    layouts: HashMap<super::Id, Vec<GlyphTriangle>>,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<bevy::asset::Assets<Mesh>>,
    mut materials: ResMut<bevy::asset::Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(crate::mesh::colored_mesh(Vec::new()));
    // The overlay camera shares the former 2D layer's 240-line orthographic view.
    commands.spawn((
        Graphic,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0., 0., crate::cry::BEHIND_HUD),
        RenderLayers::layer(3),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.insert_resource(Assets {
        mesh,
        layouts: HashMap::new(),
    });
}

pub(super) fn draw(
    messages: Res<Messages>,
    font: Res<Font>,
    mut assets: ResMut<Assets>,
    mut meshes: ResMut<bevy::asset::Assets<Mesh>>,
    cameras: Query<(&Camera, &GlobalTransform), super::geometry::GameCameras>,
    mut drawn: Local<Vec<(super::Id, f32, Vec2)>>,
) {
    let mut frame = Vec::with_capacity(messages.rewards.len());
    for active in &messages.rewards {
        let center = match active.event.origin {
            // Score labels follow their world origin, so the camera can move them.
            Some(origin) => {
                let Some(center) = cameras.iter().find_map(|(camera, pose)| {
                    let point = camera.world_to_viewport(pose, origin).ok()?;
                    let size = camera.logical_viewport_size()?;
                    Some(Vec2::new(point.x - size.x / 2., size.y / 2. - point.y) * (240. / size.y))
                }) else {
                    continue;
                };
                center
            }
            None => Vec2::ZERO,
        };
        frame.push((active.event.id, active.age as f32, center));
    }
    // Paused and between-tick frames repeat the last geometry.
    if *drawn == frame {
        return;
    }
    let mut triangles = Vec::new();
    for &(id, age, center) in &frame {
        let layout = assets.layouts.entry(id).or_insert_with(|| font.layout(id));
        let score = matches!(
            id,
            super::Id::Points250 | super::Id::Points500 | super::Id::Points750
        );
        let scale = if score {
            (2. + age) * 0.16
        } else {
            (age / 16.) * 128. / ((210. - age).max(1.) + 80.)
        };
        let alpha = if score {
            (1. - (age - 50.).max(0.) / 30.).max(0.)
        } else {
            1.
        };
        for glyph in layout.iter() {
            let layers = if id == super::Id::OneUp && glyph.depth_layers {
                6
            } else {
                1
            };
            for layer in 0..layers {
                let depth = 290. - age + (1. - layer as f32) * age / 4.;
                if depth <= 4. {
                    continue;
                }
                let layered = layers > 1;
                let scale = if layered {
                    age / 16. * 128. / depth
                } else {
                    scale
                };
                triangles.push(std::array::from_fn(|index| {
                    let offset = if layered {
                        Vec2::new(0., -18.)
                    } else {
                        Vec2::ZERO
                    };
                    let local = glyph.origin
                        + cell_point(
                            glyph.points[index] - offset,
                            glyph.cell - offset,
                            Vec2::splat(scale),
                            0.,
                        )
                        + if layered {
                            Vec2::new(0., -8. * 128. / depth)
                        } else {
                            Vec2::ZERO
                        };
                    (
                        (center + Vec2::new(local.x, -local.y)).extend(0.),
                        glyph.colors[index].with_alpha(alpha),
                    )
                }));
            }
        }
    }
    if let Some(mesh) = meshes.get_mut(&assets.mesh) {
        *mesh = crate::mesh::colored_mesh(triangles);
        mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
    }
    *drawn = frame;
}

fn cell_point(point: Vec2, center: Vec2, scale: Vec2, shear: f32) -> Vec2 {
    let local = center * scale + (point - center) * scale.min(Vec2::ONE);
    Vec2::new(local.x + local.y * shear, local.y - local.x * shear * 0.5)
}

#[cfg(test)]
#[path = "../tests/message_render.rs"]
mod tests;
