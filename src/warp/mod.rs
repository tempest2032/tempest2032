//! Warp-zone selection, entry/return transactions, and fixed-tick bonus gameplay.

use crate::{
    game::{Game, Status},
    level::{Levels, LoadReason},
    sines::sine,
};
use bevy::prelude::*;
use clap::ValueEnum;
use std::collections::VecDeque;

mod courses;
pub(crate) mod failure;
pub(crate) mod render;
mod rings;
mod tunnel;
pub(crate) mod victory;

const UNIT: i32 = 1 << 16;
pub(crate) const SCALE: f32 = 0.1;
pub(crate) const GATES: usize = 53;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ZoneKind {
    #[value(alias = "gate", alias = "1")]
    GateFlight,
    #[value(alias = "green-track", alias = "2")]
    Tunnel,
    #[value(alias = "psychedelic-rings", alias = "3")]
    Rings,
}

impl ZoneKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::GateFlight => "GATE FLIGHT",
            Self::Tunnel => "GREEN TRACK",
            Self::Rings => "PSYCHEDELIC RINGS",
        }
    }
    pub(crate) fn index(self) -> usize {
        match self {
            Self::GateFlight => 0,
            Self::Tunnel => 1,
            Self::Rings => 2,
        }
    }
    pub(crate) fn for_web(number: usize) -> Self {
        match ((number - 1) / 16) & 3 {
            0 => Self::GateFlight,
            1 | 3 => Self::Tunnel,
            _ => Self::Rings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Flying,
    Failure(u16),
    Success(u16),
    Victory(u16),
    Fade(u16),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gate {
    pub(crate) index: usize,
    pub(crate) distance: i32,
    pub(crate) age: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Axis {
    pub(crate) position: i32,
    pub(crate) velocity: i32,
}

impl Axis {
    fn step(
        &mut self,
        direction: i32,
        acceleration: i32,
        friction: i32,
        max_velocity: i32,
        min: i32,
        max: i32,
    ) {
        if direction == 0 {
            if self.velocity.abs() < friction {
                self.velocity = 0;
                return;
            }
            self.velocity -= self.velocity.signum() * friction;
        } else {
            if min != max
                && ((direction < 0 && self.position < min)
                    || (direction > 0 && self.position >= max))
            {
                self.velocity = 0;
                return;
            }
            self.velocity =
                (self.velocity + direction * acceleration).clamp(-max_velocity, max_velocity);
        }
        self.position += self.velocity;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Session {
    pub(crate) kind: ZoneKind,
    pub(crate) course: usize,
    pub(crate) destination: usize,
    pub(crate) phase: Phase,
    pub(crate) x: Axis,
    pub(crate) y: Axis,
    pub(crate) speed: i32,
    pub(crate) distance: i64,
    pub(crate) gates: VecDeque<Gate>,
    pub(crate) passed: usize,
    pub(crate) tick: u32,
    pub(crate) warp_five: bool,
    pub(crate) tunnel: Option<tunnel::Tunnel>,
    pub(crate) rings: Option<rings::Rings>,
    announcements: Vec<crate::messages::Event>,
    cursor: usize,
    generation_distance: i32,
    generation_wait: i16,
}

impl Session {
    pub(crate) fn new(
        kind: ZoneKind,
        attempt: u32,
        course: Option<u8>,
        destination: usize,
    ) -> Self {
        Self {
            kind,
            course: course.map_or(
                if kind == ZoneKind::Tunnel {
                    tunnel::ORDER[attempt as usize & 7]
                } else {
                    courses::ORDER[attempt as usize & 7]
                },
                |n| usize::from(n - 1),
            ),
            destination,
            phase: Phase::Flying,
            x: Axis {
                position: 160 * UNIT,
                velocity: 0,
            },
            y: Axis {
                position: 8 * UNIT,
                velocity: 0x80000,
            },
            speed: (if kind == ZoneKind::Rings {
                0xa000
            } else {
                0x8000
            }) + ((attempt.wrapping_mul(0x200) & 0xffff) as i32),
            distance: 0,
            gates: VecDeque::new(),
            passed: 0,
            tick: 0,
            warp_five: false,
            tunnel: (kind == ZoneKind::Tunnel).then(tunnel::Tunnel::default),
            rings: (kind == ZoneKind::Rings).then(rings::Rings::default),
            announcements: Vec::new(),
            cursor: 0,
            generation_distance: 0,
            generation_wait: 0,
        }
    }
    pub(crate) fn start_at_gate(&mut self, gate: u8) {
        let index = usize::from(gate - 1);
        for previous in 0..index {
            if self.spec(previous).2 == 3 {
                self.speed += self.speed >> 3;
            }
            if let Some(rings) = &mut self.rings {
                rings.pass();
            }
        }
        self.cursor = index;
        self.passed = index;
        let (x, y, _) = self.spec(index);
        self.x = Axis {
            position: (160 + i32::from(x)) * UNIT,
            velocity: 0,
        };
        self.y = Axis {
            position: i32::from(y) * 8 * UNIT,
            velocity: 0,
        };
    }

    pub(crate) fn position(&self) -> Vec2 {
        Vec2::new(
            (self.x.position - 160 * UNIT) as f32 / UNIT as f32,
            self.y.position as f32 / (8 * UNIT) as f32,
        )
    }
    pub(crate) fn spec(&self, index: usize) -> (i8, i8, u8) {
        let record = courses::COURSES[self.course][index];
        (
            if self.kind == ZoneKind::Rings {
                -((record >> 16) as u8 as i8)
            } else {
                (record >> 16) as u8 as i8
            },
            (record >> 8) as u8 as i8,
            record as u8,
        )
    }
    fn pass(&mut self, index: usize) -> Option<u32> {
        let (x, y, kind) = self.spec(index);
        let dx = ((self.x.position - 160 * UNIT) >> 16) - i32::from(x);
        let dy = (self.y.position >> 19) - i32::from(y);
        if dx.abs() >= 8 || dy.abs() >= 8 {
            self.phase = Phase::Failure(151);
            return None;
        }
        let error = dx.abs() + dy.abs();
        self.passed += 1;
        match kind {
            1 => self.y.velocity = -0x80000,
            2 => self.y.velocity = 0x80000,
            3 => {
                if self.kind == ZoneKind::GateFlight {
                    self.announcements
                        .push(crate::messages::Id::SpeedBoost.into());
                }
                self.speed += self.speed >> 3;
            }
            4 => self.phase = Phase::Success(151),
            _ => {}
        }
        if let Some(rings) = &mut self.rings {
            rings.pass();
            return Some([250, 500, 750, 1][(error >> 2) as usize]);
        }
        let score = if error < 4 {
            750
        } else if error < 8 {
            500
        } else {
            250
        };
        let id = match score {
            750 => crate::messages::Id::Points750,
            500 => crate::messages::Id::Points500,
            _ => crate::messages::Id::Points250,
        };
        self.announcements.push(id.into());
        Some(score)
    }
    /// Steers at the original rates and moves forward by `travel`, the session
    /// speed at the difficulty's tempo.
    fn step_flight(&mut self, horizontal: i32, vertical: i32, travel: i32) -> u32 {
        self.x
            .step(horizontal, 0x1000, 0x800, 0x20000, 0, 320 * UNIT);
        self.y
            .step(vertical, 0x8000, 0x2000, 0x80000, -320 * UNIT, 320 * UNIT);
        self.distance += i64::from(travel);
        for gate in &mut self.gates {
            gate.distance -= travel;
            gate.age = gate.age.wrapping_add(1);
        }
        let mut points = 0;
        while self.gates.front().is_some_and(|g| g.distance >> 16 <= 2) {
            let gate = self.gates.pop_front().unwrap();
            if let Some(score) = self.pass(gate.index) {
                points += score;
            }
            if !matches!(self.phase, Phase::Flying) {
                return points;
            }
        }
        self.generation_distance -= travel;
        if self.generation_distance < 0 {
            self.generation_distance += 12 * UNIT;
            self.generation_wait -= 1;
            if self.generation_wait < 0 && self.cursor < GATES {
                self.generation_wait = if self.kind == ZoneKind::Rings {
                    8
                } else {
                    (courses::COURSES[self.course][self.cursor] >> 24) as i16
                };
                self.gates.push_back(Gate {
                    index: self.cursor,
                    distance: 255 * UNIT,
                    age: 0,
                });
                self.cursor += 1;
            }
        }
        points
    }
}

pub(crate) fn advance(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    levels: Res<Levels>,
    mut game: ResMut<Game>,
    clock: Res<crate::difficulty::EnemyClock>,
) {
    if game.status != Status::Bonus || game.load.is_some() {
        return;
    }
    let Some(mut session) = game.bonus.take() else {
        return;
    };
    let was_flying = matches!(session.phase, Phase::Flying);
    let passed = session.passed;
    session.tick = session.tick.wrapping_add(1);
    if let Some(tunnel) = &mut session.tunnel {
        tunnel.settle(session.phase);
    }
    match session.phase {
        Phase::Flying => {
            let horizontal = i32::from(keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]))
                - i32::from(keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]));
            let vertical = i32::from(keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]))
                - i32::from(keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]));
            if let Some(tunnel) = &mut session.tunnel {
                // The track advances a whole segment per step, with the roll inside it.
                for _ in 0..clock.steps() {
                    let (score, phase) = tunnel.step(horizontal, session.course);
                    game.add_score(score);
                    if let Some(phase) = phase {
                        session.phase = phase;
                        break;
                    }
                }
            } else {
                let travel = clock.scaled(session.speed);
                game.add_score(session.step_flight(horizontal, vertical, travel));
            }
        }
        Phase::Victory(1) => {
            session.phase = Phase::Fade(151);
        }
        Phase::Failure(1) | Phase::Fade(1) => {
            game.load = Some(LoadReason::ReturnWarp {
                destination: session.destination,
            });
        }
        Phase::Failure(ticks) => session.phase = Phase::Failure(ticks - 1),
        Phase::Success(1) => {
            crate::sound::play(&mut commands, crate::sound::Effect::Excellent);
            let tunnel = session.kind == ZoneKind::Tunnel;
            session.warp_five = levels.0[session.destination].number < 91;
            if session.warp_five {
                game.announce(crate::messages::Id::WarpFive);
            }
            if !tunnel {
                game.add_score(20_000);
            }
            if levels.0[session.destination].number < 91 {
                session.destination = (session.destination + 4) % levels.0.len();
            }
            session.phase = Phase::Victory(if tunnel { 301 } else { 401 });
        }
        Phase::Success(ticks) => session.phase = Phase::Success(ticks - 1),
        Phase::Victory(ticks) => session.phase = Phase::Victory(ticks - 1),
        Phase::Fade(ticks) => session.phase = Phase::Fade(ticks - 1),
    }
    for index in passed..session.passed {
        let boosts = (0..=index)
            .filter(|&gate| session.spec(gate).2 == 3)
            .count();
        crate::sound::pitched(
            &mut commands,
            if boosts.is_multiple_of(2) {
                crate::sound::Effect::YesTwo
            } else {
                crate::sound::Effect::YesOne
            },
            512 + boosts as u16 * 5,
        );
    }
    if was_flying && session.kind == ZoneKind::Rings && matches!(session.phase, Phase::Failure(_)) {
        crate::sound::play(&mut commands, crate::sound::Effect::LargeExplosion);
    }
    if let Some(mut rings) = session.rings.take() {
        let position = session.position();
        let specs: Vec<_> = session
            .gates
            .iter()
            .map(|gate| session.spec(gate.index))
            .collect();
        rings.advance(
            session.tick,
            position,
            &session.gates,
            &specs,
            session.phase,
        );
        session.rings = Some(rings);
    }
    game.announcements.append(&mut session.announcements);
    game.bonus = Some(session);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
