//! Embedded MOD music and per-level files or background-loaded cached URLs, independent of effect pitch.

use crate::{
    game::{Game, Status},
    level::{Level, Soundtrack},
    warp::ZoneKind,
};
use bevy::{
    audio::{AddAudioSource, Source, Volume},
    prelude::*,
};
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
    time::Duration,
};
use xmrs::prelude::Module;
use xmrsplayer::prelude::XmrsPlayer;

const RATE: u32 = 44_100;
const DATA: [&[u8]; 7] = [
    include_bytes!("../assets/music/tune13.mod"),
    include_bytes!("../assets/music/tune7.mod"),
    include_bytes!("../assets/music/tune1.mod"),
    include_bytes!("../assets/music/tune3.mod"),
    include_bytes!("../assets/music/rave4.mod"),
    include_bytes!("../assets/music/tune5.mod"),
    include_bytes!("../assets/music/tune12.mod"),
];
static MODULES: LazyLock<[Arc<Module>; 7]> = LazyLock::new(|| {
    DATA.map(|data| Arc::new(Module::load_mod(data).expect("invalid embedded music module")))
});

#[derive(Resource)]
pub(crate) struct MusicVolume(pub(crate) f32);

impl Default for MusicVolume {
    fn default() -> Self {
        Self(0.35)
    }
}

#[derive(Asset, TypePath)]
pub(crate) struct Track(Arc<Module>);

self_cell::self_cell! {
    struct PlayerCell {
        owner: Arc<Module>,
        #[not_covariant]
        dependent: XmrsPlayer,
    }
}

pub(crate) struct Decoder(PlayerCell);

impl Iterator for Decoder {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        self.0.with_dependent_mut(|_, player| {
            player.next().or_else(|| {
                player.goto(0, 0, 0);
                player.next()
            })
        })
    }
}

impl Source for Decoder {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for Track {
    type DecoderItem = i16;
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder(PlayerCell::new(self.0.clone(), |module| {
            XmrsPlayer::new(module, RATE, 0)
        }))
    }
}

#[derive(Resource)]
struct Catalog {
    original: [Handle<Track>; 7],
    custom: HashMap<Soundtrack, Selection>,
    #[cfg(not(target_arch = "wasm32"))]
    downloads: Option<std::sync::Mutex<std::sync::mpsc::Receiver<soundtrack::Download>>>,
}

#[derive(Clone, Debug, PartialEq)]
enum Selection {
    Original(usize),
    #[cfg(not(target_arch = "wasm32"))]
    Custom(Handle<AudioSource>),
    #[cfg(not(target_arch = "wasm32"))]
    Module(Handle<Track>),
}

#[derive(Component)]
struct Playing {
    track: Selection,
    fade: f32,
}

fn selected(game: &Game, level: &Level) -> usize {
    if game.status == Status::GameOver {
        return 0;
    }
    if let Some(bonus) = &game.bonus {
        return if bonus.kind == ZoneKind::Tunnel { 3 } else { 2 };
    }
    // yak.s webtunes uses zero-based cwave >> 4 and one-based module numbers.
    [4, 1, 5, 6, 4, 1, 5, 6][(level.number.saturating_sub(1) / 16) & 7]
}

pub(crate) fn add_systems(app: &mut App) {
    // The browser's audio plugin does not register audio sources, which setup needs.
    app.add_audio_source::<Track>()
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<MusicVolume>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            update
                .after(crate::game::load_level)
                .run_if(crate::pause::running),
        );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, receive.before(update));
}

fn setup(
    mut commands: Commands,
    mut assets: ResMut<Assets<Track>>,
    levels: Res<crate::level::Levels>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    LazyLock::force(&MODULES);
    #[cfg(not(target_arch = "wasm32"))]
    let (custom, downloads) = custom_tracks(&levels, &mut audio, &mut assets);
    #[cfg(target_arch = "wasm32")]
    let _ = (&levels, &mut audio);
    commands.insert_resource(Catalog {
        original: std::array::from_fn(|index| assets.add(Track(MODULES[index].clone()))),
        #[cfg(not(target_arch = "wasm32"))]
        custom,
        #[cfg(target_arch = "wasm32")]
        custom: HashMap::new(),
        #[cfg(not(target_arch = "wasm32"))]
        downloads,
    });
}

#[cfg(not(target_arch = "wasm32"))]
mod soundtrack;

#[cfg(not(target_arch = "wasm32"))]
fn select(
    path: &Soundtrack,
    result: Result<soundtrack::Loaded, String>,
    audio: &mut Assets<AudioSource>,
    modules: &mut Assets<Track>,
) -> Option<Selection> {
    match result {
        Ok(soundtrack::Loaded::Audio(source)) => Some(Selection::Custom(audio.add(source))),
        Ok(soundtrack::Loaded::Module(module)) => {
            Some(Selection::Module(modules.add(Track(module))))
        }
        Err(error) => {
            warn!("Could not load soundtrack {path:?}: {error}; using original music");
            None
        }
    }
}

/// Loads files now and downloads URLs on a thread, so a slow server cannot
/// delay startup; their levels play the original music until they arrive.
#[cfg(not(target_arch = "wasm32"))]
fn custom_tracks(
    levels: &crate::level::Levels,
    audio: &mut Assets<AudioSource>,
    modules: &mut Assets<Track>,
) -> (
    HashMap<Soundtrack, Selection>,
    Option<std::sync::Mutex<std::sync::mpsc::Receiver<soundtrack::Download>>>,
) {
    let cache = dirs::cache_dir().map(|path| path.join("tempest2032/soundtracks"));
    let (urls, files): (Vec<_>, Vec<_>) = levels
        .0
        .iter()
        .filter_map(|level| level.soundtrack.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .partition(|path| matches!(path, Soundtrack::Url(_)));
    let tracks = files
        .into_iter()
        .filter_map(|path| {
            let result = soundtrack::load(&path, cache.as_deref());
            select(&path, result, audio, modules).map(|selection| (path, selection))
        })
        .collect();
    if urls.is_empty() {
        return (tracks, None);
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for path in urls {
            let result = soundtrack::load(&path, cache.as_deref());
            if sender.send((path, result)).is_err() {
                break;
            }
        }
    });
    (tracks, Some(std::sync::Mutex::new(receiver)))
}

#[cfg(not(target_arch = "wasm32"))]
fn receive(
    mut catalog: ResMut<Catalog>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut modules: ResMut<Assets<Track>>,
) {
    use std::sync::mpsc::TryRecvError;
    let Some(downloads) = &catalog.downloads else {
        return;
    };
    let received = downloads.lock().unwrap().try_recv();
    match received {
        Ok((path, result)) => {
            if let Some(selection) = select(&path, result, &mut audio, &mut modules) {
                catalog.custom.insert(path, selection);
            }
        }
        Err(TryRecvError::Disconnected) => catalog.downloads = None,
        Err(TryRecvError::Empty) => {}
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update(
    mut commands: Commands,
    game: Res<Game>,
    level: Res<Level>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    volume: Res<MusicVolume>,
    master: Res<GlobalVolume>,
    front: Option<Res<crate::menu::FrontEnd>>,
    mut playing: Query<(
        Entity,
        &mut Playing,
        &mut PlaybackSettings,
        Option<&mut AudioSink>,
    )>,
) {
    if volume.0 == 0. || master.volume.to_linear() == 0. {
        for (entity, _, _, _) in &mut playing {
            commands.entity(entity).despawn();
        }
        return;
    }
    // rreset requests modnum=1 at yak.s:1018; modtable[0] is tune13.mod.
    let track = if front.is_some_and(|front| front.screen != crate::menu::Screen::Gameplay) {
        Selection::Original(0)
    } else if game.status != Status::GameOver
        && game.bonus.is_none()
        && let Some(selection) = level
            .soundtrack
            .as_ref()
            .and_then(|path| catalog.custom.get(path))
    {
        selection.clone()
    } else {
        Selection::Original(selected(&game, &level))
    };
    for (entity, mut current, mut settings, sink) in &mut playing {
        current.fade = if current.track == track {
            0.
        } else {
            current.fade + time.delta_secs()
        };
        if current.fade < 1. {
            let gain = volume.0 * (1. - current.fade);
            settings.volume = Volume::Linear(gain);
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(gain * master.volume.to_linear()));
            }
            return;
        }
        commands.entity(entity).despawn();
    }
    let mut entity = commands.spawn(Playing {
        track: track.clone(),
        fade: 0.,
    });
    match track {
        Selection::Original(index) => {
            entity.insert((
                AudioPlayer(catalog.original[index].clone()),
                PlaybackSettings::ONCE.with_volume(Volume::Linear(volume.0)),
            ));
            info!("Playing original music track {}", index + 1);
        }
        #[cfg(not(target_arch = "wasm32"))]
        Selection::Custom(handle) => {
            entity.insert((
                AudioPlayer(handle),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(volume.0)),
            ));
            info!("Playing custom level soundtrack");
        }
        #[cfg(not(target_arch = "wasm32"))]
        Selection::Module(handle) => {
            entity.insert((
                AudioPlayer(handle),
                PlaybackSettings::ONCE.with_volume(Volume::Linear(volume.0)),
            ));
            info!("Playing custom tracker soundtrack");
        }
    }
}

#[cfg(test)]
#[path = "tests/music.rs"]
mod tests;
