//! Local difficulty settings, score rankings, and player Keys with separate
//! starting-web checkpoints for each difficulty.

use crate::difficulty::Difficulty;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::{io::Write, path::PathBuf};

pub(crate) const LIMIT: usize = 10;
pub(crate) const KEY_LIMIT: usize = 6;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) initials: String,
    pub(crate) score: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PlayerKey {
    pub(crate) initials: String,
    pub(crate) levels: [u8; 3],
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Data {
    version: u8,
    pub(crate) difficulty: Difficulty,
    pub(crate) initials: String,
    pub(crate) tables: [Vec<Entry>; 3],
    pub(crate) keys: Vec<PlayerKey>,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: 2,
            difficulty: Difficulty::Normal,
            initials: "AAA".into(),
            tables: default(),
            keys: Vec::new(),
        }
    }
}

impl Data {
    fn parse(text: &str) -> Result<Self, String> {
        let mut data: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if !matches!(data.version, 1 | 2)
            || !valid_initials(&data.initials)
            || data
                .tables
                .iter()
                .flatten()
                .any(|entry| !valid_initials(&entry.initials))
            || data.keys.len() > KEY_LIMIT
            || data.keys.iter().enumerate().any(|(index, key)| {
                !valid_initials(&key.initials)
                    || key.levels == [0; 3]
                    || key
                        .levels
                        .iter()
                        .any(|&level| level != 0 && !valid_key_level(level))
                    || data.keys[..index]
                        .iter()
                        .any(|other| other.initials == key.initials)
            })
        {
            return Err("Unsupported or invalid player data".into());
        }
        data.version = 2;
        for table in &mut data.tables {
            table.sort_by_key(|entry| std::cmp::Reverse(entry.score));
            table.truncate(LIMIT);
        }
        Ok(data)
    }

    pub(crate) fn rank(&self, difficulty: Difficulty, score: u32) -> Option<usize> {
        if score == 0 {
            return None;
        }
        let table = &self.tables[difficulty.index()];
        let rank = table
            .iter()
            .position(|entry| score > entry.score)
            .unwrap_or(table.len());
        (rank < LIMIT).then_some(rank)
    }

    pub(crate) fn insert(
        &mut self,
        difficulty: Difficulty,
        score: u32,
        initials: String,
    ) -> Option<usize> {
        let rank = self.rank(difficulty, score)?;
        self.initials.clone_from(&initials);
        let table = &mut self.tables[difficulty.index()];
        table.insert(rank, Entry { initials, score });
        table.truncate(LIMIT);
        Some(rank)
    }

    pub(crate) fn key_qualifies(&self, difficulty: Difficulty, level: u8) -> bool {
        valid_key_level(level)
            && (self.keys.len() < KEY_LIMIT
                || self
                    .keys
                    .iter()
                    .any(|key| key.levels[difficulty.index()] < level))
    }

    pub(crate) fn save_key(&mut self, difficulty: Difficulty, level: u8, initials: &str) -> bool {
        if !valid_key_level(level) {
            return false;
        }
        let slot = difficulty.index();
        if let Some(key) = self.keys.iter_mut().find(|key| key.initials == initials) {
            if key.levels[slot] >= level {
                return false;
            }
            key.levels[slot] = level;
        } else {
            let mut key = PlayerKey {
                initials: initials.into(),
                levels: [0; 3],
            };
            key.levels[slot] = level;
            if self.keys.len() < KEY_LIMIT {
                self.keys.push(key);
            } else {
                let (index, lowest) = self
                    .keys
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, key)| key.levels[slot])
                    .unwrap();
                if lowest.levels[slot] >= level {
                    return false;
                }
                self.keys[index] = key;
            }
        }
        self.initials = initials.into();
        true
    }
}

#[derive(Resource)]
pub(crate) struct Store {
    pub(crate) data: Data,
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<PathBuf>,
    #[cfg(target_arch = "wasm32")]
    writable: bool,
    pub(crate) notice: String,
}

impl Store {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn load() -> Self {
        Self::open(dirs::data_local_dir().map(|path| path.join("tempest2032/player.toml")))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open(path: Option<PathBuf>) -> Self {
        let mut store = Self {
            data: default(),
            path,
            notice: String::new(),
        };
        let result = (|| {
            let path = store
                .path
                .as_ref()
                .ok_or("Could not locate the application-data directory".to_string())?;
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Data::default());
                }
                Err(error) => return Err(error.to_string()),
            };
            Data::parse(&text)
        })();
        match result {
            Ok(data) => store.data = data,
            Err(error) => {
                store.notice = format!(
                    "Could not load player data: {error}. Saving is disabled for this session."
                );
                store.path = None;
            }
        }
        store
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn save(&mut self) {
        let Some(path) = &self.path else { return };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let directory = path.parent().unwrap();
            std::fs::create_dir_all(directory)?;
            let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
            temporary.write_all(toml::to_string(&self.data)?.as_bytes())?;
            temporary.as_file().sync_all()?;
            temporary.persist(path)?;
            Ok(())
        })();
        self.notice = result.err().map_or_else(String::new, |error| {
            format!("Could not save player data: {error}. Scores and Keys remain available until you quit.")
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn load() -> Self {
        let result = (|| match storage()?
            .get_item("tempest2032/player.toml")
            .map_err(|error| format!("{error:?}"))?
        {
            Some(text) => Data::parse(&text),
            None => Ok(Data::default()),
        })();
        match result {
            Ok(data) => Self {
                data,
                writable: true,
                notice: String::new(),
            },
            Err(error) => Self {
                data: default(),
                writable: false,
                notice: format!(
                    "Could not load player data: {error}. Saving is disabled for this session."
                ),
            },
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn save(&mut self) {
        if !self.writable {
            return;
        }
        let result = (|| {
            storage()?
                .set_item(
                    "tempest2032/player.toml",
                    &toml::to_string(&self.data).map_err(|error| error.to_string())?,
                )
                .map_err(|error| format!("{error:?}"))
        })();
        self.notice = result.err().map_or_else(String::new, |error| {
            format!(
                "Could not save player data: {error}. Scores and Keys remain available until you reload."
            )
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .unwrap()
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "Browser storage is unavailable".into())
}

fn valid_initials(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|letter| letter.is_ascii_uppercase())
}

fn valid_key_level(level: u8) -> bool {
    (17..=97).contains(&level) && level % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rankings_are_separate_stable_on_ties_and_bounded() {
        let mut data = Data::default();
        assert_eq!(data.rank(Difficulty::Normal, 0), None);
        for score in (1..=10).rev() {
            data.insert(Difficulty::Normal, score, "AAA".into());
        }
        assert_eq!(data.rank(Difficulty::Normal, 1), None);
        assert_eq!(data.insert(Difficulty::Normal, 10, "BBB".into()), Some(1));
        assert_eq!(data.tables[1][0].initials, "AAA");
        assert_eq!(data.tables[1].len(), LIMIT);
        assert_eq!(data.rank(Difficulty::Hard, 1), Some(0));
    }

    #[test]
    fn six_player_keys_keep_independent_difficulty_progress_and_never_regress() {
        let mut data = Data::default();
        for (initials, level) in [
            ("AAA", 17),
            ("BBB", 21),
            ("CCC", 25),
            ("DDD", 31),
            ("EEE", 41),
            ("FFF", 51),
        ] {
            assert!(data.save_key(Difficulty::Normal, level, initials));
        }
        assert_eq!(data.keys.len(), 6);
        assert!(data.save_key(Difficulty::Easy, 61, "AAA"));
        assert!(data.save_key(Difficulty::Hard, 19, "AAA"));
        assert!(!data.save_key(Difficulty::Normal, 17, "AAA"));
        assert!(!data.save_key(Difficulty::Hard, 17, "AAA"));
        assert_eq!(data.keys[0].levels, [61, 17, 19]);
        assert!(
            data.keys[1..]
                .iter()
                .all(|key| key.levels[0] == 0 && key.levels[2] == 0)
        );
        assert_eq!(data.keys.len(), KEY_LIMIT);
        assert_eq!(data.initials, "AAA");
    }

    #[test]
    fn full_key_table_replaces_only_a_lower_checkpoint_in_the_selected_difficulty() {
        let mut data = Data::default();
        for initials in ["AAA", "BBB", "CCC", "DDD", "EEE", "FFF"] {
            data.save_key(Difficulty::Normal, 31, initials);
        }
        data.save_key(Difficulty::Normal, 21, "GGG");
        assert!(!data.key_qualifies(Difficulty::Normal, 31));
        assert_eq!(data.keys[0].initials, "AAA");
        assert!(data.save_key(Difficulty::Normal, 33, "GGG"));
        assert_eq!(data.keys.len(), KEY_LIMIT);
        assert_eq!(data.keys[0].initials, "GGG");
        assert_eq!(data.keys[0].levels, [0, 33, 0]);
        assert!(data.key_qualifies(Difficulty::Hard, 17));
        assert!(data.save_key(Difficulty::Hard, 17, "HHH"));
        assert_eq!(data.keys.len(), KEY_LIMIT);
    }

    #[test]
    fn player_data_migrates_old_scores_and_validates_key_slots() {
        let old = "version = 1\ndifficulty = 'high'\ninitials = 'XYZ'\ntables = [[], [{initials = 'ABC', score = 12345}], []]";
        let mut data = Data::parse(old).unwrap();
        assert_eq!(data.version, 2);
        assert_eq!(data.difficulty, Difficulty::Hard);
        assert_eq!(data.tables[1][0].score, 12345);
        assert!(data.keys.is_empty());
        data.save_key(Difficulty::Easy, 97, "XYZ");
        data.save_key(Difficulty::Normal, 31, "XYZ");
        let encoded = toml::to_string(&data).unwrap();
        assert_eq!(Data::parse(&encoded).unwrap().keys[0].levels, [97, 31, 0]);
        for levels in [[0, 0, 0], [15, 0, 0], [18, 0, 0], [99, 0, 0]] {
            data.keys[0].levels = levels;
            assert!(Data::parse(&toml::to_string(&data).unwrap()).is_err());
        }
        data.keys[0].levels = [17, 0, 0];
        data.keys.push(data.keys[0].clone());
        assert!(Data::parse(&toml::to_string(&data).unwrap()).is_err());
        data.keys.clear();
        for initials in ["AAA", "BBB", "CCC", "DDD", "EEE", "FFF", "GGG"] {
            data.keys.push(PlayerKey {
                initials: initials.into(),
                levels: [0, 17, 0],
            });
        }
        assert!(Data::parse(&toml::to_string(&data).unwrap()).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn round_trip_and_invalid_files_are_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("player.toml");
        let mut store = Store::open(Some(path.clone()));
        store.data.difficulty = Difficulty::Hard;
        store.data.insert(Difficulty::Easy, 123, "XYZ".into());
        store.data.save_key(Difficulty::Hard, 31, "XYZ");
        store.save();
        assert!(store.notice.is_empty());
        let loaded = Store::open(Some(path.clone()));
        assert_eq!(loaded.data.difficulty, Difficulty::Hard);
        assert_eq!(loaded.data.initials, "XYZ");
        assert_eq!(loaded.data.tables[0][0].score, 123);
        assert_eq!(loaded.data.keys[0].levels, [0, 0, 31]);
        std::fs::write(&path, "invalid [").unwrap();
        let mut broken = Store::open(Some(path.clone()));
        assert!(!broken.notice.is_empty());
        broken.save();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid [");
    }
}
