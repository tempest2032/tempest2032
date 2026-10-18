//! vicbon/versiondraw: glowing pyramid and pulsing EXCELLENT in CRY feedback.

use super::{Phase, ZoneKind};
use crate::{
    cry,
    game::{Game, Status},
    sines::sine,
};
use bevy::{camera::visibility::RenderLayers, prelude::*};
use cry::{FEEDBACK_HEIGHT as HEIGHT, FEEDBACK_WIDTH as WIDTH};

const ART: &[u8; 178 * 68 * 2] = include_bytes!("../../assets/messages/warp-victory.cry");
const WARP: &[u8; 238 * 18 * 2] = include_bytes!("../../assets/messages/warp-five.cry");
const TICKS_PER_FRAME: u32 = cry::TICKS_PER_FRAME as u32;

#[derive(Resource, Default, Clone)]
pub(crate) struct Feedback {
    source: Option<u32>,
    pixels: Vec<u16>,
    scratch: Vec<u16>,
    frame: u32,
}

impl Feedback {
    fn update(&mut self, source: u32, age: u32, warp_five: bool) {
        if self.source != Some(source) {
            self.source = Some(source);
            self.pixels = vec![0; WIDTH * HEIGHT];
            self.scratch = vec![0; WIDTH * HEIGHT];
            self.frame = 0;
        }
        let victory_frames = 401_u32.div_ceil(TICKS_PER_FRAME);
        let frames = if age < 401 {
            age / TICKS_PER_FRAME + 1
        } else {
            victory_frames + (age - 401) / TICKS_PER_FRAME + 1
        };
        while self.frame < frames {
            let elapsed = if self.frame < victory_frames {
                self.frame * TICKS_PER_FRAME
            } else {
                401 + (self.frame - victory_frames) * TICKS_PER_FRAME
            };
            let tick = source.wrapping_add(elapsed);
            if elapsed < 401 {
                // versiondraw's source center drifts independently on the two axes.
                cry::feedback_from(
                    &mut self.pixels,
                    &mut self.scratch,
                    UVec2::new(WIDTH as u32, HEIGHT as u32),
                    (((tick >> 2) & 127) as i32 - 63).abs() - 31,
                    Vec2::splat(494. / 256.),
                    if self.frame == 0 { -8 } else { -27 } * 65536,
                    Vec2::new(
                        1. + sine(self.frame as i32) as f32 / 32.,
                        4. + sine(tick as i32) as f32 / 16.,
                    ),
                );
                self.pyramid(self.frame + 1);
                self.excellent(tick);
            } else {
                if elapsed - TICKS_PER_FRAME < 401 && warp_five {
                    // fade merges the separate sharp text strip into the bitmap first.
                    self.stamp_warp();
                }
                cry::feedback(
                    &mut self.pixels,
                    &mut self.scratch,
                    UVec2::new(WIDTH as u32, HEIGHT as u32),
                    cry::settle(
                        cry::ofade_angle(source),
                        (elapsed - 401) / TICKS_PER_FRAME + 1,
                    ),
                    Vec2::splat(500. / 256.),
                    -8 * 65536,
                );
            }
            self.frame += 1;
        }
    }

    fn excellent(&mut self, tick: u32) {
        // vicbon's bobo -> yh -> rex, projected at Z=34. Shears are unprojected.
        let scale = (((0x6000 + sine(tick as i32) * 64) / (34 * 4)) << 9) as f32 / 65536.;
        let shear = Vec2::new(
            sine(tick as i32) as f32 / 2048.,
            sine(tick as i32 + 64) as f32 / 1024.,
        );
        let determinant = scale * scale - shear.x * shear.y;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let p = Vec2::new(x as f32 - 192., y as f32 - 120.);
                let p = Vec2::new(p.x * scale - p.y * shear.x, p.y * scale - p.x * shear.y)
                    / determinant
                    + Vec2::new(89., 34.);
                if p.x >= 0. && p.x < 178. && p.y >= 0. && p.y < 68. {
                    let i = (p.y as usize * 178 + p.x as usize) * 2;
                    let pixel = u16::from_be_bytes([ART[i], ART[i + 1]]);
                    if pixel != 0 {
                        self.pixels[y * WIDTH + x] = pixel;
                    }
                }
            }
        }
    }

    fn pyramid(&mut self, frame: u32) {
        let pulse = |phase| (((sine(phase) + 128) & 240) | ((sine(phase + 64) + 128) >> 4)) as u16;
        let a = pulse((frame * 4) as i32);
        let b = pulse(((frame * 0x60100) >> 16) as i32);
        // makepyr(64, 80, 192, 120): two base corners, top, and bright center.
        let left = (Vec2::new(160., 148.), a << 8 | 95);
        let right = (Vec2::new(224., 148.), a << 8 | 95);
        let top = (Vec2::new(192., 68.), b << 8 | 95);
        let center = (Vec2::new(192., 120.), 0x88ff);
        for triangle in [
            [left, top, center],
            [top, right, center],
            [right, left, center],
        ] {
            self.triangle(triangle);
        }
    }

    fn triangle(&mut self, vertices: [(Vec2, u16); 3]) {
        let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
        let [a, b, c] = vertices.map(|(p, _)| p);
        let area = cross(b - a, c - a);
        for y in 68..=148 {
            for x in 160..=224 {
                let p = Vec2::new(x as f32, y as f32);
                let u = cross(b - p, c - p) / area;
                let v = cross(c - p, a - p) / area;
                let w = 1. - u - v;
                if u >= 0. && v >= 0. && w >= 0. {
                    let component = |shift: u32, mask: u16| {
                        vertices
                            .iter()
                            .zip([u, v, w])
                            .map(|((_, pixel), weight)| f32::from((pixel >> shift) & mask) * weight)
                            .sum::<f32>() as u16
                    };
                    self.pixels[y * WIDTH + x] =
                        component(12, 15) << 12 | component(8, 15) << 8 | component(0, 255);
                }
            }
        }
    }

    fn stamp_warp(&mut self) {
        for (i, bytes) in WARP.as_chunks::<2>().0.iter().enumerate() {
            let pixel = u16::from_be_bytes(*bytes);
            if pixel != 0 {
                self.pixels[(202 + i / 238) * WIDTH + (WIDTH - 238) / 2 + i % 238] = pixel;
            }
        }
    }
}

/// The black backdrop (`None`) or the feedback bitmap with its image and material.
#[derive(Component)]
pub(crate) struct Canvas(Option<(Handle<Image>, Handle<StandardMaterial>)>);

pub(crate) fn setup(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.insert_resource(Feedback::default());
    let quad = meshes.add(Rectangle::new(1., 1.));
    let image = images.add(cry::feedback_image());
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image.clone()),
        unlit: true,
        ..default()
    });
    let backdrop = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        ..default()
    });
    for (canvas, material, depth) in [
        (Canvas(Some((image, material.clone()))), material, 2.),
        (Canvas(None), backdrop, 1.),
    ] {
        commands.spawn((
            canvas,
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0., 0., cry::BEHIND_HUD + depth),
            Visibility::Hidden,
            RenderLayers::layer(3),
        ));
    }
}

pub(crate) fn draw(
    game: Res<Game>,
    mut feedback: ResMut<Feedback>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut canvases: Query<(&Canvas, &mut Transform, &mut Visibility)>,
    mut preview: Local<cry::FeedbackPreview>,
) {
    let fresh = game.is_changed();
    let session = game
        .bonus
        .as_ref()
        .filter(|s| game.status == Status::Bonus && s.kind != ZoneKind::Tunnel);
    let active = session.and_then(|s| match s.phase {
        Phase::Victory(ticks) => Some((s, 401 - u32::from(ticks.min(401)))),
        Phase::Fade(ticks) => Some((s, 401 + 151 - u32::from(ticks.min(151)))),
        _ => None,
    });
    let viewport = crate::messages::geometry::viewport_width(&windows);
    if let Some((session, age)) = active {
        feedback.update(session.tick.saturating_sub(age), age, session.warp_five);
    } else {
        feedback.source = None;
    }
    let scales = cry::feedback_scales(viewport);
    for (canvas, mut pose, mut visible) in &mut canvases {
        visible.set_if_neq(if active.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        let scale = scales[usize::from(canvas.0.is_some())];
        if pose.scale != scale {
            pose.scale = scale;
        }
        let (Some((session, age)), Some((image, material))) = (active, &canvas.0) else {
            continue;
        };
        if fresh && let Some(image) = images.get_mut(image) {
            let phase = if age < 401 { age } else { age - 401 } % TICKS_PER_FRAME;
            let next = if phase == 0 {
                &feedback.pixels
            } else {
                preview.get_or_update(
                    (
                        u64::from(feedback.source.unwrap()) << 1 | u64::from(session.warp_five),
                        feedback.frame,
                    ),
                    || {
                        let mut next = feedback.clone();
                        next.update(
                            feedback.source.unwrap(),
                            age + TICKS_PER_FRAME - phase,
                            session.warp_five,
                        );
                        next.pixels
                    },
                )
            };
            cry::write_feedback(image, &feedback.pixels, next, phase);
            // Updating the texture replaces its GPU view; refresh the material binding too.
            let _ = materials.get_mut(material);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_accumulates_feedback_holds_between_frames_and_catches_up() {
        let mut feedback = Feedback::default();
        feedback.update(1000, 0, true);
        let first = feedback.pixels.clone();
        assert!(first.iter().any(|p| p & 255 > 0));
        feedback.update(1000, 3, true);
        assert_eq!(feedback.pixels, first);
        feedback.update(1000, 4, true);
        assert_ne!(feedback.pixels, first);
        for age in 5..=200 {
            feedback.update(1000, age, true);
        }
        let mut skipped = Feedback::default();
        skipped.update(1000, 200, true);
        assert_eq!(feedback.pixels, skipped.pixels);
        feedback.update(2000, 0, true);
        let mut fresh = Feedback::default();
        fresh.update(2000, 0, true);
        assert_eq!(feedback.pixels, fresh.pixels);
    }

    #[test]
    fn final_melt_fades_the_message_and_pyramid_to_black() {
        let mut feedback = Feedback::default();
        feedback.update(1000, 400, true);
        let bright = feedback.pixels.clone();
        feedback.update(1000, 401, true);
        assert_ne!(feedback.pixels, bright);
        let first_melt = feedback.pixels.clone();
        feedback.update(1000, 404, true);
        assert_eq!(feedback.pixels, first_melt);
        let mut late_game = Feedback::default();
        late_game.update(1000, 404, false);
        assert_ne!(feedback.pixels, late_game.pixels);
        feedback.update(1000, 551, true);
        assert!(feedback.pixels.iter().all(|p| p & 255 == 0));
    }

    #[test]
    fn both_gate_zones_replace_flight_with_feedback_then_clear_it_on_return() {
        let mut app = App::new();
        app.init_resource::<Game>()
            .init_resource::<crate::difficulty::EnemyClock>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(Startup, super::super::render::setup)
            .add_systems(Update, (super::super::render::draw, draw).chain());
        for kind in [ZoneKind::GateFlight, ZoneKind::Rings] {
            let mut session = super::super::Session::new(kind, 0, Some(1), 0);
            session.tick = 1000;
            session.phase = Phase::Victory(401);
            let mut game = app.world_mut().resource_mut::<Game>();
            game.status = Status::Bonus;
            game.bonus = Some(session);
            app.update();
            assert!(
                app.world_mut()
                    .query_filtered::<&Visibility, With<Canvas>>()
                    .iter(app.world())
                    .all(|v| *v == Visibility::Visible)
            );
            assert!(
                app.world_mut()
                    .query_filtered::<&Visibility, With<super::super::render::Crosshair>>()
                    .iter(app.world())
                    .all(|v| *v == Visibility::Hidden)
            );
            assert!(
                app.world_mut()
                    .query_filtered::<&Visibility, With<super::super::render::WarpNode>>()
                    .iter(app.world())
                    .all(|v| *v == Visibility::Hidden)
            );
            let mut game = app.world_mut().resource_mut::<Game>();
            let session = game.bonus.as_mut().unwrap();
            session.tick += 401;
            session.phase = Phase::Fade(151);
            app.update();
            assert_eq!(app.world().resource::<Feedback>().source, Some(1000));
            app.world_mut().resource_mut::<Game>().status = Status::Playing;
            app.update();
            assert!(
                app.world_mut()
                    .query_filtered::<&Visibility, With<Canvas>>()
                    .iter(app.world())
                    .all(|v| *v == Visibility::Hidden)
            );
            assert_eq!(app.world().resource::<Feedback>().source, None);
        }
    }
}
