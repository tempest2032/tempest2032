//! Manual full sweep and last-chance zap, with yak.s fire/run_objects cadence.

use super::Upgrades;
use crate::{
    enemies::{
        Enemy, SpawnOrder,
        aliens::AlienDroid,
        beasts::Reflected,
        flipper_render::FlipperAssets,
        flippers::{Flipper, Motion},
        fuseballs::{self, Fuseball},
        mirrors::ReturnedShot,
        pulsar_render::PulsarAssets,
        pulsars::Pulsar,
        shots::EnemyShot,
        spikers::Spiker,
        tankers::{self, Contents, Tanker, TankerAssets},
    },
    explosion,
    game::{Game, Status},
    glider::Glider,
    level::Level,
    messages::Id,
    web::{FRONT_RADIUS, WORLD_UNIT, Web},
};
use bevy::{camera::visibility::RenderLayers, prelude::*};

pub(crate) struct Superzapper {
    uses: u8,
    pub(crate) queued: bool,
    active: bool,
    single: bool,
    delay: u8,
    bolt: Option<Vec3>,
    bolt_ticks: u8,
    frame: u32,
}

impl Default for Superzapper {
    fn default() -> Self {
        Self {
            uses: 2,
            queued: false,
            active: false,
            single: false,
            delay: 0,
            bolt: None,
            bolt_ticks: 0,
            frame: 0,
        }
    }
}

impl Superzapper {
    pub(super) fn cancel(&mut self) {
        self.active = false;
        self.queued = false;
        self.bolt = None;
        self.bolt_ticks = 0;
        self.delay = 0;
    }

    pub(super) fn powerup(&mut self) {
        // sprzap restores the desperate zap if both manual uses were spent.
        self.uses = self.uses.max(1);
        self.active = true;
        self.single = false;
        self.delay = 0;
        self.queued = false;
    }
}

pub(crate) fn queue(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<Game>,
    mut upgrades: ResMut<Upgrades>,
) {
    let zapper = &mut upgrades.zapper;
    if game.status == Status::Playing && zapper.uses > 0 && !zapper.active {
        zapper.queued |= keys.just_pressed(KeyCode::KeyC);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn advance(
    mut commands: Commands,
    mut game: ResMut<Game>,
    level: Res<crate::level::Level>,
    mut upgrades: ResMut<Upgrades>,
    bursts: Res<explosion::BurstAssets>,
    flippers: Res<FlipperAssets>,
    shells: Res<TankerAssets>,
    fuses: Res<fuseballs::FuseAssets>,
    pulsars: Res<PulsarAssets>,
    targets: Query<
        (
            Entity,
            &Transform,
            Option<&Flipper>,
            Option<&Tanker>,
            Option<&Contents>,
            Option<&Fuseball>,
            Option<&Pulsar>,
            Option<&Spiker>,
            Option<&AlienDroid>,
            Option<&EnemyShot>,
            Option<&Reflected>,
            Option<&ReturnedShot>,
            &SpawnOrder,
        ),
        Or<(With<Enemy>, With<EnemyShot>)>,
    >,
) {
    let zapper = &mut upgrades.zapper;
    if game.status != Status::Playing {
        zapper.cancel();
        return;
    }
    zapper.frame = zapper.frame.wrapping_add(1);
    zapper.bolt_ticks = zapper.bolt_ticks.saturating_sub(1);
    if std::mem::take(&mut zapper.queued) && !zapper.active && zapper.uses > 0 {
        zapper.single = zapper.uses == 1;
        zapper.uses -= 1;
        zapper.active = true;
        zapper.delay = 0;
        if !zapper.single {
            game.announce(Id::EatElectricDeath);
        }
        crate::sound::play(&mut commands, crate::sound::Effect::Crackle);
    }
    if !zapper.active {
        return;
    }
    if zapper.delay > 0 {
        zapper.delay -= 1;
        return;
    }
    // collie bypasses shot windows and Beastie armor. mcollie-only enemies,
    // approaching pixels, spikes, and reflected shots cannot consume a zap.
    let target = targets
        .iter()
        .filter(
            |(
                _,
                _,
                flipper,
                tanker,
                _,
                fuse,
                pulsar,
                spiker,
                alien,
                shot,
                reflected,
                returned,
                _,
            )| {
                if let Some(flipper) = flipper {
                    !matches!(flipper.motion, Motion::Approach)
                } else if let Some(tanker) = tanker {
                    !tanker.approach
                } else if let Some(fuse) = fuse {
                    !matches!(fuse.motion, fuseballs::Motion::Approach)
                } else if let Some(pulsar) = pulsar {
                    !pulsar.approach
                } else if spiker.is_some() {
                    true
                } else if let Some(alien) = alien {
                    !alien.approach && !alien.climbing()
                } else {
                    shot.is_some() && !zapper.single && reflected.is_none() && returned.is_none()
                }
            },
        )
        .max_by_key(|(.., order)| order.0);
    let Some((entity, pose, flipper, tanker, contents, fuse, pulsar, spiker, alien, _, _, _, _)) =
        target
    else {
        zapper.active = false;
        return;
    };
    if let Some(tanker) = tanker {
        tankers::open(
            &mut commands,
            &mut game,
            &level,
            &flippers,
            &shells,
            &fuses,
            &pulsars,
            tanker,
            *contents.unwrap(),
        );
    } else {
        let points = if let Some(pulsar) = pulsar {
            pulsar.points()
        } else if flipper.is_some() || spiker.is_some() {
            150
        } else if fuse.is_some() || alien.is_some() {
            game.bonus_points()
        } else {
            0
        };
        game.add_score(points);
        if fuse.is_some() || alien.is_some() {
            game.score_popup(points, pose.translation);
        }
    }
    commands.entity(entity).despawn();
    explosion::spawn_explosion(&mut commands, &bursts, pose.translation);
    if zapper.single {
        // r_end1 falls through clzapa and clears bolt_lock before drawing.
        zapper.cancel();
    } else {
        zapper.bolt = Some(pose.translation);
        zapper.bolt_ticks = 4;
        zapper.delay = 3;
    }
}

#[derive(Resource)]
pub(crate) struct BoltAssets(Handle<Mesh>);

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(crate::mesh::colored_mesh(Vec::new()));
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0., 0., crate::cry::BEHIND_HUD),
        RenderLayers::layer(3),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.insert_resource(BoltAssets(mesh));
}

fn bolt_origins(web: &Web, progress: i32, z: f32) -> [Vec3; 2] {
    // draw_objects calls fline twice, using webinfo's two lane ends at claw Z.
    let lane = web.selected_lane(progress);
    [lane, lane + 1.].map(|edge| web.vertex(edge, FRONT_RADIUS, z))
}

fn bolt_points(target: Vec3, origin: Vec3, frame: u32) -> [Vec3; 33] {
    let mut seed = frame as u8 | 1;
    std::array::from_fn(|i| {
        if i == 0 {
            return target;
        }
        if i == 32 {
            return origin;
        }
        let p = target.lerp(origin, i as f32 / 32.);
        // ox.gas: (8-bit LFSR - 128) << 10 in 16:16 XY; Z is unchanged.
        let mut noise = || {
            seed = (seed >> 1) ^ if seed & 1 != 0 { 0xb8 } else { 0 };
            (f32::from(seed) - 128.) / 64. * WORLD_UNIT
        };
        p + Vec3::new(noise(), -noise(), 0.)
    })
}

/// `gamefx` rebuilds `flashcol` every frame: `sine[frames]` supplies the high
/// nibble and `sine[frames + 0x40]` the low one, so the bolt pulses through the
/// CRY palette instead of holding a fixed colour.
fn bolt_hue(frame: u32) -> u8 {
    let phase = (frame & 255) as i32;
    (((crate::sines::sine(phase) + 128) & 0xf0) | ((crate::sines::sine(phase + 0x40) + 128) >> 4))
        as u8
}

fn bolt_triangles(vertices: &[(Vec2, Color)]) -> Vec<[(Vec3, Color); 3]> {
    vertices
        .windows(2)
        .flat_map(|edge| {
            let side = (edge[1].0 - edge[0].0).normalize_or_zero().perp() * 0.5;
            let corners = [
                ((edge[0].0 - side).extend(0.5), edge[0].1),
                ((edge[0].0 + side).extend(0.5), edge[0].1),
                ((edge[1].0 + side).extend(0.5), edge[1].1),
                ((edge[1].0 - side).extend(0.5), edge[1].1),
            ];
            [
                [corners[0], corners[1], corners[2]],
                [corners[0], corners[2], corners[3]],
            ]
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    upgrades: Res<Upgrades>,
    game: Res<Game>,
    level: Res<Level>,
    assets: Res<BoltAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    player: Single<(&Glider, &Transform)>,
    cameras: Query<(&Camera, &GlobalTransform), crate::messages::geometry::GameCameras>,
    mut empty: Local<bool>,
) {
    let zapper = &upgrades.zapper;
    let mut triangles = Vec::new();
    if let Some(target) = zapper
        .bolt
        .filter(|_| zapper.bolt_ticks > 0 && game.status == Status::Playing)
        && let Some((camera, pose, size)) = cameras.iter().find_map(|(camera, pose)| {
            camera
                .logical_viewport_size()
                .map(|size| (camera, pose, size))
        })
    {
        let (glider, claw) = *player;
        let view = pose.affine().inverse();
        let hue = bolt_hue(zapper.frame);
        for origin in bolt_origins(&level.web, glider.progress, claw.translation.z) {
            let projected: Option<Vec<_>> = bolt_points(target, origin, zapper.frame)
                .iter()
                .map(|&p| {
                    let xy = camera.world_to_viewport(pose, p).ok()?;
                    let xy = Vec2::new(xy.x - size.x / 2., size.y / 2. - xy.y) * (240. / size.y);
                    // fline depth-cues with z_max=1300 and Z shifted from 16:16 to 16:2.
                    let depth = -view.transform_point3(p).z;
                    let intensity =
                        (255. * (1. - depth / (1300. * WORLD_UNIT / 4.)).clamp(0., 1.)) as u8;
                    Some((xy, crate::cry::color(hue, intensity)))
                })
                .collect();
            if let Some(projected) = projected {
                triangles.extend(bolt_triangles(&projected));
            }
        }
    }
    // An empty mesh stays empty, so skip re-uploading it on the frames without a bolt.
    let was_empty = std::mem::replace(&mut *empty, triangles.is_empty());
    if !(was_empty && *empty) {
        *meshes.get_mut(&assets.0).unwrap() = crate::mesh::colored_mesh(triangles);
    }
}

#[cfg(test)]
#[path = "tests/superzapper.rs"]
mod tests;
