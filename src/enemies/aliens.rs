//! Alien Droids climb past the rim, traverse lanes in sixteen delayed steps,
//! and zap for thirty-two ticks. During ascent, hits repel them toward the rear.

use crate::{
    animations::glider_death::Death,
    cry,
    enemies::{Enemy, flipper_render::draw_approach_dot, tankers::Tanker},
    game::{Game, Status},
    glider::Glider,
    level::{Level, LevelActor},
    mesh::colored_mesh,
    powerups::Upgrades,
    weapons::Weapon,
    web::{LANE_UNIT, WORLD_UNIT, Web},
};
use bevy::{camera::visibility::RenderLayers, prelude::*};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Motion {
    Climb,
    Ready,
    Cross {
        from: usize,
        to: usize,
        step: u8,
        delay: u8,
    },
    Zap(u8),
}

#[derive(Component)]
#[require(LevelActor, Enemy)]
pub(crate) struct AlienDroid {
    pub(crate) lane: usize,
    pub(crate) depth: i32,
    pub(crate) approach: bool,
    pub(crate) direction: i32,
    pub(crate) angle: u8,
    pub(crate) motion: Motion,
}

impl AlienDroid {
    /// One enemy tick of the lane crossing and the zap that follows it.
    fn step(&mut self, web: &Web) {
        self.angle = self.angle.wrapping_sub(2);
        match self.motion {
            Motion::Climb => {}
            Motion::Ready => {
                if web.neighbor(self.lane, self.direction).is_none() {
                    self.direction = -self.direction;
                }
                let to = web.neighbor(self.lane, self.direction).unwrap();
                self.motion = Motion::Cross {
                    from: self.lane,
                    to,
                    step: 0,
                    delay: 2,
                };
            }
            Motion::Cross {
                from,
                to,
                step,
                delay,
            } => {
                if delay > 0 {
                    self.motion = Motion::Cross {
                        from,
                        to,
                        step,
                        delay: delay - 1,
                    };
                } else {
                    let step = step + 1;
                    if step > 8 {
                        self.lane = to;
                    }
                    self.motion = if step == 16 {
                        Motion::Zap(32)
                    } else {
                        Motion::Cross {
                            from,
                            to,
                            step,
                            delay: 2,
                        }
                    };
                }
            }
            Motion::Zap(ticks) => {
                self.angle = self.angle.wrapping_add(3);
                self.motion = if ticks == 1 {
                    Motion::Ready
                } else {
                    Motion::Zap(ticks - 1)
                };
            }
        }
    }

    pub(crate) fn target(&self) -> Option<(usize, i32)> {
        (!self.approach).then_some((self.lane, self.depth))
    }
    pub(crate) fn climbing(&self) -> bool {
        matches!(self.motion, Motion::Climb)
    }
    pub(crate) fn hit(&mut self, speed: i32, rear: i32) -> bool {
        if !self.climbing() {
            return true;
        }
        self.depth += speed * 16;
        self.depth >= rear
    }
    pub(crate) fn transform(&self, web: &Web) -> Transform {
        let mut pose = web.glider_transform(self.lane as i32 * LANE_UNIT);
        if let Motion::Cross { from, to, step, .. } = self.motion {
            pose.translation = web
                .glider_transform(from as i32 * LANE_UNIT)
                .translation
                .lerp(
                    web.glider_transform(to as i32 * LANE_UNIT).translation,
                    f32::from(step) / 16.,
                );
        }
        pose.translation.z = -self.depth as f32 / LANE_UNIT as f32 * WORLD_UNIT + 0.06;
        pose.rotation =
            web.orientation(self.lane) * Quat::from_rotation_z(-f32::from(self.angle) * TAU / 256.);
        pose
    }
}

#[derive(Component)]
pub(crate) struct ZapBeam;

#[derive(Resource, Default)]
pub(crate) struct AlienAssets {
    pub(crate) body: Handle<Mesh>,
    pub(crate) point: Handle<Mesh>,
    pub(crate) beam: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) shades: Vec<Handle<StandardMaterial>>,
    pub(crate) beam_shades: Vec<Handle<StandardMaterial>>,
}

pub(crate) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let vertices = [
        (1., 1.),
        (17., 1.),
        (17., 17.),
        (1., 17.),
        (5., 5.),
        (13., 5.),
        (13., 13.),
        (5., 13.),
    ]
    .map(|(x, y)| Vec3::new((x - 9.) * 0.075, (9. - y) * 0.075, 0.));
    let faces = [
        (0x55, [0, 1, 4], [255, 192, 192]),
        (0x5b, [1, 2, 5], [192, 128, 128]),
        (0xbb, [2, 3, 6], [128, 192, 64]),
        (0xb5, [3, 0, 7], [192, 255, 128]),
        (0x56, [4, 5, 1], [192, 128, 192]),
        (0x5c, [5, 6, 2], [128, 64, 128]),
        (0xbc, [6, 7, 3], [64, 128, 192]),
        (0xb6, [7, 4, 0], [128, 192, 240]),
    ];
    let beam = colored_mesh((0..32).flat_map(|i| {
        let point = |index: usize| {
            let jitter = if index == 0 || index == 32 {
                0.
            } else {
                ((index * 17 % 7) as f32 - 3.) * 0.012
            };
            Vec3::new(jitter, 0., -(index as f32) / 32.)
        };
        let side = Vec3::Y * 0.015;
        let a = point(i);
        let b = point(i + 1);
        let [a, b, c, d] = [a - side, a + side, b + side, b - side].map(|p| (p, Color::WHITE));
        [[a, b, c], [a, c, d]]
    }));
    commands.insert_resource(AlienAssets {
        body: meshes.add(colored_mesh(faces.map(|(hue, face, intensity)| {
            std::array::from_fn(|i| (vertices[face[i]], cry::color(hue, intensity[i])))
        }))),
        point: meshes.add(Rectangle::new(1., 1.)),
        beam: meshes.add(beam),
        material: materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        shades: (0..=255)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: cry::color(0x55, i),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
        beam_shades: (0..16)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color: cry::color(0x80 | i, 255),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
    });
}

pub(crate) fn spawn(
    level: Res<Level>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    assets: Res<AlienAssets>,
    living: Query<(), With<Enemy>>,
    tankers: Query<(), With<Tanker>>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(generator) = &level.aliens else {
        return;
    };
    if game.status != Status::Playing || game.remaining == 0 {
        return;
    }
    if !crate::difficulty::due(&mut game.alien_timer, clock.steps()) {
        return;
    }
    if crate::enemies::occupancy(&living, &tankers) >= level.flippers.max_active {
        return;
    }
    game.advance_random();
    let lane = (game.random >> 16) as usize % level.web.lanes();
    game.advance_random();
    let alien = AlienDroid {
        lane,
        depth: level.web.rear() + 300 * LANE_UNIT,
        approach: true,
        direction: if (game.random >> 24) & 1 == 0 { -1 } else { 1 },
        angle: 0,
        motion: Motion::Climb,
    };
    let id = commands
        .spawn((
            alien.transform(&level.web),
            alien,
            RenderLayers::layer(1),
            Mesh3d(assets.point.clone()),
            MeshMaterial3d(assets.shades.first().cloned().unwrap_or_default()),
        ))
        .id();
    commands.spawn((
        ChildOf(id),
        ZapBeam,
        RenderLayers::layer(1),
        Transform::default(),
        Visibility::Hidden,
        Mesh3d(assets.beam.clone()),
        MeshMaterial3d(assets.beam_shades.first().cloned().unwrap_or_default()),
    ));
    game.remaining -= 1;
    game.alien_timer = generator.spawn_interval_ticks - 1;
}

#[allow(clippy::type_complexity)]
pub(crate) fn advance(
    level: Res<Level>,
    game: Res<Game>,
    assets: Res<AlienAssets>,
    mut aliens: Query<(
        &mut AlienDroid,
        &mut Transform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    let Some(generator) = &level.aliens else {
        return;
    };
    if game.status != Status::Playing {
        return;
    }
    for (mut alien, mut pose, mut mesh, mut material) in &mut aliens {
        if alien.approach {
            alien.depth -= clock.scaled(2 * LANE_UNIT);
            if alien.depth <= level.web.rear() {
                alien.depth = level.web.rear();
                alien.approach = false;
                mesh.0 = assets.body.clone();
                material.0 = assets.material.clone();
            }
        } else if matches!(alien.motion, Motion::Climb) {
            alien.angle = alien.angle.wrapping_sub(2 * clock.steps() as u8);
            alien.depth -= clock.scaled(generator.climb_speed);
            if alien.depth <= -15 * LANE_UNIT {
                alien.depth = -15 * LANE_UNIT;
                alien.motion = Motion::Ready;
            }
        } else {
            // Crossing and zapping advance in whole enemy ticks.
            for _ in 0..clock.steps() {
                alien.step(&level.web);
            }
        }
        *pose = alien.transform(&level.web);
    }
}

pub(crate) fn check_player(
    level: Res<Level>,
    mut game: ResMut<Game>,
    mut upgrades: ResMut<Upgrades>,
    mut player: Single<(&mut Glider, &mut Weapon, &Transform, &mut Visibility)>,
    aliens: Query<&AlienDroid>,
) {
    if game.status != Status::Playing || game.invulnerable > 0 || upgrades.jump_height > 0 {
        return;
    }
    let lane = level.web.selected_lane(player.0.progress) as usize;
    if aliens
        .iter()
        .any(|alien| alien.lane == lane && matches!(alien.motion, Motion::Zap(_)))
    {
        game.status = Status::Fried;
        game.announce(crate::messages::Id::FriedYou);
        game.death = Some(Death::new(player.2.translation, &level.web));
        player.0.velocity = 0;
        *player.1 = Weapon::default();
        *player.3 = Visibility::Hidden;
        upgrades.lose_life();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_points(
    assets: Res<AlienAssets>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<Camera3d>,
            Without<AlienDroid>,
            Without<crate::messages::geometry::OverlayCamera>,
        ),
    >,
    mut aliens: Query<(
        &AlienDroid,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some((camera, Projection::Perspective(projection))) = cameras.iter().next() else {
        return;
    };
    for (alien, mut pose, mut material) in &mut aliens {
        if alien.approach {
            draw_approach_dot(&mut pose, &mut material, &assets.shades, camera, projection);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_beams(
    level: Res<Level>,
    assets: Res<AlienAssets>,
    aliens: Query<(&AlienDroid, &Children)>,
    mut beams: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<ZapBeam>,
    >,
) {
    for (alien, children) in &aliens {
        for child in children {
            let Ok((mut pose, mut visible, mut material)) = beams.get_mut(*child) else {
                continue;
            };
            visible.set_if_neq(if matches!(alien.motion, Motion::Zap(_)) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
            let length = (level.web.rear() - alien.depth) as f32 / LANE_UNIT as f32 * WORLD_UNIT;
            if pose.scale.z != length {
                pose.scale.z = length;
            }
            material.set_if_neq(MeshMaterial3d(
                assets.beam_shades[(alien.angle & 15) as usize].clone(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "tests/aliens.rs"]
mod tests;
