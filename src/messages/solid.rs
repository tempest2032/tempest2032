//! Native-resolution 3D lettering overlay; the accepted approach effects stay 2D.

use super::{
    Id, Messages,
    geometry::{self, ADVANCE, Glyphs, OverlayCamera, Style, Triangle},
};
use bevy::{
    camera::{ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
};

#[derive(Resource)]
struct Graphics {
    banner: Handle<Mesh>,
    pause: Handle<Mesh>,
    last: Option<FrameKey>,
}

#[derive(PartialEq)]
struct FrameKey {
    epoch: u32,
    clock: u32,
    banner: Option<(Id, u32)>,
    viewport: f32,
    paused: bool,
    pause_choice: usize,
}

pub(super) fn add_systems(app: &mut App) {
    app.init_resource::<Glyphs>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            draw.after(super::render::draw)
                .after(crate::game::load_level)
                .after(crate::hud::update_hud),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        OverlayCamera,
        // Later window cameras, such as the menu lettering, must not take over the UI.
        IsDefaultUiCamera,
        // A few small, changing overlay meshes use direct draws.
        bevy::render::view::NoIndirectDrawing,
        Camera3d::default(),
        Camera {
            order: 3,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 240.,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0., 0., 100.),
        RenderLayers::layer(3),
        Tonemapping::None,
        Msaa::Sample4,
        // Smooth the combined scene once, after the web, actors, and 3D HUD.
        bevy::anti_alias::fxaa::Fxaa::default(),
    ));
    let opaque = materials.add(StandardMaterial {
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let mut spawn = |material: Handle<StandardMaterial>| {
        let mesh = meshes.add(geometry::mesh(Vec::new()));
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            RenderLayers::layer(3),
            bevy::camera::visibility::NoFrustumCulling,
        ));
        mesh
    };
    let banner = spawn(opaque.clone());
    let pause = spawn(opaque);
    commands.insert_resource(Graphics {
        banner,
        pause,
        last: None,
    });
}

fn draw(
    messages: Res<Messages>,
    glyphs: Res<Glyphs>,
    mut graphics: ResMut<Graphics>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time<Virtual>>,
    menu: Res<crate::pause::Menu>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let viewport = crate::messages::geometry::viewport_width(&windows);
    let key = FrameKey {
        epoch: messages.epoch,
        clock: if messages.banner.is_some() {
            messages.clock
        } else {
            0
        },
        banner: messages.banner.map(|a| (a.event.id, a.age)),
        viewport,
        paused: time.is_paused(),
        pause_choice: menu.selected,
    };
    if graphics.last.as_ref() == Some(&key) {
        return;
    }
    let mut banner = Vec::new();
    if let Some(active) = messages.banner {
        let text = active.event.id.text();
        let width = text.lines().map(|l| l.len()).max().unwrap_or(0) as f32 * ADVANCE;
        let hint = matches!(active.event.id, Id::GreenTrack | Id::WarpFive);
        let small = if active.event.id == Id::GreenTrack {
            0.5
        } else {
            1.
        };
        let fit = ((viewport - 24.) / (width.max(1.) * small)).min(1.) * small;
        let scale = if hint { Vec2::ONE } else { active.scale() };
        let shear = if hint {
            0.
        } else {
            (((messages.clock.wrapping_mul(2) & 255) as i32 - 127).abs() - 63) as f32 / 64.
        };
        let anchor = match active.event.id {
            Id::GreenTrack => -60.,
            Id::WarpFive => -91.,
            _ => 40.,
        };
        for (line, text) in text.lines().enumerate() {
            let left = -(text.len() as f32) * ADVANCE / 2.;
            for (index, character) in text.chars().enumerate() {
                let glyph = glyphs.get(character, Style::Chrome);
                let origin = Vec2::new(left + 7.5 + index as f32 * ADVANCE, line as f32 * 20.);
                let transform = |p: Vec3, center: Vec2| {
                    let p = p - Vec3::new(7.5, 9., 0.);
                    let center = center - Vec2::new(7.5, 9.);
                    let xy = center * scale + (p.truncate() - center) * scale.min(Vec2::ONE);
                    let xy = Vec2::new(xy.x + xy.y * shear, xy.y - xy.x * shear * 0.5);
                    Vec3::new(
                        (origin.x + xy.x) * fit,
                        anchor - (origin.y + xy.y) * fit,
                        10. + p.z * fit,
                    )
                };
                if scale == Vec2::ONE {
                    banner.extend(
                        glyph
                            .triangles
                            .iter()
                            .map(|t| t.map(|(p, c)| (transform(p, Vec2::new(7.5, 9.)), c))),
                    );
                } else {
                    banner.extend(
                        glyph
                            .fragments()
                            .iter()
                            .map(|f| f.triangle.map(|(p, c)| (transform(p, f.center), c))),
                    );
                }
            }
        }
    }
    let mut paused = paused_geometry(&glyphs, viewport, time.is_paused());
    if time.is_paused() {
        let fit = ((viewport - 24.) / 174.).clamp(0.01, 1.);
        for (row, text) in crate::pause::CHOICES.iter().enumerate() {
            let selected = row == menu.selected;
            let (center, scale) = crate::pause::choice_layout(row, viewport);
            let left = -(text.len() as f32 * ADVANCE - 2.) * scale / 2.;
            glyphs.place(
                &mut paused,
                text,
                if selected { Style::Gold } else { Style::Chrome },
                Vec3::new(left, center + 9. * scale, 60.),
                Vec3::splat(scale),
                ADVANCE * scale,
            );
            if selected {
                let color = crate::cry::color(0x8f, 255);
                for side in [-1., 1.] {
                    let x = side * (text.len() as f32 * ADVANCE * scale / 2. + 10. * fit);
                    let y = center;
                    paused.push([
                        (Vec3::new(x - side * 4. * fit, y, 61.), color),
                        (Vec3::new(x + side * 2. * fit, y + 4. * fit, 61.), color),
                        (Vec3::new(x + side * 2. * fit, y - 4. * fit, 61.), color),
                    ]);
                }
            }
        }
    }
    for (handle, triangles, was_visible) in [
        (
            &graphics.banner,
            banner,
            graphics.last.as_ref().is_some_and(|k| k.banner.is_some()),
        ),
        (
            &graphics.pause,
            paused,
            graphics.last.as_ref().is_some_and(|k| k.paused),
        ),
    ] {
        if was_visible || !triangles.is_empty() {
            *meshes.get_mut(handle).unwrap() = geometry::mesh(triangles);
        }
    }
    graphics.last = Some(key);
}

fn paused_geometry(glyphs: &Glyphs, viewport: f32, paused: bool) -> Vec<Triangle> {
    if !paused {
        return Vec::new();
    }
    let fit = ((viewport - 24.) / 174.).clamp(0.01, 1.);
    let mut triangles = Vec::new();
    // bfont's 27x31 PAUSED glyphs on a 29-unit advance.
    glyphs.place(
        &mut triangles,
        "PAUSED",
        Style::Gold,
        Vec3::new(-87. * fit, 55., 60.),
        Vec3::new(27. / 15. * fit, 31. / 18. * fit, fit),
        29. * fit,
    );
    triangles
}

#[cfg(test)]
#[path = "../tests/solid.rs"]
mod tests;
