//! Moving background stars and warp depth samples. Draws screen-sized dots
//! behind the web with seeded positions and original depth-cued brightness.

use crate::game::{Game, Status};
use crate::mesh::colored_mesh;
use crate::web::LANE_UNIT;
use crate::web::WORLD_UNIT;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

mod patterns;
pub(crate) use patterns::{Animation, Pattern};

pub(crate) const COUNT: usize = 256;

pub(crate) const DEPTH_WRAP: i32 = 512 * LANE_UNIT;

pub(crate) const STEP: i32 = 2 * LANE_UNIT;

pub(crate) const BACKGROUND_DISTANCE: f32 = 100.0;

#[derive(Resource, Default)]
pub(crate) struct Warp {
    pub(crate) distance: i32,
    pub(crate) spacing: i32,
    pub(crate) copies: u8,
}

pub(crate) struct Star {
    pub(crate) position: Vec2,
    pub(crate) depth: i32,
    pub(crate) color: Color,
}

impl Star {
    pub(crate) fn advance(&mut self, step: i32) {
        self.depth = (self.depth - step).rem_euclid(DEPTH_WRAP);
    }

    pub(crate) fn projected(&self, viewpoint: Vec2) -> Vec2 {
        // llama.gas divides by (z >> 14) + 1, then shifts by seven.
        (self.position - viewpoint) * (128.0 / (self.depth as f32 / LANE_UNIT as f32 + 0.25))
    }

    fn background_position(&self, camera: &Transform) -> Option<Vec2> {
        let depth = (self.depth as f32 / LANE_UNIT as f32 + 0.25) * WORLD_UNIT;
        let relative = camera.rotation.inverse()
            * (self.position.extend(0.) * WORLD_UNIT - camera.translation.truncate().extend(depth));
        let distance = -relative.z;
        (distance > 0.).then(|| relative.truncate() * (BACKGROUND_DISTANCE / distance))
    }

    pub(crate) fn color(&self) -> Color {
        let intensity = (255 - (self.depth >> 17)) as f32 / 255.0;
        let color = self.color.to_srgba();
        Color::srgb(
            color.red * intensity,
            color.green * intensity,
            color.blue * intensity,
        )
    }
}

#[derive(Resource)]
pub(crate) struct Stars(pub(crate) Vec<Star>);

#[derive(Resource)]
pub(crate) struct BonusStars(Vec<Star>);

impl Default for BonusStars {
    fn default() -> Self {
        Self(patterns::rings(400))
    }
}

impl Default for Stars {
    fn default() -> Self {
        Self(Pattern::Level.stars(1))
    }
}

#[derive(Component)]
pub(crate) struct StarField;

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.insert_resource(Stars::default());
    commands.insert_resource(BonusStars::default());
    commands.insert_resource(Warp::default());
    commands.spawn((
        StarField,
        RenderLayers::layer(0),
        Mesh3d(meshes.add(colored_mesh([]))),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        // The mesh changes its bounds as stars cross the screen.
        bevy::camera::visibility::NoFrustumCulling,
    ));
}

pub(crate) fn add_systems(app: &mut App) {
    app.init_resource::<Animation>()
        .add_systems(
            FixedUpdate,
            advance_stars
                .after(crate::game::load_level)
                .run_if(crate::messages::running),
        )
        .add_systems(Update, draw_stars.after(crate::warp::render::draw));
}

pub(crate) fn advance_stars(
    mut stars: ResMut<Stars>,
    mut animation: ResMut<Animation>,
    level: Res<crate::level::Level>,
    game: Option<Res<Game>>,
) {
    let epoch = game.as_ref().map(|game| game.message_epoch);
    if animation.epoch != epoch {
        animation.offset = 0;
        animation.tick = 0;
        animation.level = None;
        animation.epoch = epoch;
    }
    if level.is_changed()
        || animation.level != Some(level.number)
        || animation.selected != Some(animation.pattern)
    {
        stars.0 = animation.pattern.stars(level.number);
        for star in &mut stars.0 {
            star.depth = (star.depth - animation.offset).rem_euclid(DEPTH_WRAP);
        }
        animation.level = Some(level.number);
        animation.selected = Some(animation.pattern);
        animation.entry_motion.clear();
        animation.entry_warp = Warp::default();
        if animation.pattern == Pattern::Level
            && game
                .as_ref()
                .is_some_and(|game| game.status == Status::Entering && game.entry.is_some())
        {
            let mut exit = crate::animations::level_exit::Exit::new(
                level.web.glider_transform(level.web.start()).translation,
                &level.web,
            );
            while exit.flash == 0 {
                exit.advance();
                animation
                    .entry_motion
                    .push((exit.speed, i32::from(exit.ticks) * 0x4000));
            }
        }
    }
    let mut step = animation.pattern.step();
    if game.as_ref().is_some_and(|game| {
        !matches!(game.status, Status::Entering | Status::Playing) || game.exit.is_some()
    }) {
        animation.entry_motion.clear();
    }
    if let Some((speed, spacing)) = animation.entry_motion.pop() {
        step += speed;
        animation.entry_warp.spacing = spacing;
        animation.entry_warp.copies = 3;
    } else {
        animation.entry_warp = Warp::default();
    }
    for star in &mut stars.0 {
        star.advance(step);
    }
    animation.offset = (animation.offset + step).rem_euclid(DEPTH_WRAP);
    animation.tick = animation.tick.wrapping_add(1);
}

// Project the moving stars onto a distant camera-aligned plane. This keeps the
// original's single-pixel dots while allowing the web to cover the background.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn draw_stars(
    stars: Res<Stars>,
    warp: Res<Warp>,
    game: Option<Res<Game>>,
    bonus_stars: Option<Res<BonusStars>>,
    animation: Option<Res<Animation>>,
    mut meshes: ResMut<Assets<Mesh>>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<StarField>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut field: Single<(&Mesh3d, &mut Transform), With<StarField>>,
    front: Option<Res<crate::menu::FrontEnd>>,
) {
    // Menus draw their own star field with the preview camera.
    if front.is_some_and(|front| front.screen != crate::menu::Screen::Gameplay) {
        return;
    }
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    let (pose, half, pixel) = background_plane(camera, projection);
    let bonus = game
        .as_ref()
        .filter(|game| game.status == Status::Bonus)
        .and_then(|game| game.bonus.as_ref());
    let bonus_warp = Warp {
        distance: bonus.map_or(0, |session| {
            ((session.tick % 512 * 3) % 512) as i32 * LANE_UNIT
        }),
        spacing: 26 * LANE_UNIT,
        copies: 5,
    };
    let (copies, spacing) = animation
        .as_ref()
        .map_or((0, 0), |animation| animation.pattern.trails(animation.tick));
    let pattern_warp = Warp {
        distance: 0,
        copies,
        spacing,
    };
    let warp = if bonus.is_some() {
        &bonus_warp
    } else if warp.copies == 0
        && let Some(animation) = &animation
        && animation.entry_warp.copies > 0
    {
        &animation.entry_warp
    } else if warp.copies == 0 && copies > 0 {
        &pattern_warp
    } else {
        &*warp
    };
    let stars = if bonus.is_some() {
        bonus_stars.as_ref().map_or(&stars.0, |stars| &stars.0)
    } else {
        &stars.0
    };
    let particles = bonus.is_some_and(|session| session.kind != crate::warp::ZoneKind::GateFlight);
    let centres = stars.iter().filter(|_| !particles).flat_map(|star| {
        (0..=warp.copies).filter_map(move |index| {
            let star = Star {
                position: star.position,
                depth: (star.depth - warp.distance + i32::from(index) * warp.spacing)
                    .rem_euclid(DEPTH_WRAP),
                color: star.color,
            };
            let centre = if bonus.is_some() {
                star.projected(Vec2::ZERO) * pixel
            } else {
                star.background_position(camera)?
            };
            Some((centre, star.color()))
        })
    });
    let (mesh, transform) = &mut *field;
    if let Some(mesh) = meshes.get_mut(&mesh.0) {
        *mesh = star_mesh(centres, half, pixel);
    }
    **transform = pose;
}

/// The camera-aligned plane stars are drawn on: its pose, its half extent, and the
/// size there of one pixel of the original 384-wide screen.
pub(crate) fn background_plane(
    camera: &Transform,
    projection: &PerspectiveProjection,
) -> (Transform, Vec2, f32) {
    let half_height = BACKGROUND_DISTANCE * (projection.fov * 0.5).tan();
    let half = Vec2::new(half_height * projection.aspect_ratio, half_height);
    let pose = Transform::from_translation(
        camera.translation + camera.rotation * Vec3::NEG_Z * BACKGROUND_DISTANCE,
    )
    .with_rotation(camera.rotation);
    (pose, half, 2.0 * half.x / 384.0)
}

/// One-pixel quads at star centres on the background plane, skipping stars outside it.
pub(crate) fn star_mesh(
    stars: impl IntoIterator<Item = (Vec2, Color)>,
    half: Vec2,
    pixel: f32,
) -> Mesh {
    colored_mesh(
        stars
            .into_iter()
            .filter(move |(centre, _)| centre.x.abs() <= half.x && centre.y.abs() <= half.y)
            .flat_map(move |(centre, color)| {
                let quad = [
                    Vec2::new(-0.5, -0.5),
                    Vec2::new(0.5, -0.5),
                    Vec2::new(0.5, 0.5),
                    Vec2::new(-0.5, 0.5),
                ]
                .map(|corner| ((centre + corner * pixel).extend(0.0), color));
                [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
            }),
    )
}

#[cfg(test)]
#[path = "tests/stars.rs"]
mod tests;
