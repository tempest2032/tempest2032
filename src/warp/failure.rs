//! failfade/ffade reprocess the last scene on gate exits and when a tunnel collapses or completes.

use super::{Phase, ZoneKind};
use crate::{
    cry,
    game::{Game, Status},
};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use cry::{FEEDBACK_HEIGHT as HEIGHT, FEEDBACK_WIDTH as WIDTH, TICKS_PER_FRAME};
use std::sync::OnceLock;

fn palette() -> &'static [u16] {
    static PALETTE: OnceLock<Vec<u16>> = OnceLock::new();
    PALETTE.get_or_init(|| {
        (0..32768)
            .map(|i| encode([10, 5, 0].map(|shift| (((i >> shift) & 31) * 255 / 31) as u8)))
            .collect()
    })
}

#[derive(Resource, Default, Clone)]
pub(crate) struct Feedback {
    source: Option<u32>,
    generation: u64,
    pixels: Vec<u16>,
    scratch: Vec<u16>,
    age: u16,
}

impl Feedback {
    fn capture(&mut self, image: &Image) {
        // Sample the RGBA target in place rather than converting a full-resolution copy.
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb,
            "failure frame must use the capture target's format"
        );
        let data = image.data.as_ref().expect("failure frame must be readable");
        let (width, height) = (image.width() as usize, image.height() as usize);
        let palette = palette();
        self.pixels = (0..WIDTH * HEIGHT)
            .map(|i| {
                let x = (i % WIDTH) * width / WIDTH;
                let y = (i / WIDTH) * height / HEIGHT;
                palette[data[(y * width + x) * 4..][..3]
                    .iter()
                    .fold(0, |i, &c| (i << 5) | usize::from(c >> 3))]
            })
            .collect();
        self.scratch = vec![0; WIDTH * HEIGHT];
    }

    fn advance(&mut self, age: u16, angle: i32, settle: bool) {
        if self.pixels.is_empty() {
            return;
        }
        while self.age < 1 + age / TICKS_PER_FRAME {
            let angle = if settle {
                cry::settle(angle, u32::from(self.age + 1))
            } else {
                angle
            };
            cry::feedback(
                &mut self.pixels,
                &mut self.scratch,
                UVec2::new(WIDTH as u32, HEIGHT as u32),
                angle,
                Vec2::splat(500. / 256.),
                -8 * 65536,
            );
            self.age += 1;
        }
    }
}

// Find the nearest CRY hue and intensity for each five-bit RGB lookup entry.
fn encode(rgb: [u8; 3]) -> u16 {
    let rgb = rgb.map(i32::from);
    (0..256)
        .map(|hue| {
            let channels = [&cry::RED, &cry::GREEN, &cry::BLUE]
                .map(|table| i32::from(table[hue >> 4][hue & 15]));
            let intensity = (rgb.iter().zip(channels).map(|(a, b)| a * b).sum::<i32>() * 256
                / channels.iter().map(|c| c * c).sum::<i32>().max(1))
            .clamp(0, 255);
            let error = rgb
                .iter()
                .zip(channels)
                .map(|(a, b)| (a - b * intensity / 256).pow(2))
                .sum::<i32>();
            (error, (hue as u16) << 8 | intensity as u16)
        })
        .min_by_key(|(error, _)| *error)
        .unwrap()
        .1
}

#[derive(Component)]
pub(crate) struct Canvas;

pub(crate) fn setup(commands: &mut Commands, images: &mut Assets<Image>) {
    // Build the RGB lookup before gameplay, rather than searching 256 hues per pixel on failure.
    palette();
    commands.insert_resource(Feedback::default());
    commands.spawn((
        Canvas,
        ImageNode::new(images.add(cry::feedback_image())),
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(30),
    ));
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    mut commands: Commands,
    game: Res<Game>,
    mut feedback: ResMut<Feedback>,
    mut images: ResMut<Assets<Image>>,
    mut canvas: Single<(&ImageNode, &mut Visibility), With<Canvas>>,
    cameras: Query<
        (&Camera, &Transform, &Projection, &RenderLayers, &Msaa),
        With<crate::camera::WebCamera>,
    >,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
    mut preview: Local<cry::FeedbackPreview>,
) {
    // Ticks change `Game`, and the screenshot observer changes `Feedback`.
    let fresh = game.is_changed() || feedback.is_changed();
    let failure = game
        .bonus
        .as_ref()
        .filter(|_| game.status == Status::Bonus)
        .and_then(|session| match (session.kind, session.phase) {
            (ZoneKind::GateFlight, Phase::Failure(ticks) | Phase::Success(ticks)) => {
                let age = 151 - ticks.min(151);
                Some((session.tick.saturating_sub(u32::from(age)), age, false))
            }
            (ZoneKind::Tunnel, Phase::Failure(ticks) | Phase::Fade(ticks)) if ticks <= 151 => {
                let age = 151 - ticks;
                Some((session.tick.saturating_sub(u32::from(age)), age, true))
            }
            _ => None,
        });
    let source = failure.map(|(source, _, _)| source);
    if source != feedback.source {
        feedback.source = source;
        feedback.generation = feedback.generation.wrapping_add(1);
        feedback.pixels.clear();
        feedback.age = 0;
        if source.is_some() {
            let generation = feedback.generation;
            // Render only the scene, without UI, so the crosshair never enters the feedback bitmap.
            let size = window.map_or(UVec2::new(WIDTH as u32, HEIGHT as u32), |w| {
                w.resolution.physical_size()
            });
            let mut image =
                Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
            image.texture_descriptor.usage |=
                bevy::render::render_resource::TextureUsages::COPY_SRC;
            // A render target needs no zeroed, window-sized copy in main memory.
            image.data = None;
            let target = images.add(image);
            let snapshots: Vec<_> = cameras
                .iter()
                .map(|(camera, pose, projection, layers, msaa)| {
                    commands
                        .spawn((
                            Camera3d::default(),
                            crate::messages::geometry::OverlayCamera,
                            Camera {
                                order: camera.order - 2,
                                clear_color: camera.clear_color,
                                ..default()
                            },
                            RenderTarget::Image(target.clone().into()),
                            *pose,
                            projection.clone(),
                            layers.clone(),
                            *msaa,
                            Tonemapping::None,
                        ))
                        .id()
                })
                .collect();
            commands.spawn(Screenshot::image(target)).observe(
                move |event: On<ScreenshotCaptured>,
                      mut feedback: ResMut<Feedback>,
                      mut commands: Commands| {
                    for &entity in &snapshots {
                        commands.entity(entity).despawn();
                    }
                    if feedback.source.is_some() && feedback.generation == generation {
                        feedback.capture(&event.image);
                    }
                },
            );
        }
    }
    let Some((source, age, settle)) = failure else {
        canvas.1.set_if_neq(Visibility::Hidden);
        return;
    };
    let angle = if settle {
        cry::ofade_angle(source)
    } else {
        i32::from(
            game.bonus
                .as_ref()
                .is_some_and(|s| matches!(s.phase, Phase::Failure(_))),
        )
    };
    feedback.advance(age, angle, settle);
    if feedback.pixels.is_empty() {
        canvas.1.set_if_neq(Visibility::Hidden);
        return;
    }
    canvas.1.set_if_neq(Visibility::Visible);
    if fresh && let Some(image) = images.get_mut(&canvas.0.image) {
        let phase = age % TICKS_PER_FRAME;
        let next = if phase == 0 {
            &feedback.pixels
        } else {
            preview.get_or_update((feedback.generation, u32::from(feedback.age)), || {
                let mut next = feedback.clone();
                next.advance(age + TICKS_PER_FRAME - phase, angle, settle);
                next.pixels
            })
        };
        cry::write_feedback(image, &feedback.pixels, next, u32::from(phase));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Image {
        let mut image = cry::feedback_image();
        for (i, pixel) in image
            .data
            .as_mut()
            .unwrap()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            if i % WIDTH % 32 < 4 || i / WIDTH % 32 < 4 {
                *pixel = [240, 80, 20, 255];
            }
        }
        image
    }

    #[test]
    fn bitmap_feedback_accumulates_distortion_and_fade_at_fifteen_hz() {
        let mut feedback = Feedback::default();
        feedback.capture(&frame());
        let original = feedback.pixels.clone();
        feedback.advance(0, 1, false);
        assert_ne!(original, feedback.pixels);
        let first = feedback.pixels.clone();
        feedback.advance(3, 1, false);
        assert_eq!(first, feedback.pixels);
        feedback.advance(4, 1, false);
        let second = feedback.pixels.clone();
        feedback.advance(7, 1, false);
        assert_eq!(second, feedback.pixels);
        for tick in 8..=80 {
            feedback.advance(tick, 1, false);
        }
        let mut skipped = Feedback::default();
        skipped.capture(&frame());
        skipped.advance(80, 1, false);
        assert_eq!(feedback.pixels, skipped.pixels);
        assert!(
            feedback
                .pixels
                .iter()
                .map(|p| u64::from(p & 255))
                .sum::<u64>()
                < original.iter().map(|p| u64::from(p & 255)).sum::<u64>()
        );
        feedback.advance(150, 1, false);
        assert!(feedback.pixels.iter().all(|p| p & 255 == 0));
    }

    #[test]
    fn final_gate_fades_the_scene_without_the_missed_gate_rotation() {
        let mut feedback = Feedback::default();
        feedback.capture(&frame());
        let mut expected = feedback.pixels.clone();
        let mut scratch = feedback.scratch.clone();
        cry::feedback(
            &mut expected,
            &mut scratch,
            UVec2::new(WIDTH as u32, HEIGHT as u32),
            0,
            Vec2::splat(500. / 256.),
            -8 * 65536,
        );
        feedback.advance(0, 0, false);
        assert_eq!(feedback.pixels, expected);
    }

    #[test]
    fn tunnel_success_waits_for_the_final_fade_before_capturing() {
        let mut app = App::new();
        app.init_resource::<Game>()
            .init_resource::<crate::difficulty::EnemyClock>()
            .init_resource::<Assets<Image>>()
            .add_systems(
                Startup,
                |mut commands: Commands, mut images: ResMut<Assets<Image>>| {
                    setup(&mut commands, &mut images)
                },
            )
            .add_systems(Update, draw);
        let mut session = super::super::Session::new(ZoneKind::Tunnel, 0, Some(1), 0);
        session.tick = 1000;
        session.phase = Phase::Victory(1);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(session);
        app.update();
        assert!(app.world().resource::<Feedback>().source.is_none());
        let mut game = app.world_mut().resource_mut::<Game>();
        let session = game.bonus.as_mut().unwrap();
        session.tick += 1;
        session.phase = Phase::Fade(151);
        app.update();
        assert_eq!(app.world().resource::<Feedback>().source, Some(1001));
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn tunnel_failure_captures_after_collapse_and_melts_before_returning() {
        let mut app = App::new();
        app.init_resource::<Game>()
            .init_resource::<crate::difficulty::EnemyClock>()
            .init_resource::<Assets<Image>>()
            .add_systems(
                Startup,
                |mut commands: Commands, mut images: ResMut<Assets<Image>>| {
                    setup(&mut commands, &mut images)
                },
            )
            .add_systems(Update, draw);
        let mut session = super::super::Session::new(ZoneKind::Tunnel, 0, Some(1), 0);
        session.tick = 200;
        session.phase = Phase::Failure(183);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(session);
        app.update();
        assert!(app.world().resource::<Feedback>().source.is_none());
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            0
        );

        let mut game = app.world_mut().resource_mut::<Game>();
        let session = game.bonus.as_mut().unwrap();
        session.tick = 232;
        session.phase = Phase::Failure(151);
        app.update();
        let request = app
            .world_mut()
            .query_filtered::<Entity, With<Screenshot>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(ScreenshotCaptured {
            entity: request,
            image: frame(),
        });
        app.update();
        assert!(
            app.world()
                .resource::<Feedback>()
                .pixels
                .iter()
                .any(|p| p & 255 > 0)
        );
        let canvas = app
            .world_mut()
            .query_filtered::<Entity, With<Canvas>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Visibility>(canvas),
            Some(&Visibility::Visible)
        );

        let mut game = app.world_mut().resource_mut::<Game>();
        let session = game.bonus.as_mut().unwrap();
        session.tick = 382;
        session.phase = Phase::Failure(1);
        app.update();
        let feedback = app.world().resource::<Feedback>();
        assert_eq!(feedback.source, Some(232));
        assert!(feedback.pixels.iter().all(|p| p & 255 == 0));
    }

    #[test]
    fn failure_captures_once_displays_feedback_and_discards_late_captures_on_return() {
        let mut app = App::new();
        app.init_resource::<Game>()
            .init_resource::<crate::difficulty::EnemyClock>()
            .init_resource::<Assets<Image>>()
            .add_systems(
                Startup,
                |mut commands: Commands, mut images: ResMut<Assets<Image>>| {
                    setup(&mut commands, &mut images)
                },
            )
            .add_systems(Update, draw);
        let mut session = super::super::Session::new(ZoneKind::GateFlight, 0, Some(1), 0);
        session.tick = 200;
        session.phase = Phase::Failure(151);
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::Bonus;
        game.bonus = Some(session);
        for layer in 0..=1 {
            app.world_mut().spawn((
                Camera3d::default(),
                crate::camera::WebCamera,
                Camera {
                    order: layer as isize,
                    ..default()
                },
                Transform::default(),
                Projection::default(),
                RenderLayers::layer(layer),
                Msaa::Sample4,
            ));
        }
        app.update();
        let snapshots: Vec<_> = app.world_mut()
            .query_filtered::<(Entity, &Camera, &RenderTarget), With<crate::messages::geometry::OverlayCamera>>()
            .iter(app.world())
            .map(|(entity, camera, target)| {
                assert!(camera.order < 0);
                assert!(matches!(target, RenderTarget::Image(_)));
                entity
            })
            .collect();
        assert_eq!(snapshots.len(), 2);
        let request = app
            .world_mut()
            .query_filtered::<Entity, With<Screenshot>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(ScreenshotCaptured {
            entity: request,
            image: frame(),
        });
        app.update();
        assert!(
            snapshots
                .iter()
                .all(|&entity| app.world().get_entity(entity).is_err())
        );
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            1
        );
        let canvas = app
            .world_mut()
            .query_filtered::<Entity, With<Canvas>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Visibility>(canvas),
            Some(&Visibility::Visible)
        );
        let mut game = app.world_mut().resource_mut::<Game>();
        let session = game.bonus.as_mut().unwrap();
        session.tick += 20;
        session.phase = Phase::Failure(131);
        app.update();
        assert_eq!(app.world().resource::<Feedback>().age, 6);
        app.world_mut().resource_mut::<Game>().status = Status::Playing;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(canvas),
            Some(&Visibility::Hidden)
        );
        app.world_mut().trigger(ScreenshotCaptured {
            entity: request,
            image: frame(),
        });
        assert!(app.world().resource::<Feedback>().pixels.is_empty());
    }
}
