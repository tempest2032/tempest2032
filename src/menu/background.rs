//! Menu backdrop drawn straight into the window: independent level-two stars on
//! every menu screen and, on Welcome, a rotating, native-resolution web preview.

use super::{FrontEnd, Screen};
use crate::{
    level::Level,
    messages::geometry::OverlayCamera,
    web::{BACK_RADIUS, FRONT_RADIUS, Web},
};
use bevy::{camera::visibility::RenderLayers, core_pipeline::tonemapping::Tonemapping, prelude::*};

#[derive(Component)]
struct PreviewStars(Vec<crate::stars::Star>);

#[derive(Component)]
struct PreviewCamera;

#[derive(Component)]
struct PreviewWeb;

const ROTATION_SPEED: f32 = std::f32::consts::TAU / 20.;

#[derive(Resource)]
struct PreviewAssets {
    mesh: Handle<Mesh>,
}

pub(super) fn add_systems(app: &mut App) {
    app.add_systems(
        Update,
        (update_web_preview, advance).chain().after(super::draw),
    );
}

pub(super) fn setup_preview(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = meshes.add(crate::mesh::colored_mesh([]));
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    // Sharing the window's targets avoids a separate full-screen MSAA image.
    commands.spawn((
        PreviewCamera,
        OverlayCamera,
        Camera3d::default(),
        Camera {
            order: -2,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 0.88,
            ..default()
        }),
        Transform::default(),
        RenderLayers::layer(5),
        bevy::render::view::NoIndirectDrawing,
        Tonemapping::None,
        Msaa::Sample4,
    ));
    commands.spawn((
        PreviewWeb,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::default(),
        RenderLayers::layer(5),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.spawn((
        PreviewStars(crate::stars::Pattern::Level.stars(2)),
        Mesh3d(meshes.add(crate::mesh::colored_mesh([]))),
        MeshMaterial3d(material),
        Transform::default(),
        RenderLayers::layer(5),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.insert_resource(PreviewAssets { mesh });
}

fn edge(a: Vec3, b: Vec3, width: f32, colors: [Color; 2]) -> Vec<[(Vec3, Color); 3]> {
    let direction = (b - a).normalize();
    let u = direction.any_orthonormal_vector() * width * 0.5;
    let v = direction.cross(u);
    let corners = [-u - v, u - v, u + v, -u + v];
    (0..4)
        .flat_map(|side| {
            let next = (side + 1) % 4;
            let quad = [
                (a + corners[side], colors[0]),
                (a + corners[next], colors[0]),
                (b + corners[next], colors[1]),
                (b + corners[side], colors[1]),
            ];
            [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
        })
        .collect()
}

fn web_mesh(level: &Level, width: f32) -> Mesh {
    let web = &level.web;
    let [r, g, b] = level.appearance.lines;
    let colors = [Color::srgb(r, g, b), Color::srgb(r * 0.4, g * 0.4, b * 0.4)];
    let ring = |radius, depth| {
        (0..web.vertices.len())
            .map(|index| web.vertex(index as f32, radius, depth))
            .collect::<Vec<_>>()
    };
    let front = ring(FRONT_RADIUS, 0.);
    let back = ring(BACK_RADIUS, -web.world_depth());
    let mut triangles = Vec::new();
    for (points, color) in [(&front, colors[0]), (&back, colors[1])] {
        for lane in 0..web.lanes() {
            let next = if web.closed {
                (lane + 1) % points.len()
            } else {
                lane + 1
            };
            triangles.extend(edge(points[lane], points[next], width, [color; 2]));
        }
    }
    for index in 0..front.len() {
        triangles.extend(edge(front[index], back[index], width * 0.8, colors));
    }
    crate::mesh::colored_mesh(triangles)
}

fn fitted_camera(web: &Web, aspect: f32, fov: f32) -> (Transform, f32, Vec3) {
    let front = (0..web.vertices.len())
        .map(|index| web.vertex(index as f32, FRONT_RADIUS, 0.))
        .collect::<Vec<_>>();
    let minimum = front.iter().copied().fold(Vec3::INFINITY, Vec3::min);
    let maximum = front.iter().copied().fold(Vec3::NEG_INFINITY, Vec3::max);
    let center = (minimum + maximum) * 0.5;
    let radius = front
        .iter()
        .map(|point| (point - center).truncate().length())
        .fold(0., f32::max);
    let tangent = (fov * 0.5).tan();
    // Keep the view parallel to the tunnel so its vanishing point matches the stars.
    // Move above flat webs rather than tilting the view and shifting that point.
    let offset = if maximum.y == minimum.y {
        radius * 0.22
    } else {
        0.
    };
    // Fit the entire rotational sweep, filling 90% of the shorter window axis.
    let distance = ((radius / aspect).max(radius + offset) / (tangent * 0.9)).max(1.);
    (
        Transform::from_translation(center + Vec3::new(0., offset, distance)),
        distance * tangent * 0.006,
        center,
    )
}

fn rotation_pose(center: Vec3, angle: f32) -> Transform {
    let rotation = Quat::from_rotation_z(angle);
    Transform::from_translation(center - rotation * center).with_rotation(rotation)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_web_preview(
    front: Res<FrontEnd>,
    time: Res<Time<Real>>,
    mut camera: Single<
        (&mut Camera, &mut Projection, &mut Transform),
        (With<PreviewCamera>, Without<PreviewWeb>),
    >,
    mut web: Single<(&mut Transform, &mut Visibility), With<PreviewWeb>>,
    assets: Res<PreviewAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last: Local<Option<(usize, UVec2)>>,
    mut center: Local<Vec3>,
    mut angle: Local<f32>,
) {
    let active = front.screen != Screen::Gameplay;
    if camera.0.is_active != active {
        camera.0.is_active = active;
    }
    let welcome = front.screen == Screen::Welcome;
    web.1.set_if_neq(if welcome {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if !welcome {
        return;
    }
    let selected = front.preview_level();
    let size = camera
        .0
        .physical_viewport_size()
        .unwrap_or(UVec2::new(640, 400))
        .max(UVec2::ONE);
    if *last != Some((selected, size))
        && let Some(level) = front.originals.get(selected.saturating_sub(1))
        && let Some(mesh) = meshes.get_mut(&assets.mesh)
        && let Projection::Perspective(projection) = &mut *camera.1
    {
        let aspect = size.x as f32 / size.y as f32;
        let (pose, width, pivot) = fitted_camera(&level.web, aspect, projection.fov);
        projection.aspect_ratio = aspect;
        *camera.2 = pose;
        *center = pivot;
        *mesh = web_mesh(level, width);
        *last = Some((selected, size));
    }
    *angle =
        (*angle + time.delta_secs().min(0.1) * ROTATION_SPEED).rem_euclid(std::f32::consts::TAU);
    *web.0 = rotation_pose(*center, *angle);
}

#[allow(clippy::type_complexity)]
fn advance(
    front: Res<FrontEnd>,
    time: Res<Time<Real>>,
    camera: Single<(&Transform, &Projection), With<PreviewCamera>>,
    mut stars: Single<(&mut PreviewStars, &Mesh3d, &mut Transform), Without<PreviewCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fraction: Local<f32>,
) {
    let (camera, Projection::Perspective(projection)) = *camera else {
        return;
    };
    if front.screen == Screen::Gameplay {
        return;
    }
    // Limit motion after suspension, without advancing the gameplay star field.
    *fraction += time.delta_secs().min(0.1) * 60. * crate::stars::STEP as f32;
    let step = *fraction as i32;
    *fraction -= step as f32;
    let (field, mesh, pose) = &mut *stars;
    for star in &mut field.0 {
        star.advance(step);
    }
    let (plane, half, pixel) = crate::stars::background_plane(camera, projection);
    if let Some(mesh) = meshes.get_mut(&mesh.0) {
        *mesh = crate::stars::star_mesh(
            field
                .0
                .iter()
                .map(|star| (star.projected(Vec2::ZERO) * pixel, star.color())),
            half,
            pixel,
        );
    }
    pose.set_if_neq(plane);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_wireframes_fit_closed_open_and_flat_webs_through_a_full_turn() {
        let levels = crate::test_support::levels();
        for level in &levels.0 {
            for aspect in [0.65, 1.2, 1.9] {
                let (camera, width, center) = fitted_camera(&level.web, aspect, 0.88);
                assert_eq!(camera.rotation * Vec3::NEG_Z, Vec3::NEG_Z);
                if level.web.horizontal_plane() {
                    assert!(camera.translation.y > center.y);
                } else {
                    assert!(camera.translation.truncate().distance(center.truncate()) < 1e-5);
                }
                let mesh = web_mesh(level, width);
                let bevy::mesh::VertexAttributeValues::Float32x3(vertices) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
                else {
                    panic!("missing positions")
                };
                let tangent = 0.44_f32.tan();
                for step in 0..16 {
                    let model = rotation_pose(center, step as f32 * std::f32::consts::TAU / 16.);
                    assert!(model.transform_point(center).distance(center) < 1e-5);
                    for &point in vertices {
                        let point = camera.rotation.inverse()
                            * (model.transform_point(Vec3::from_array(point)) - camera.translation);
                        assert!(point.z < -0.1, "web {} near plane", level.number);
                        let projected =
                            point.truncate() / -point.z / Vec2::new(tangent * aspect, tangent);
                        assert!(
                            projected.is_finite() && projected.abs().max_element() < 0.92,
                            "web {} at aspect {aspect}, turn {step}: {projected}",
                            level.number
                        );
                    }
                }
                let bevy::mesh::VertexAttributeValues::Float32x3(normals) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
                else {
                    panic!("missing normals")
                };
                assert!(normals.iter().flatten().all(|value| value.is_finite()));
                assert!(
                    vertices
                        .iter()
                        .any(|point| point[2] <= -level.web.world_depth())
                );
            }
        }
    }

    #[test]
    fn wireframes_use_the_gameplay_palette_even_when_the_outline_repeats() {
        let levels = crate::test_support::levels();
        let mut palettes = Vec::new();
        for number in [1, 17, 33, 49, 65, 81, 97] {
            let level = &levels.0[number - 1];
            let mesh = web_mesh(level, 0.04);
            let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
            else {
                panic!("missing colors")
            };
            let expected = Color::srgb_from_array(level.appearance.lines)
                .to_linear()
                .to_f32_array();
            assert!(
                colors.contains(&expected),
                "web {number} has no original front-rim color"
            );
            assert!(colors.iter().all(|color| color[3] == 1.));
            assert!(
                !palettes.contains(&expected),
                "web {number} lost its distinct palette"
            );
            palettes.push(expected);
        }
        assert_eq!(levels.0[0].web.vertices, levels.0[32].web.vertices);
    }

    #[test]
    fn preview_camera_is_isolated_from_gameplay_and_selection_reuses_assets() {
        let levels = crate::test_support::levels();
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Time<Real>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(Level::default())
            .insert_resource(FrontEnd::new(Screen::Welcome, false, levels.0))
            .add_systems(
                Startup,
                |mut commands: Commands,
                 mut meshes: ResMut<Assets<Mesh>>,
                 mut materials: ResMut<Assets<StandardMaterial>>| {
                    setup_preview(&mut commands, &mut meshes, &mut materials);
                },
            )
            .add_systems(
                Update,
                (update_web_preview, crate::camera::follow_glider).chain(),
            );
        app.world_mut().spawn((
            crate::glider::Glider {
                progress: 0,
                velocity: 0,
            },
            Transform::default(),
        ));
        app.update();
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<PreviewCamera>>()
            .single(app.world())
            .unwrap();
        assert!(app.world().get::<OverlayCamera>(entity).is_some());
        let original = *app.world().get::<Transform>(entity).unwrap();
        for number in [17, 33, 65, 1] {
            app.world_mut().resource_mut::<FrontEnd>().start_level = number;
            app.update();
        }
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), original);
        let model = app
            .world_mut()
            .query_filtered::<Entity, With<PreviewWeb>>()
            .single(app.world())
            .unwrap();
        for _ in 0..60 {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(std::time::Duration::from_secs_f32(1. / 60.));
            app.update();
        }
        let spun = *app.world().get::<Transform>(model).unwrap();
        let expected = Quat::from_rotation_z(ROTATION_SPEED) * Vec3::X;
        assert!((spun.rotation * Vec3::X).distance(expected) < 1e-5);
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), original);
        app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
        app.update();
        assert!(!app.world().get::<Camera>(entity).unwrap().is_active);
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), original);
        assert_eq!(*app.world().get::<Transform>(model).unwrap(), spun);
        // The web and the star field.
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        // The preview draws into the window rather than an offscreen image.
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
    }
}
