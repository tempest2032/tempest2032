//! yak.s _tunn/tundraw course buffer, angular controls, and particle tail.

use super::{Axis, Phase, UNIT, sine};
use bevy::prelude::*;
use std::collections::VecDeque;

pub(super) const ORDER: [usize; 8] = [0, 6, 2, 1, 3, 4, 5, 7];
const COURSES: [[i8; 64]; 8] = [
    [
        0, 0, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, -1, -1, -2, -2, -3, -3, -2, -1, 0, 2, 0, 2, 4, 2, 4,
        4, 4, 2, 0, -2, 2, -3, 3, -4, 4, 0, 0, 0, 1, 2, 3, 4, 5, 6, 6, -6, -6, -5, -4, -3, -2, -1,
        0, 8, 8, 8, 0, 0, 0, 0, 0,
    ],
    [
        0, 0, 2, 2, -2, -2, 2, 2, 0, 3, 0, 3, 0, 3, 0, 3, 0, -3, 0, -3, 0, -3, 0, -3, 6, 6, 6, 6,
        6, 6, 6, 6, 0, -6, -6, 0, -6, -6, 0, -6, -6, -6, -5, -5, -4, -4, -3, -3, -2, -2, -1, -1, 8,
        -2, 8, -2, -7, -5, -7, 0, 0, 0, 0, 0,
    ],
    [
        0, 0, 2, 2, 2, 2, 2, 2, -3, -3, -3, -3, -3, -3, -3, -3, 4, 4, 4, 4, 4, 4, 4, 4, -5, -5, -5,
        -5, -5, -5, -5, -5, 6, 6, 6, 6, 6, 6, 6, 6, -6, -6, -6, -6, -6, -6, -6, -6, 0, 0, 0, 0, 6,
        -6, 6, -6, 7, 7, 7, 7, 0, 0, 0, 0,
    ],
    [
        0, 0, 0, 0, 0, 0, 0, 0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, -7, -7, -7, -7, -7,
        -7, -7, -7, 7, 7, 7, 7, 7, 7, 7, 7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7,
        -7, -7, 8, 8, 8, 8, 0, 0, 0, 0,
    ],
    [
        0, 0, 2, 4, 6, 2, 6, 4, -4, -2, -3, -1, -4, -2, -3, -1, 2, 3, -8, 2, 3, -8, 2, 4, -2, -3,
        8, -2, -3, 8, -2, -4, -8, -8, -7, -7, -6, -6, -5, -5, -4, -4, -3, -3, -2, -2, -1, -1, 0, 1,
        2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 0, 0, 0,
    ],
    [
        0, 0, 4, 4, -12, 4, 4, -12, 0, -12, 1, -12, 2, -12, 3, -12, 4, -12, 5, -12, 6, -12, 7, -12,
        8, -12, 7, -12, 6, -12, 5, -12, 4, -12, 3, -12, 2, -12, 1, -12, 0, -12, 0, -12, 0, -12, 0,
        -12, 8, 8, 8, 8, 9, 9, 9, 9, -12, -12, -12, -12, 0, 0, 0, 0,
    ],
    [
        0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5,
        5, 5, 6, 6, 6, 6, 7, 7, 7, 7, -7, -7, -7, -7, -6, -6, -6, -6, -5, -5, -5, -5, -4, -4, -4,
        -4, -3, -3, -2, -2, -1, 0, 0, 0,
    ],
    [
        1, 2, 3, 4, -8, -8, 4, 5, 6, 7, -8, -8, 0, 0, 4, -9, 3, -2, 6, 2, 7, -4, -7, 2, -5, 2, -5,
        3, -8, 9, -5, 6, 8, 8, 8, -12, 8, 8, 8, -12, 12, -8, -8, -8, 12, -8, -8, -8, 7, 0, -7, 0,
        -7, -7, -7, 13, 13, 7, 7, -13, 0, 0, 0, 0,
    ],
];

#[derive(Clone, Copy, Debug)]
struct Arc {
    phase: u8,
    count: u8,
    hue: u8,
    offset: u8,
}

#[derive(Clone, Debug)]
struct Pattern {
    color: [i32; 2],
    velocity: [i32; 2],
    phase: u16,
    step: u16,
    count: u16,
    count_step: u16,
}

impl Pattern {
    fn advance(&mut self) -> Arc {
        for axis in 0..2 {
            self.color[axis] += self.velocity[axis];
            if !(0..=0xfff).contains(&self.color[axis]) {
                self.velocity[axis] = -self.velocity[axis];
                self.color[axis] += self.velocity[axis];
            }
        }
        self.phase = self.phase.wrapping_add(self.step);
        self.count = self.count.wrapping_add(self.count_step);
        Arc {
            phase: (self.phase >> 8) as u8,
            count: (((self.count >> 8) & 63) as i32 - 31).unsigned_abs() as u8 + 16,
            hue: ((self.color[0] >> 8) | ((self.color[1] >> 4) & 0xf0)) as u8,
            offset: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Tunnel {
    pub(crate) roll: Axis,
    pub(crate) tail: i32,
    pub(crate) segments: u32,
    pub(crate) on_track: bool,
    pub(crate) viewpoint: Vec2,
    pub(crate) star_offset: Vec2,
    arcs: [[Arc; 3]; 64],
    patterns: [Pattern; 2],
    track_phase: u16,
    turn: i16,
    trail: VecDeque<u8>,
    pattern_wait: u16,
    random: u32,
    radius: u8,
}

impl Default for Tunnel {
    fn default() -> Self {
        Self {
            roll: Axis {
                position: 0,
                velocity: 0,
            },
            tail: 3 * UNIT,
            segments: 0,
            on_track: true,
            viewpoint: Vec2::ZERO,
            star_offset: Vec2::ZERO,
            arcs: std::array::from_fn(|index| {
                [
                    Arc {
                        phase: index as u8,
                        count: 0x18,
                        hue: 0x01,
                        offset: 0x80,
                    },
                    Arc {
                        phase: index as u8,
                        count: 0x1b,
                        hue: 0x02,
                        offset: 0,
                    },
                    Arc {
                        phase: 0xe2,
                        count: 0x40,
                        hue: 0,
                        offset: 0,
                    },
                ]
            }),
            patterns: [
                Pattern {
                    color: [0x80, 0x40],
                    velocity: [0x16, 0x1c],
                    phase: 0x2000,
                    step: 0x100,
                    count: 0x800,
                    count_step: 0x60,
                },
                Pattern {
                    color: [0x50, 0x70],
                    velocity: [9, 15],
                    phase: 0x6000,
                    step: 0x280,
                    count: 0xc00,
                    count_step: 0xc0,
                },
            ],
            track_phase: 0,
            turn: 0,
            trail: VecDeque::new(),
            pattern_wait: 300,
            random: 0x2000,
            radius: 32,
        }
    }
}

impl Tunnel {
    fn next_random(&mut self) -> u16 {
        self.random = crate::game::next_random(self.random);
        (self.random >> 16) as u16
    }

    fn advance_patterns(&mut self) {
        if self.pattern_wait == 0 {
            self.pattern_wait = 800;
            for index in 0..2 {
                self.patterns[index].velocity = [
                    i32::from((self.next_random() & 15) + 1) * 4,
                    i32::from((self.next_random() & 7) + 1) * 4,
                ];
                self.patterns[index].step = self.next_random().wrapping_mul(2);
                self.patterns[index].count_step = (self.next_random() & 127) + 16;
            }
        } else {
            self.pattern_wait -= 1;
        }
    }

    pub(super) fn settle(&mut self, phase: Phase) {
        match phase {
            Phase::Victory(_) => {
                self.roll.position = (self.roll.position + 6 * UNIT).rem_euclid(256 * UNIT);
                self.roll.velocity = 0;
                self.tail = 31 * UNIT;
                self.trail.push_front((self.roll.position >> 16) as u8);
                self.trail.truncate(32);
                self.viewpoint *= 0.9;
                self.advance_patterns();
                let index = self.segments as usize & 63;
                self.arcs[index][0] = self.patterns[0].advance();
                self.arcs[index][1] = self.patterns[1].advance();
                self.segments += 1;
            }
            Phase::Failure(ticks) => {
                self.radius = ticks.saturating_sub(152).min(32) as u8;
                self.viewpoint -= self.viewpoint.clamp(Vec2::splat(-0.25), Vec2::splat(0.25));
            }
            _ => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn target(&self) -> u8 {
        self.arcs[(self.segments as usize + 8) & 63][2].offset
    }

    pub(super) fn step(&mut self, direction: i32, course: usize) -> (u32, Option<Phase>) {
        self.advance_patterns();
        self.roll.step(direction, 0x1000, 0x100, 0x40000, 0, 0);
        self.roll.position = self.roll.position.rem_euclid(256 * UNIT);
        let angle = (self.roll.position >> 16) as u8;
        let target = Vec2::new(
            -((sine(i32::from(angle) - 1) >> 3) as f32) / 2.,
            -((sine(i32::from(angle) + 63) >> 3) as f32) / 2.,
        );
        for axis in 0..2 {
            if target[axis] != self.viewpoint[axis] {
                self.viewpoint[axis] += (target[axis] - self.viewpoint[axis]).signum() * 0.25;
            }
        }
        self.star_offset -= target;
        self.track_phase = self.track_phase.wrapping_add(self.turn as u16);
        let index = self.segments as usize & 63;
        self.arcs[index][0] = self.patterns[0].advance();
        self.arcs[index][1] = self.patterns[1].advance();
        self.arcs[index][2] = Arc {
            phase: 0xe2,
            count: 0x40,
            hue: if index & 15 > 13 { 0xf0 } else { 0x8f },
            offset: (self.track_phase >> 8) as u8,
        };
        let complete = self.segments == 1023;
        if index & 15 == 15 {
            self.turn = i16::from(COURSES[course][self.segments as usize / 16]) * 64;
        }
        let track = &mut self.arcs[(index + 8) & 63][2];
        // tundraw negates both bytes, then uses a 255-complement at the seam.
        let distance =
            (i16::from(angle.wrapping_neg()) - i16::from(track.offset.wrapping_neg())).abs();
        let distance = if distance > 127 {
            255 - distance
        } else {
            distance
        };
        self.on_track = distance <= 32;
        let mut score = 0;
        let mut phase = None;
        if complete {
            phase = Some(Phase::Success(1));
        } else if self.on_track {
            track.hue = 0;
            track.count = 0x20;
            score = [1, 4, 8, 30][(self.tail >> 19) as usize];
            if self.tail + 0x8000 <= 0x1fffff {
                self.tail += 0x8000;
            }
        } else {
            self.tail -= UNIT;
            if self.tail < 0 {
                phase = Some(Phase::Failure(183));
            }
        }
        self.trail.push_front(angle);
        self.trail.truncate(32);
        self.segments += 1;
        (score, phase)
    }

    /// Pixels for this frame; `ahead` fifths of a segment of progress toward the
    /// next step scroll the rings smoothly when the tempo is not a whole tick.
    pub(super) fn pixels(&self, tick: u32, ahead: i32) -> Vec<(Vec2, Color)> {
        let mut pixels = Vec::new();
        // donky.gas starplane uses repeatable LFSRs on sixteen scrolling planes.
        for plane in 0..16 {
            let mut x = 16 - plane;
            let mut y = 84;
            let hue = ((sine((tick / 2) as i32 + plane * 4) + 128) as u8 & 0xf0) | 8;
            for _ in 0..32 {
                x = (x >> 1) ^ if x & 1 == 1 { 0x110 } else { 0 };
                y = (y >> 1) ^ if y & 1 == 1 { 0xb8 } else { 0 };
                let point = Vec2::new(
                    ((x + ((self.star_offset.x as i32 * (plane + 1)) >> 5)) & 511) as f32,
                    ((y + ((self.star_offset.y as i32 * (plane + 1)) >> 5)) & 511) as f32,
                );
                if point.x < 384. && point.y < 240. {
                    pixels.push((
                        point - Vec2::new(192., 120.),
                        crate::cry::color(hue, (0x70 + plane * 8) as u8),
                    ));
                }
            }
        }
        for depth in 0..64 {
            let z = 550 - depth * 8 - ahead * 8 / 5;
            let center = self.viewpoint * (512. / z as f32);
            let radius = i32::from(self.radius) * 512 / z;
            for arc in self.arcs[(self.segments as usize + 64 - depth as usize) & 63] {
                let span = sine(i32::from(arc.phase)) + 128;
                let start = i32::from(arc.offset) * 256 - (span / 2) * 256;
                let step = span * 256 / (i32::from(arc.count) + 1);
                for point in 0..=i32::from(arc.count) {
                    let angle = (start + point * step) >> 8;
                    pixels.push((
                        center
                            + Vec2::new(
                                ((sine(angle) * radius) >> 8) as f32,
                                ((sine(angle + 64) * radius) >> 8) as f32,
                            ),
                        crate::cry::color(arc.hue, (64 + depth * 3) as u8),
                    ));
                }
            }
        }
        // claps returns before drawing the player tail once the collapse starts.
        if self.tail < 0 {
            return pixels;
        }
        for (index, angle) in self
            .trail
            .iter()
            .take((self.tail.max(0) >> 16) as usize + 1)
            .enumerate()
        {
            let center = Vec2::new(
                (sine(i32::from(*angle) - 1) >> 3) as f32,
                (sine(i32::from(*angle) + 63) >> 3) as f32,
            ) / 2.;
            let z = (30 - index as i32).max(1) as f32;
            let hue = if index == 0 {
                0x88
            } else {
                ((tick as usize + index * 8) & 255) as u8
            };
            for offset in [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                pixels.push((
                    (center + offset * if index == 0 { 0.75 } else { 0.4 }) * (128. / z),
                    crate::cry::color(hue, 255),
                ));
            }
        }
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_failure_removes_the_tail_and_collapses_toward_screen_center() {
        let mut tunnel = Tunnel {
            tail: 0,
            viewpoint: Vec2::new(2., -2.),
            ..default()
        };
        tunnel.trail.push_front(128);
        let flying = tunnel.pixels(0, 0);
        tunnel.tail = -UNIT;
        assert_eq!(tunnel.pixels(0, 0), flying[..flying.len() - 5]);
        for ticks in (152..=183).rev() {
            tunnel.settle(Phase::Failure(ticks));
        }
        assert_eq!(tunnel.radius, 0);
        assert_eq!(tunnel.viewpoint, Vec2::ZERO);
    }

    #[test]
    fn track_tolerance_wraps_and_tail_grows_drains_and_scores_in_source_tiers() {
        for (angle, passes) in [
            (32, true),
            (33, true),
            (34, false),
            (224, true),
            (223, false),
            (255, true),
        ] {
            let mut tunnel = Tunnel::default();
            tunnel.roll.position = angle * UNIT;
            let (score, phase) = tunnel.step(0, 0);
            assert_eq!(tunnel.on_track, passes);
            assert_eq!(score, u32::from(passes));
            assert!(phase.is_none());
            assert_eq!(
                tunnel.tail,
                if passes { 3 * UNIT + 0x8000 } else { 2 * UNIT }
            );
        }
        for (tail, score) in [(3, 1), (8, 4), (16, 8), (24, 30)] {
            let mut tunnel = Tunnel {
                tail: tail * UNIT,
                ..default()
            };
            assert_eq!(tunnel.step(0, 0).0, score);
        }
        let mut tunnel = Tunnel::default();
        tunnel.roll.position = 128 * UNIT;
        for _ in 0..3 {
            assert!(tunnel.step(0, 0).1.is_none());
        }
        assert!(matches!(tunnel.step(0, 0).1, Some(Phase::Failure(183))));
    }

    #[test]
    fn all_courses_finish_after_1024_segments_with_delayed_track_samples() {
        assert_eq!(ORDER, [0, 6, 2, 1, 3, 4, 5, 7]);
        for course in 0..8 {
            let mut tunnel = Tunnel::default();
            for segment in 0..1024 {
                tunnel.roll.position = i32::from(tunnel.target()) * UNIT;
                tunnel.roll.velocity = 0;
                let (_, phase) = tunnel.step(0, course);
                assert!(tunnel.on_track);
                assert_eq!(phase.is_some(), segment == 1023);
                if let Some(phase) = phase {
                    assert!(matches!(phase, Phase::Success(1)));
                }
            }
            assert_eq!(tunnel.tail, 31 * UNIT + 0x8000);
        }
    }
}
