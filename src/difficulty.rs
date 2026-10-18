//! Difficulty as an enemy tempo, plus how crowded a wave may get.
//!
//! The levels, the player, and the reward economy never change: every setting
//! keeps the same enemy budget, drops, points, lives, and player mechanics.
//! `Easy` and `Hard` run every enemy and warp-zone timing at a fifth slower or
//! faster through [`EnemyClock`], so the original relationships between those
//! timings hold, and allow one fewer or one more simultaneous enemy shot and
//! two fewer or more enemies at once. `Normal` is the original, unchanged.
//!
//! This is deliberately not Beastly mode. The original's Beastly mode is a
//! separate, harsher mode: it halves the *player's* bullet speed, forces an
//! absolute seven-shot alien budget, and stops Flippers pausing between moves.
//! See docs/reference.md for the source details.

use bevy::prelude::*;
use clap::ValueEnum;

/// Enemy time in fifths of a simulation tick. Integer arithmetic keeps every
/// setting deterministic and makes `Normal` the original tick for tick.
#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct EnemyClock {
    tempo: u64,
    before: u64,
    after: u64,
}

impl Default for EnemyClock {
    /// One `Normal` tick, so systems run unchanged without the clock system.
    fn default() -> Self {
        Self {
            tempo: 5,
            before: 0,
            after: 5,
        }
    }
}

impl EnemyClock {
    /// Enemy ticks elapsed during this simulation tick: zero, one, or two.
    pub(crate) fn steps(&self) -> u32 {
        self.scaled(1) as u32
    }

    /// Progress toward the next enemy tick, from 0 to 4 fifths. Always 0 on `Normal`.
    pub(crate) fn fifths_ahead(&self) -> i32 {
        (self.after % 5) as i32
    }

    /// This tick's share of `amount` per enemy tick, spread so that the total
    /// over time is exact. Speeds stay smooth instead of skipping ticks.
    pub(crate) fn scaled(&self, amount: i32) -> i32 {
        let at = |fifths: u64| (fifths as i64 * i64::from(amount)).div_euclid(5);
        (at(self.after) - at(self.before)) as i32
    }
}

/// Advances enemy time by one simulation tick at the selected tempo.
pub(crate) fn advance_enemy_clock(difficulty: Res<Difficulty>, mut clock: ResMut<EnemyClock>) {
    clock.tempo = difficulty.tempo_fifths();
    clock.before = clock.after;
    clock.after += clock.tempo;
}

/// Counts a timer down by this tick's enemy steps, original-style: a timer at
/// zero is due, and the caller reloads it.
pub(crate) fn due(timer: &mut u16, steps: u32) -> bool {
    for _ in 0..steps {
        if *timer == 0 {
            return true;
        }
        *timer -= 1;
    }
    false
}

#[derive(
    Resource,
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Difficulty {
    /// Enemies and warp zones at four-fifths tempo, with fewer at once and one shot fewer.
    #[serde(alias = "low")]
    #[value(alias = "low")]
    Easy,
    #[default]
    /// The original game, unchanged.
    Normal,
    /// Enemies and warp zones at six-fifths tempo, with more at once and one shot more.
    #[serde(alias = "high")]
    #[value(alias = "high")]
    Hard,
}

impl Difficulty {
    pub(crate) const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    pub(crate) fn index(self) -> usize {
        Self::ALL.iter().position(|value| *value == self).unwrap()
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Easy => "EASY",
            Self::Normal => "NORMAL",
            Self::Hard => "HARD",
        }
    }

    fn tempo_fifths(self) -> u64 {
        match self {
            Self::Easy => 4,
            Self::Normal => 5,
            Self::Hard => 6,
        }
    }

    /// Simultaneous enemy shots, from the level's original `ashots` budget.
    /// Hard stops at Beastly's eight.
    pub(crate) fn enemy_shots(self, original: usize) -> usize {
        match self {
            Self::Easy => original.saturating_sub(1).max(1),
            Self::Normal => original,
            Self::Hard => (original + 1).min(original.max(8)),
        }
    }

    /// Enemies that may be on the web at once, from the level's `max_active`.
    pub(crate) fn max_active(self, original: usize) -> usize {
        match self {
            Self::Easy => original.saturating_sub(2).max(1),
            Self::Normal => original,
            Self::Hard => original + 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tempo: Difficulty, ticks: usize, amount: i32) -> (u32, i32) {
        let mut app = App::new();
        app.insert_resource(tempo)
            .init_resource::<EnemyClock>()
            .add_systems(Update, advance_enemy_clock);
        let (mut steps, mut total) = (0, 0);
        for _ in 0..ticks {
            app.update();
            let clock = app.world().resource::<EnemyClock>();
            steps += clock.steps();
            total += clock.scaled(amount);
        }
        (steps, total)
    }

    #[test]
    fn normal_is_one_enemy_tick_per_simulation_tick() {
        let clock = EnemyClock::default();
        assert_eq!((clock.steps(), clock.scaled(0x8000)), (1, 0x8000));
        assert_eq!(run(Difficulty::Normal, 60, 0x8000), (60, 60 * 0x8000));
    }

    #[test]
    fn easy_and_hard_spread_a_fifth_less_or_more_enemy_time_exactly() {
        assert_eq!(run(Difficulty::Easy, 60, 0x8000), (48, 48 * 0x8000));
        assert_eq!(run(Difficulty::Hard, 60, 0x8000), (72, 72 * 0x8000));
        assert_eq!(run(Difficulty::Easy, 5, 4), (4, 16));
    }

    #[test]
    fn timers_fire_after_the_same_enemy_ticks_on_every_setting() {
        let mut timer = 2;
        assert!(!due(&mut timer, 1));
        assert!(!due(&mut timer, 1));
        assert!(due(&mut timer, 1));
        let mut timer = 1;
        assert!(!due(&mut timer, 0));
        assert!(due(&mut timer, 2));
    }

    #[test]
    fn pressure_moves_by_one_shot_and_two_enemies_within_beastly_limits() {
        assert_eq!(Difficulty::Easy.enemy_shots(1), 1);
        assert_eq!(Difficulty::Easy.enemy_shots(4), 3);
        assert_eq!(Difficulty::Hard.enemy_shots(4), 5);
        assert_eq!(Difficulty::Hard.enemy_shots(8), 8);
        assert_eq!(Difficulty::Easy.max_active(11), 9);
        assert_eq!(Difficulty::Hard.max_active(11), 13);
        for setting in Difficulty::ALL {
            assert!(setting.enemy_shots(1) >= 1);
        }
    }
}

#[cfg(test)]
#[path = "tests/tempo.rs"]
mod tempo;
