//! A graded difficulty scale layered over level definitions.
//!
//! These multipliers never touch `levels/*.toml`. They are applied once to the
//! parsed catalog so the shipped source values stay intact, and `normal` uses
//! the configured values unchanged.
//!
//! This is deliberately not Beastly mode. The original's Beastly mode is a
//! separate, harsher mode: it halves the *player's* bullet speed, forces an
//! absolute seven-shot alien budget, and stops Flippers pausing between moves.
//! See docs/reference.md for the source details.

use bevy::prelude::*;
use clap::ValueEnum;

/// The multipliers applied to a level. `Normal` is the identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Scale {
    /// Multiplies the player's bullet step.
    pub(crate) bullet_speed: f32,
    /// Multiplies the maximum simultaneous alien shot budget.
    pub(crate) alien_shots: f32,
    /// Multiplies every enemy climb speed.
    pub(crate) climb_speed: f32,
    /// Multiplies every enemy spawn interval.
    pub(crate) spawn_interval: f32,
}

impl Scale {
    const IDENTITY: Self = Self {
        bullet_speed: 1.,
        alien_shots: 1.,
        climb_speed: 1.,
        spawn_interval: 1.,
    };
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
    /// Faster player bullets, fewer alien shots, slower and rarer enemies.
    #[serde(alias = "low")]
    #[value(alias = "low")]
    Easy,
    #[default]
    /// The original source values, unchanged.
    Normal,
    /// Slower player bullets, more alien shots, faster and denser enemies.
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

    pub(crate) fn scale(self) -> Scale {
        match self {
            Self::Normal => Scale::IDENTITY,
            Self::Easy => Scale {
                bullet_speed: 1.25,
                alien_shots: 0.5,
                climb_speed: 0.85,
                spawn_interval: 1.3,
            },
            Self::Hard => Scale {
                bullet_speed: 0.85,
                alien_shots: 2.5,
                climb_speed: 1.2,
                spawn_interval: 0.8,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Difficulty;

    #[test]
    fn normal_leaves_every_multiplier_at_one() {
        let scale = Difficulty::Normal.scale();
        assert_eq!(scale.bullet_speed, 1.);
        assert_eq!(scale.alien_shots, 1.);
        assert_eq!(scale.climb_speed, 1.);
        assert_eq!(scale.spawn_interval, 1.);
    }

    #[test]
    fn low_is_uniformly_more_permissive_than_high() {
        let low = Difficulty::Easy.scale();
        let high = Difficulty::Hard.scale();
        assert!(low.bullet_speed > high.bullet_speed);
        assert!(low.alien_shots < high.alien_shots);
        assert!(low.climb_speed < high.climb_speed);
        assert!(low.spawn_interval > high.spawn_interval);
    }
}
