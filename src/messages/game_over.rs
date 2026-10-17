//! gofeed's full-color bitmap, sine pulse, and fading scale-and-rotate feedback.

use super::Messages;
use bevy::{camera::visibility::RenderLayers, prelude::*};

const WIDTH: usize = 384;
const HEIGHT: usize = 240;
const ART_WIDTH: usize = 145;
const ART_HEIGHT: usize = 68;
const ART: &[u8] = include_bytes!("../../assets/messages/game-over.cry");
// gofeed advances per completed GPU frame, not per gameplay/VBlank tick.
// Retain 15 Hz feedback checkpoints and interpolate their presentation at 60 Hz.
const TICKS_PER_FRAME: u32 = 4;

#[derive(Resource, Clone)]
struct Feedback {
    pixels: Vec<u16>,
    scratch: Vec<u16>,
    age: Option<u32>,
}

impl Default for Feedback {
    fn default() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
            scratch: vec![0; WIDTH * HEIGHT],
            age: None,
        }
    }
}

impl Feedback {
    fn update(&mut self, age: u32) {
        if self.age == Some(age) {
            return;
        }
        let start = if let Some(previous) = self.age.filter(|previous| *previous < age) {
            previous + 1
        } else {
            self.pixels.fill(0);
            self.scratch.fill(0);
            0
        };
        for tick in start..=age {
            crate::cry::feedback(
                &mut self.pixels,
                &mut self.scratch,
                UVec2::new(WIDTH as u32, HEIGHT as u32),
                ((tick / 8) & 15) as i32 - 7,
                Vec2::splat(500. / 256.),
                -8 * 65536,
            );
            self.stamp(tick);
        }
        self.age = Some(age);
    }

    fn stamp(&mut self, tick: u32) {
        let scale = [15, 17].map(|speed| {
            let scale =
                0x8000 + (crate::sines::sine((tick.wrapping_mul(speed) & 255) as i32) + 128) * 64;
            // camel.gas rex projects the 16:16 scale at dstz = 96.
            ((scale / (96 * 4)) << 9) as f32 / 65536.
        });
        let origin = Vec2::new(WIDTH as f32, HEIGHT as f32) / 2.
            - Vec2::new(ART_WIDTH as f32 * scale[0], ART_HEIGHT as f32 * scale[1]) / 2.;
        for y in 0..ART_HEIGHT {
            for x in 0..ART_WIDTH {
                let index = (y * ART_WIDTH + x) * 2;
                let pixel = u16::from_be_bytes([ART[index], ART[index + 1]]);
                if pixel == 0 {
                    continue;
                }
                let dx = (origin.x + x as f32 * scale[0]).floor() as usize;
                let dy = (origin.y + y as f32 * scale[1]).floor() as usize;
                self.pixels[dy * WIDTH + dx] = pixel;
            }
        }
    }
}

#[derive(Component)]
enum Canvas {
    Background,
    Feedback,
}

#[derive(Resource)]
struct ImageHandle(Handle<Image>, Handle<StandardMaterial>);

pub(super) fn add_systems(app: &mut App) {
    app.init_resource::<Feedback>()
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, advance.after(super::advance))
        .add_systems(Update, draw.after(super::render::draw));
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = images.add(crate::cry::feedback_image());
    let quad = meshes.add(Rectangle::new(1., 1.));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image.clone()),
        unlit: true,
        ..default()
    });
    for (canvas, material, depth) in [
        (
            Canvas::Background,
            materials.add(StandardMaterial {
                base_color: Color::BLACK,
                unlit: true,
                ..default()
            }),
            0.,
        ),
        (Canvas::Feedback, material.clone(), 1.),
    ] {
        commands.spawn((
            canvas,
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0., 0., crate::cry::BEHIND_HUD + depth),
            Visibility::Hidden,
            RenderLayers::layer(3),
        ));
    }
    commands.insert_resource(ImageHandle(image, material));
}

fn advance(messages: Res<Messages>, mut feedback: ResMut<Feedback>) {
    let age = messages
        .game_over
        .map(|active| active.age / TICKS_PER_FRAME);
    if feedback.age == age {
        return;
    }
    if let Some(age) = age {
        feedback.update(age);
    } else {
        *feedback = Feedback::default();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    messages: Res<Messages>,
    feedback: Res<Feedback>,
    handle: Res<ImageHandle>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut canvases: Query<(&Canvas, &mut Transform, &mut Visibility)>,
    mut preview: Local<crate::cry::FeedbackPreview>,
) {
    let age = messages.game_over.map(|active| active.age);
    let viewport = crate::messages::geometry::viewport_width(&windows);
    let scales = crate::cry::feedback_scales(viewport);
    for (canvas, mut transform, mut visible) in &mut canvases {
        visible.set_if_neq(if age.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        let scale = scales[usize::from(matches!(canvas, Canvas::Feedback))];
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
    let Some(age) = age else { return };
    let frame = age / TICKS_PER_FRAME;
    if feedback.age != Some(frame) || !messages.is_changed() {
        return;
    }
    if let Some(image) = images.get_mut(&handle.0) {
        let phase = age % TICKS_PER_FRAME;
        let next = if phase == 0 {
            &feedback.pixels
        } else {
            preview.get_or_update((u64::from(messages.epoch), frame), || {
                let mut next = feedback.clone();
                next.update(frame + 1);
                next.pixels
            })
        };
        crate::cry::write_feedback(image, &feedback.pixels, next, phase);
        // Updating the texture replaces its GPU view; refresh the material binding too.
        let _ = materials.get_mut(&handle.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_game_over_before_another_fixed_tick_hides_the_cached_canvas() {
        let mut app = App::new();
        app.init_resource::<Messages>()
            .init_resource::<Feedback>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, advance)
            .add_systems(Update, draw);
        app.update();
        app.world_mut().resource_mut::<Messages>().game_over = Some(super::super::Active {
            event: super::super::Id::GameOver.into(),
            age: 61,
        });
        app.world_mut().run_schedule(FixedUpdate);
        app.update();
        assert!(
            app.world_mut()
                .query::<&Visibility>()
                .iter(app.world())
                .all(|v| *v == Visibility::Visible)
        );
        // Initials entry clears messages while gameplay fixed updates are stopped.
        app.world_mut().resource_mut::<Messages>().game_over = None;
        app.update();
        assert!(app.world().resource::<Feedback>().age.is_some());
        assert!(
            app.world_mut()
                .query::<&Visibility>()
                .iter(app.world())
                .all(|v| *v == Visibility::Hidden)
        );
    }

    #[test]
    fn presentation_changes_each_tick_and_preserves_feedback_checkpoints() {
        let mut app = App::new();
        app.init_resource::<Messages>()
            .init_resource::<Feedback>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, setup)
            .add_systems(Update, (advance, draw).chain());
        let mut previous = Vec::new();
        for tick in 0..=12 {
            app.world_mut().resource_mut::<Messages>().game_over = Some(super::super::Active {
                event: super::super::Id::GameOver.into(),
                age: tick,
            });
            app.update();
            let handle = app.world().resource::<ImageHandle>().0.clone();
            let image = app
                .world()
                .resource::<Assets<Image>>()
                .get(&handle)
                .unwrap();
            assert_ne!(*image.data.as_ref().unwrap(), previous);
            let mut expected = image.clone();
            let mut reference = Feedback::default();
            reference.update(tick / 4);
            let mut next = reference.clone();
            next.update(tick / 4 + 1);
            crate::cry::write_feedback(&mut expected, &reference.pixels, &next.pixels, tick % 4);
            assert_eq!(image.data, expected.data);
            previous.clone_from(image.data.as_ref().unwrap());
            app.update();
            let image = app
                .world()
                .resource::<Assets<Image>>()
                .get(&handle)
                .unwrap();
            assert_eq!(*image.data.as_ref().unwrap(), previous);
        }
    }

    #[test]
    fn feedback_runs_at_fifteen_hz_and_holds_on_intermediate_ticks() {
        let mut app = App::new();
        app.init_resource::<Messages>()
            .init_resource::<Feedback>()
            .add_systems(FixedUpdate, advance);
        for tick in 0..=60 {
            let before = app.world().resource::<Feedback>().pixels.clone();
            app.world_mut().resource_mut::<Messages>().game_over = Some(super::super::Active {
                event: super::super::Id::GameOver.into(),
                age: tick,
            });
            app.world_mut().run_schedule(FixedUpdate);
            let feedback = app.world().resource::<Feedback>();
            assert_eq!(feedback.age, Some(tick / 4));
            if tick % 4 != 0 {
                assert_eq!(feedback.pixels, before);
            }
        }
        let mut reference = Feedback::default();
        reference.update(15);
        assert_eq!(app.world().resource::<Feedback>().pixels, reference.pixels);
    }

    #[test]
    fn skipped_render_frames_and_frozen_previews_keep_the_same_feedback() {
        let mut every_tick = Feedback::default();
        for age in 0..=90 {
            every_tick.update(age);
        }
        let mut skipped = Feedback::default();
        skipped.update(90);
        assert_eq!(every_tick.pixels, skipped.pixels);
        skipped.update(90);
        assert_eq!(every_tick.pixels, skipped.pixels);
        skipped.update(0);
        let mut fresh = Feedback::default();
        fresh.update(0);
        assert_eq!(fresh.pixels, skipped.pixels);
    }

    #[test]
    fn original_pixels_keep_their_shading_and_trails_spread_beyond_the_stamp() {
        assert_eq!(ART.len(), ART_WIDTH * ART_HEIGHT * 2);
        let mut feedback = Feedback::default();
        feedback.update(0);
        let bright = feedback
            .pixels
            .iter()
            .filter(|pixel| **pixel & 255 > 80)
            .count();
        assert!(bright > 1000 && bright < 10_000);
        let colors: std::collections::HashSet<_> = feedback
            .pixels
            .iter()
            .copied()
            .filter(|pixel| *pixel != 0)
            .collect();
        let source: std::collections::HashSet<_> = ART
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pixel| u16::from_be_bytes(*pixel))
            .collect();
        assert!(colors.len() > 20 && colors.is_subset(&source));
        feedback.update(60);
        assert!(
            feedback
                .pixels
                .iter()
                .filter(|pixel| **pixel & 255 > 80)
                .count()
                > bright
        );
        assert!(
            feedback.pixels[..80 * WIDTH]
                .iter()
                .any(|pixel| *pixel & 255 > 80)
        );
    }
}
