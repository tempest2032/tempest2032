//! Warp-zone polygons, particle scenes, CRY feedback, and crosshair.

use super::{GATES, Phase, SCALE, ZoneKind};
use crate::{
    cry,
    game::{Game, Status},
    mesh::colored_mesh,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::f32::consts::TAU;

// horse.gas shifts vertex × signed-byte sine by 8, giving 1/2-sized polygons.
const POLYGON_SCALE: f32 = SCALE * 0.5;

#[derive(Component, Clone)]
pub(crate) struct SavedCamera {
    pub(crate) pose: Transform,
    pub(crate) projection: Option<Projection>,
}

#[derive(Component)]
pub(crate) enum WarpNode {
    Ground,
    Gate(usize),
    Icon(usize),
    Tunnel,
    Rings,
}

#[derive(Component)]
pub(crate) struct Crosshair;

#[derive(Resource)]
pub(crate) struct WarpAssets {
    ground: Handle<Mesh>,
    tunnel: Handle<Mesh>,
    rings: Handle<Mesh>,
    feedback: Handle<Image>,
    feedback_material: Handle<StandardMaterial>,
    gates: [Handle<Mesh>; 2],
    gate_materials: [Handle<StandardMaterial>; GATES],
    icons: [Handle<Mesh>; 5],
}

const SURFACE_WIDTH: u32 = 320;
const SURFACE_HEIGHT: u32 = 128;
const SURFACE_CRY: &[u8; (SURFACE_WIDTH * SURFACE_HEIGHT * 2) as usize] =
    include_bytes!("../../assets/warp/beasty6.cry");

fn surface_image() -> Image {
    let mut pixels = Vec::with_capacity((SURFACE_WIDTH * SURFACE_HEIGHT * 4) as usize);
    for pixel in SURFACE_CRY.as_chunks::<2>().0 {
        let cyan = usize::from(pixel[0] >> 4);
        let red = usize::from(pixel[0] & 15);
        let intensity = u16::from(pixel[1]);
        for table in [&cry::RED, &cry::GREEN, &cry::BLUE] {
            pixels.push(((u16::from(table[cyan][red]) * intensity) >> 8) as u8);
        }
        pixels.push(255);
    }
    let mut image = Image::new(
        Extent3d {
            width: SURFACE_WIDTH,
            height: SURFACE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        // Mode 7 reflects the texture's vertical offset around its 128-row midpoint.
        address_mode_v: bevy::image::ImageAddressMode::MirrorRepeat,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });
    image
}

fn crosshair_image() -> Image {
    let pixels = include_bytes!("../../assets/warp/crosshair.cry")
        .as_chunks::<2>()
        .0
        .iter()
        .flat_map(|pixel| {
            let color = cry::color(pixel[0], pixel[1]).to_srgba();
            [
                (color.red * 255.) as u8,
                (color.green * 255.) as u8,
                (color.blue * 255.) as u8,
                if *pixel == [0, 0] { 0 } else { 255 },
            ]
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: 40,
            height: 39,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

fn gate_mesh(alternate: bool) -> Mesh {
    let vertices = [
        Vec3::new(-17., 0., 0.),
        Vec3::new(-14., 0., 0.),
        Vec3::new(-13., 3., 0.),
        Vec3::new(-13., -3., 0.),
    ];
    colored_mesh((0..8).flat_map(|arm| {
        let rotation = Quat::from_rotation_z(arm as f32 * TAU / 8.);
        [
            ([2, 1, 0], if alternate { 0x62 } else { 0x8c }),
            ([3, 1, 0], if alternate { 0x66 } else { 0x8f }),
        ]
        .map(|(face, hue)| {
            std::array::from_fn(|i| {
                (
                    rotation * vertices[face[i]] * POLYGON_SCALE,
                    cry::color(hue, [192, 255, 64][i]),
                )
            })
        })
    }))
}

#[allow(clippy::type_complexity)]
fn icon_mesh(kind: usize) -> Mesh {
    let (points, faces, hue): (&[(f32, f32)], &[[usize; 3]], u8) = match kind {
        1 | 2 => (
            &[
                (5., 7.),
                (9., 3.),
                (13., 7.),
                (7., 7.),
                (11., 7.),
                (7., 13.),
                (11., 13.),
            ],
            &[[0, 1, 2], [3, 4, 5], [4, 5, 6]],
            0xef,
        ),
        3 => (
            &[
                (9., 5.),
                (5., 9.),
                (7., 9.),
                (11., 9.),
                (13., 9.),
                (9., 9.),
                (5., 13.),
                (7., 13.),
                (11., 13.),
                (13., 13.),
            ],
            &[[0, 1, 2], [0, 3, 4], [5, 6, 7], [5, 8, 9]],
            0x36,
        ),
        _ => (
            &[(9., 13.), (9., 1.), (17., 17.), (1., 17.)],
            &[[1, 2, 0], [2, 3, 0], [3, 1, 0]],
            0xfc,
        ),
    };
    colored_mesh(faces.iter().map(|face| {
        std::array::from_fn(|i| {
            let (x, y) = points[face[i]];
            let point =
                Vec3::new(x - 9., 9. - y, 0.) * POLYGON_SCALE * if kind == 2 { -1. } else { 1. };
            let intensity = if matches!(kind, 1 | 2) {
                // Shared arrow corners must have the same shade across triangle edges.
                (255. - (y - 3.) * 12.7) as u8
            } else {
                [255, 192, 128][i]
            };
            (point, cry::color(hue, intensity))
        })
    }))
}

const GROUND_WIDTH: usize = 32;
const GROUND_DEPTH: usize = 48;

fn ground_rows() -> impl Iterator<Item = (f32, f32)> {
    (0..=GROUND_DEPTH).map(|row| {
        let depth = ((row as f32 / GROUND_DEPTH as f32) * 2000.0_f32.ln()).exp() - 1.;
        (depth, -depth * SCALE)
    })
}

fn ground_column(column: usize) -> f32 {
    (column as f32 - 16.) * 16.
}

/// Only the texture coordinates scroll, so frames rewrite just this attribute.
fn ground_uvs(distance: f32, tick: u32) -> Vec<[f32; 2]> {
    let mut uvs = Vec::with_capacity((GROUND_DEPTH + 1) * (GROUND_WIDTH + 1));
    for (_, z) in ground_rows() {
        // Match Mode 7's scanline depth to our 240-line, 1.2-radian projection.
        let mode7_depth = -z * 8. * 0.6_f32.tan() / (120. * SCALE);
        let sine = (mode7_depth * 32. * TAU / 256. + tick as f32 * TAU / 256.).sin();
        let texture_scale = 0.25 - (sine + 1.) * (255. * 4. / 65536.);
        let v = (distance / SCALE + mode7_depth * texture_scale * (0xff000 as f32 / 65536.))
            / SURFACE_HEIGHT as f32;
        uvs.extend((0..=GROUND_WIDTH).map(|column| {
            [
                0.5 + ground_column(column) * (texture_scale / 0.25)
                    / (SURFACE_WIDTH as f32 * SCALE),
                v,
            ]
        }));
    }
    uvs
}

fn ground_mesh(distance: f32, tick: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for (depth, z) in ground_rows() {
        let brightness = (1. - depth / 2000.).max(0.05);
        for column in 0..=GROUND_WIDTH {
            positions.push([ground_column(column), 0., z]);
            colors.push([brightness, brightness, brightness, 1.]);
        }
    }
    let mut indices = Vec::new();
    for row in 0..GROUND_DEPTH {
        for column in 0..GROUND_WIDTH {
            let a = (row * (GROUND_WIDTH + 1) + column) as u32;
            let b = a + 1;
            let c = a + (GROUND_WIDTH + 1) as u32;
            indices.extend([a, b, c, b, c + 1, c]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; positions.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, ground_uvs(distance, tick))
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn pixel_mesh(pixels: &[(Vec2, Color)], fov: f32) -> Mesh {
    let pixel = 20. * (fov * 0.5).tan() / 240.;
    let visible = || {
        pixels
            .iter()
            .filter(|(p, _)| p.x.abs() <= 192. && p.y.abs() <= 120.)
    };
    let vertices = visible().count() * 6;
    let mut positions = Vec::with_capacity(vertices);
    let mut colors = Vec::with_capacity(vertices);
    for (p, color) in visible() {
        let center = Vec3::new(p.x * pixel, -p.y * pixel, -10.);
        let color = LinearRgba::from(*color).to_f32_array();
        for (x, y) in [
            (-0.5, -0.5),
            (0.5, -0.5),
            (0.5, 0.5),
            (-0.5, -0.5),
            (0.5, 0.5),
            (-0.5, 0.5),
        ] {
            positions.push((center + Vec3::new(x, y, 0.) * pixel).to_array());
            colors.push(color);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; positions.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

fn screen_mesh(aspect_ratio: f32) -> Mesh {
    let y = 10. * (1.2_f32 * 0.5).tan();
    let x = y * aspect_ratio;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-x, -y, -10.], [x, -y, -10.], [x, y, -10.], [-x, y, -10.]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

fn gate_views(session: &super::Session) -> Vec<super::Gate> {
    if session.kind != ZoneKind::GateFlight {
        return Vec::new();
    }
    // Six visible gates plus the seventh gate used as the opacity endpoint.
    let mut gates: Vec<_> = session.gates.iter().take(7).copied().collect();
    let mut distance = session.generation_distance;
    let mut wait = session.generation_wait;
    let mut travel = 0;
    let mut ticks = 0_u16;
    for index in session.cursor..GATES {
        if gates.len() == 7 {
            break;
        }
        loop {
            ticks = ticks.wrapping_add(1);
            travel += session.speed;
            distance -= session.speed;
            if distance < 0 {
                distance += 12 * super::UNIT;
                wait -= 1;
                if wait < 0 {
                    break;
                }
            }
        }
        gates.push(super::Gate {
            index,
            distance: 255 * super::UNIT + travel,
            // Count up to age zero so the preview keeps rotating through its real spawn.
            age: 0_u16.wrapping_sub(ticks),
        });
        wait = (super::courses::COURSES[session.course][index] >> 24) as i16;
    }
    gates
}

fn gate_opacity(gates: &[super::Gate], rank: usize) -> f32 {
    if rank < 3 {
        return 1.;
    }
    if rank >= 6 {
        return 0.;
    }
    let near = gates[2].distance;
    let far = gates.get(6).map_or_else(
        || {
            let last = gates.last().unwrap().distance;
            last + (last - gates[gates.len() - 2].distance).max(1)
        },
        |gate| gate.distance,
    );
    (far - gates[rank].distance) as f32 / (far - near) as f32
}

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    super::failure::setup(&mut commands, &mut images);
    super::victory::setup(&mut commands, &mut images, &mut meshes, &mut materials);
    let gates = [meshes.add(gate_mesh(false)), meshes.add(gate_mesh(true))];
    let icons = std::array::from_fn(|kind| meshes.add(icon_mesh(kind)));
    let material = materials.add(StandardMaterial {
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let ground = meshes.add(ground_mesh(0., 0));
    let tunnel = meshes.add(colored_mesh([]));
    let rings = meshes.add(screen_mesh(1.6));
    let feedback = images.add(cry::feedback_image());
    let feedback_material = materials.add(StandardMaterial {
        base_color_texture: Some(feedback.clone()),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        WarpNode::Rings,
        Mesh3d(rings.clone()),
        MeshMaterial3d(feedback_material.clone()),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(1),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.spawn((
        WarpNode::Tunnel,
        Mesh3d(tunnel.clone()),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(1),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    let ground_material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(surface_image())),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        WarpNode::Ground,
        Mesh3d(ground.clone()),
        MeshMaterial3d(ground_material.clone()),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(0),
    ));
    let gate_materials = std::array::from_fn(|_| {
        materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        })
    });
    for (index, material) in gate_materials.iter().enumerate() {
        commands.spawn((
            WarpNode::Gate(index),
            Mesh3d(gates[0].clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            RenderLayers::layer(1),
        ));
        commands.spawn((
            WarpNode::Icon(index),
            Mesh3d(icons[0].clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            RenderLayers::layer(1),
        ));
    }
    commands.insert_resource(WarpAssets {
        ground,
        tunnel,
        rings,
        feedback,
        feedback_material,
        gates,
        gate_materials,
        icons,
    });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(31),
        ))
        .with_children(|parent| {
            parent.spawn((
                Crosshair,
                ImageNode::new(images.add(crosshair_image())),
                Visibility::Hidden,
                Node {
                    height: percent(39. / 240. * 100.),
                    aspect_ratio: Some(40. / 39.),
                    ..default()
                },
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn draw(
    game: Res<Game>,
    assets: Res<WarpAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<
        (&mut Transform, &mut Projection),
        (
            With<Camera3d>,
            Without<WarpNode>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut nodes: Query<
        (&WarpNode, &mut Transform, &mut Mesh3d, &mut Visibility),
        (Without<Camera3d>, With<WarpNode>),
    >,
    mut crosshairs: Query<&mut Visibility, (With<Crosshair>, Without<Text>, Without<WarpNode>)>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut screen_aspect: Local<f32>,
) {
    let bonus = game.status == Status::Bonus;
    let session = game.bonus.as_ref().filter(|_| bonus);
    for mut visible in &mut crosshairs {
        visible.set_if_neq(
            if session.is_some_and(|s| {
                s.kind != ZoneKind::Tunnel
                    && (matches!(s.phase, Phase::Flying | Phase::Success(_))
                        || s.kind == ZoneKind::GateFlight && matches!(s.phase, Phase::Failure(_)))
            }) {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        );
    }
    let Some(session) = session.filter(|s| {
        s.kind == ZoneKind::Tunnel || !matches!(s.phase, Phase::Victory(_) | Phase::Fade(_))
    }) else {
        for (_, _, _, mut visible) in &mut nodes {
            visible.set_if_neq(Visibility::Hidden);
        }
        return;
    };
    let gates = gate_views(session);
    if session.kind == ZoneKind::GateFlight {
        for (rank, gate) in gates.iter().take(6).enumerate() {
            let handle = &assets.gate_materials[gate.index];
            let color = Color::srgba(1., 1., 1., gate_opacity(&gates, rank));
            let mode = if rank < 3 {
                AlphaMode::Opaque
            } else {
                AlphaMode::Blend
            };
            if materials
                .get(handle)
                .is_some_and(|m| m.base_color != color || m.alpha_mode != mode)
                && let Some(material) = materials.get_mut(handle)
            {
                material.base_color = color;
                material.alpha_mode = mode;
            }
        }
    }
    let failure_age = if session.kind == ZoneKind::GateFlight {
        if let Phase::Failure(ticks) | Phase::Success(ticks) = session.phase {
            151 - ticks.min(151)
        } else {
            0
        }
    } else {
        0
    };
    let title = format!(
        "Tempest 2032 | {} | Course {}",
        session.kind.label(),
        session.course + 1
    );
    for mut window in &mut windows {
        if window.title != title {
            window.title.clone_from(&title);
        }
    }
    let position = if session.kind != ZoneKind::GateFlight {
        Vec2::ZERO
    } else {
        session.position()
    };
    for (mut pose, mut projection) in &mut cameras {
        pose.set_if_neq(Transform::from_xyz(
            position.x * SCALE,
            -position.y * SCALE,
            0.,
        ));
        let Projection::Perspective(perspective) = projection.bypass_change_detection() else {
            continue;
        };
        let aspect_ratio = perspective.aspect_ratio;
        if (perspective.fov, perspective.near, perspective.far) != (1.2, 0.01, 1000.) {
            perspective.fov = 1.2;
            perspective.near = 0.01;
            perspective.far = 1000.;
            projection.set_changed();
        }
        if session.rings.is_some()
            && *screen_aspect != aspect_ratio
            && let Some(mesh) = meshes.get_mut(&assets.rings)
        {
            *screen_aspect = aspect_ratio;
            *mesh = screen_mesh(aspect_ratio);
        }
    }
    // The session only advances on simulation ticks, which always change `Game`.
    let advanced = game.is_changed();
    if advanced
        && session.kind == ZoneKind::GateFlight
        && let Some(mesh) = meshes.get_mut(&assets.ground)
    {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            ground_uvs(
                session.distance as f32 / 65536. * SCALE,
                session.tick.saturating_sub(u32::from(failure_age)),
            ),
        );
    }
    if advanced
        && let Some(rings) = &session.rings
        && let Some(image) = images.get_mut(&assets.feedback)
    {
        // Updating the texture replaces its GPU view; refresh the material binding too.
        let _ = materials.get_mut(&assets.feedback_material);
        image
            .data
            .get_or_insert_with(|| vec![0; cry::FEEDBACK_WIDTH * cry::FEEDBACK_HEIGHT * 4]);
        let phase = match session.phase {
            Phase::Failure(ticks) | Phase::Success(ticks) => (151 - ticks.min(151)) % 4,
            _ => 0,
        };
        if phase == 0 {
            cry::write_feedback(image, &rings.pixels, &rings.pixels, 0);
        } else {
            let mut next = rings.clone();
            next.feedback(
                i32::from(matches!(session.phase, Phase::Failure(_))),
                Vec2::splat(500. / 256.),
                -8 * super::UNIT,
            );
            cry::write_feedback(image, &rings.pixels, &next.pixels, u32::from(phase));
        }
    }
    for (node, mut pose, mut mesh, mut visible) in &mut nodes {
        let shown = match *node {
            WarpNode::Rings => session.rings.is_some(),
            WarpNode::Tunnel => {
                if advanced
                    && let Some(tunnel) = &session.tunnel
                    && let Some(mesh) = meshes.get_mut(&assets.tunnel)
                {
                    let tick = if let Phase::Failure(ticks) | Phase::Fade(ticks) = session.phase {
                        session.tick.saturating_sub(u32::from(151 - ticks.min(151)))
                    } else {
                        session.tick
                    };
                    *mesh = pixel_mesh(&tunnel.pixels(tick), 1.2);
                }
                session.tunnel.is_some()
            }
            WarpNode::Ground => session.kind == ZoneKind::GateFlight,
            WarpNode::Gate(index) | WarpNode::Icon(index) => 'gate: {
                if session.kind != ZoneKind::GateFlight {
                    break 'gate false;
                }
                let Some(gate) = gates.iter().take(6).find(|g| g.index == index) else {
                    break 'gate false;
                };
                let (x, y, kind) = session.spec(index);
                if (y < 0) != (position.y < 0.) {
                    break 'gate false;
                }
                let mut target = Transform::from_xyz(
                    f32::from(x) * SCALE,
                    -f32::from(y) * SCALE,
                    -gate.distance as f32 / 65536. * SCALE,
                );
                let handle = if matches!(*node, WarpNode::Gate(_)) {
                    target.rotation =
                        Quat::from_rotation_z(-f32::from(gate.age as u8) * TAU / 256.);
                    &assets.gates[usize::from(index % 3 == 2)]
                } else if kind > 0 {
                    target.translation.z += 0.01;
                    &assets.icons[usize::from(kind)]
                } else {
                    break 'gate false;
                };
                pose.set_if_neq(target);
                if mesh.0 != *handle {
                    mesh.0 = handle.clone();
                }
                true
            }
        };
        visible.set_if_neq(if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;
    use crate::warp::{Gate, Session, ZoneKind};

    #[test]
    fn three_gates_are_opaque_then_three_fade_by_distance_and_the_rest_are_hidden() {
        let gates: Vec<_> = [10, 20, 40, 60, 100, 150, 210, 280, 360, 450]
            .into_iter()
            .enumerate()
            .map(|(index, distance)| Gate {
                index,
                distance,
                age: 0,
            })
            .collect();
        let opacity: Vec<_> = (0..gates.len())
            .map(|rank| gate_opacity(&gates, rank))
            .collect();
        assert_eq!(&opacity[..3], &[1.; 3]);
        assert!(opacity[3..6].iter().all(|alpha| *alpha > 0. && *alpha < 1.));
        assert!(opacity[2..7].windows(2).all(|pair| pair[0] > pair[1]));
        assert_eq!(&opacity[6..], &[0.; 4]);
        assert_eq!(opacity[3], 150. / 170.);
        assert_eq!(opacity[5], 60. / 170.);
        let mut farther = gates.clone();
        farther[3].distance = 80;
        assert!(gate_opacity(&farther, 3) < opacity[3]);
        assert!(gate_opacity(&gates[..4], 3) > 0.);
    }

    #[test]
    fn gate_and_icon_materials_share_opacity_and_no_more_than_six_gates_are_drawn() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Game>()
            .add_systems(Startup, setup)
            .add_systems(Update, draw);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(Session::new(ZoneKind::GateFlight, 0, Some(1), 0));
        app.update();
        let mut nodes = app
            .world_mut()
            .query::<(&WarpNode, &Visibility, &MeshMaterial3d<StandardMaterial>)>();
        let assets = app.world().resource::<WarpAssets>();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let mut visible_gates = 0;
        for (node, visibility, material) in nodes.iter(app.world()) {
            let (WarpNode::Gate(index) | WarpNode::Icon(index)) = *node else {
                continue;
            };
            assert_eq!(material.0, assets.gate_materials[index]);
            if index >= 6 {
                assert_eq!(*visibility, Visibility::Hidden);
            } else {
                if matches!(node, WarpNode::Gate(_)) {
                    assert_eq!(*visibility, Visibility::Visible);
                    visible_gates += 1;
                }
                let material = materials.get(&material.0).unwrap();
                if index < 3 {
                    assert_eq!(material.base_color.to_linear(), Color::WHITE.to_linear());
                    assert_eq!(material.alpha_mode, AlphaMode::Opaque);
                } else {
                    assert!(material.base_color.alpha() > 0. && material.base_color.alpha() < 1.);
                    assert_eq!(material.alpha_mode, AlphaMode::Blend);
                }
            }
        }
        assert_eq!(visible_gates, 6);
    }

    #[test]
    fn distant_gate_previews_cover_the_visible_range_and_join_live_gates_without_popping() {
        for course in 1..=8 {
            let mut session = Session::new(ZoneKind::GateFlight, 0, Some(course), 0);
            let previews = gate_views(&session);
            assert_eq!(previews.len(), 7);
            assert!(
                previews
                    .windows(2)
                    .all(|gates| gates[0].distance < gates[1].distance)
            );
            assert!(previews.last().unwrap().distance > 255 * super::super::UNIT);
            assert!(session.gates.is_empty());
            assert_eq!(
                session.cursor, 0,
                "previewing must not generate gameplay gates"
            );
            for _ in 0..450 {
                let before = gate_views(&session);
                let speed = session.speed;
                session.step_flight(0, 0);
                for gate in &session.gates {
                    let preview = before
                        .iter()
                        .find(|preview| preview.index == gate.index)
                        .unwrap();
                    assert_eq!(preview.distance - speed, gate.distance);
                    assert_eq!(preview.age.wrapping_add(1), gate.age);
                }
            }
        }
    }

    #[test]
    fn rings_and_tunnel_isolate_their_scene_and_upload_visible_feedback() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Game>()
            .init_resource::<Level>()
            .add_systems(Startup, setup)
            .add_systems(Update, draw);
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_xyz(5., 5., 0.),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        for kind in [ZoneKind::Tunnel, ZoneKind::Rings] {
            let mut session = Session::new(kind, 0, Some(1), 0);
            if let Some(rings) = &mut session.rings {
                for tick in 1..=60 {
                    rings.advance(
                        tick,
                        Vec2::ZERO,
                        &std::collections::VecDeque::from([Gate {
                            index: 0,
                            distance: 200 * 65536,
                            age: 0,
                        }]),
                        &[(0, 16, 0)],
                        Phase::Flying,
                    );
                }
            }
            let mut game = app.world_mut().resource_mut::<Game>();
            game.status = Status::Bonus;
            game.bonus = Some(session);
            app.update();
            let visible: Vec<_> = app
                .world_mut()
                .query::<(&WarpNode, &Visibility)>()
                .iter(app.world())
                .filter(|(_, visibility)| **visibility == Visibility::Visible)
                .map(|(node, _)| node)
                .collect();
            assert_eq!(visible.len(), 1);
            assert!(matches!(
                (kind, visible[0]),
                (ZoneKind::Tunnel, WarpNode::Tunnel) | (ZoneKind::Rings, WarpNode::Rings)
            ));
            if kind == ZoneKind::Rings {
                let assets = app.world().resource::<WarpAssets>();
                let pixels = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&assets.feedback)
                    .unwrap()
                    .data
                    .as_ref()
                    .unwrap();
                assert!(
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|rgba| rgba[..3] != [0, 0, 0])
                        .count()
                        > 128
                );
            }
        }
        app.world_mut().resource_mut::<Game>().status = Status::Playing;
        app.update();
        assert!(
            app.world_mut()
                .query::<(&WarpNode, &Visibility)>()
                .iter(app.world())
                .all(|(_, v)| *v == Visibility::Hidden)
        );
    }

    #[test]
    fn failed_gate_keeps_the_source_scene_frozen_for_bitmap_feedback() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Game>()
            .add_systems(Startup, setup)
            .add_systems(Update, draw);
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
            ))
            .id();
        let mut session = Session::new(ZoneKind::GateFlight, 0, Some(1), 0);
        session.tick = 200;
        session.phase = Phase::Failure(151);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(session);
        app.update();
        let ground = app.world().resource::<WarpAssets>().ground.clone();
        let uvs = |app: &App| {
            let meshes = app.world().resource::<Assets<Mesh>>();
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(values)) =
                meshes.get(&ground).unwrap().attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!()
            };
            values.clone()
        };
        let frozen_uvs = uvs(&app);
        let crosshair = app
            .world_mut()
            .query_filtered::<Entity, With<Crosshair>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Visibility>(crosshair),
            Some(&Visibility::Visible)
        );
        let parent = app.world().get::<ChildOf>(crosshair).unwrap().parent();
        assert!(app.world().get::<GlobalZIndex>(parent).unwrap().0 > 30);
        let mut game = app.world_mut().resource_mut::<Game>();
        let session = game.bonus.as_mut().unwrap();
        session.tick += 20;
        session.phase = Phase::Failure(131);
        app.update();
        let pose = app.world().get::<Transform>(camera).unwrap();
        assert_eq!(pose.rotation, Quat::IDENTITY);
        let Projection::Perspective(projection) = app.world().get::<Projection>(camera).unwrap()
        else {
            panic!()
        };
        assert_eq!(projection.fov, 1.2);
        assert_eq!(frozen_uvs, uvs(&app));
        assert_eq!(
            app.world().get::<Visibility>(crosshair),
            Some(&Visibility::Visible)
        );
        app.world_mut().resource_mut::<Game>().status = Status::Playing;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(crosshair),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn gate_and_icon_sizes_match_the_original_polygon_renderer() {
        use bevy::mesh::VertexAttributeValues;

        for alternate in [false, true] {
            let mesh = gate_mesh(alternate);
            let VertexAttributeValues::Float32x3(positions) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!()
            };
            let radii = positions
                .iter()
                .map(|p| Vec3::from_array(*p).length() / SCALE);
            assert!((radii.clone().fold(0., f32::max) - 8.5).abs() < 1e-5);
            assert!((radii.fold(f32::INFINITY, f32::min) - 178_f32.sqrt() / 2.).abs() < 1e-5);
        }
        let mesh = icon_mesh(1);
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        assert!((positions.iter().map(|p| p[1]).fold(0., f32::max) / SCALE - 3.).abs() < 1e-5);
    }

    #[test]
    fn original_surface_decodes_as_a_warm_320_by_128_cry_image() {
        let image = surface_image();
        assert_eq!((image.width(), image.height()), (320, 128));
        let pixels = image.data.as_ref().unwrap();
        assert_eq!(pixels.len(), 320 * 128 * 4);
        let mut total = [0_u64; 3];
        for (raw, rgba) in SURFACE_CRY
            .as_chunks::<2>()
            .0
            .iter()
            .zip(pixels.as_chunks::<4>().0)
        {
            let expected = cry::color(raw[0], raw[1]).to_srgba();
            for (actual, channel) in
                rgba[..3]
                    .iter()
                    .zip([expected.red, expected.green, expected.blue])
            {
                assert!((f32::from(*actual) - channel * 255.).abs() < 0.001);
            }
            assert_eq!(rgba[3], 255);
            for channel in 0..3 {
                total[channel] += u64::from(rgba[channel]);
            }
        }
        assert!(
            total[0] > total[1] && total[1] > total[2],
            "surface should retain its red/gold palette"
        );
    }

    #[test]
    fn surface_stays_at_the_original_zero_height_and_scrolls_its_texture() {
        use bevy::mesh::VertexAttributeValues;
        let mesh = ground_mesh(0., 0);
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        assert!(positions.iter().all(|p| p[1] == 0.));
        assert!(positions.iter().any(|p| p[2] < -190.));
        let later = ground_mesh(10., 0);
        let VertexAttributeValues::Float32x2(before) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!()
        };
        let VertexAttributeValues::Float32x2(after) =
            later.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!()
        };
        assert!((after[0][1] - before[0][1] - 10. / 12.8).abs() < 1e-6);
    }

    #[test]
    fn rendering_owns_bonus_camera_visibility_and_original_gate_geometry() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Game>()
            .init_resource::<Level>()
            .add_systems(Startup, setup)
            .add_systems(Update, draw);
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                Transform::default(),
            ))
            .id();
        let mut session = Session::new(ZoneKind::GateFlight, 0, Some(1), 0);
        session.gates.push_back(Gate {
            index: 0,
            distance: 200 * 65536,
            age: 16,
        });
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(session);
        app.update();
        let Projection::Perspective(p) = app.world().get::<Projection>(camera).unwrap() else {
            panic!()
        };
        assert_eq!((p.fov, p.near), (1.2, 0.01));
        assert!(
            app.world_mut()
                .query::<(&WarpNode, &Visibility)>()
                .iter(app.world())
                .filter(|(node, visibility)| matches!(node, WarpNode::Gate(_))
                    && **visibility == Visibility::Visible)
                .count()
                > 3
        );
        let rotation = app
            .world_mut()
            .query::<(&WarpNode, &Transform)>()
            .iter(app.world())
            .find_map(|(node, pose)| matches!(node, WarpNode::Gate(0)).then_some(pose.rotation))
            .unwrap();
        assert!(
            (rotation * Vec3::X).distance(Vec3::new((TAU / 16.).cos(), -(TAU / 16.).sin(), 0.))
                < 1e-6
        );
        let assets = app.world().resource::<WarpAssets>();
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&assets.gates[0])
                .unwrap()
                .count_vertices(),
            48
        );
        assert!(
            app.world_mut()
                .query::<(&WarpNode, &Visibility)>()
                .iter(app.world())
                .any(|(node, visibility)| matches!(node, WarpNode::Gate(0))
                    && *visibility == Visibility::Visible)
        );
        app.world_mut().resource_mut::<Game>().status = Status::Entering;
        app.update();
        assert!(
            app.world_mut()
                .query::<(&WarpNode, &Visibility)>()
                .iter(app.world())
                .all(|(_, visibility)| *visibility == Visibility::Hidden)
        );
    }
}
