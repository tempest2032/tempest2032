//! Original track decoding, uninterrupted same-track playback, and transitions.

use super::*;

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<Level>()
        .init_resource::<Time>()
        .init_resource::<MusicVolume>()
        .init_resource::<Assets<Track>>()
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<crate::level::Levels>()
        .insert_resource(GlobalVolume::new(Volume::Linear(0.6)))
        .add_systems(Startup, setup)
        .add_systems(Update, update);
    app
}

fn current(app: &mut App) -> (Entity, Selection) {
    let mut query = app.world_mut().query::<(Entity, &Playing)>();
    let (entity, playing) = query.single(app.world()).unwrap();
    (entity, playing.track.clone())
}

#[test]
fn all_seven_original_modules_generate_stereo_music_at_normal_speed() {
    for index in 0..7 {
        assert_eq!(MODULES[index].get_num_channels(), 4);
        let mut decoder = Track(MODULES[index].clone()).decoder();
        assert_eq!(decoder.channels(), 2);
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.total_duration(), None);
        let audio: Vec<_> = decoder.by_ref().take(RATE as usize * 2).collect();
        assert_eq!(audio.len(), RATE as usize * 2);
        assert!(audio.iter().any(|sample| sample.unsigned_abs() > 1000));
        assert!(
            audio
                .as_chunks::<2>()
                .0
                .iter()
                .any(|pair| pair[0] != pair[1])
        );
    }
}

#[test]
fn decoder_continues_after_song_end() {
    let mut decoder = Track(MODULES[4].clone()).decoder();
    decoder.0.with_dependent_mut(|_, player| {
        player.set_max_loop_count(1);
        assert!(player.seek_seconds(player.duration_seconds() - 0.02));
    });
    assert_eq!(
        decoder.by_ref().take(RATE as usize * 4).count(),
        RATE as usize * 4
    );
    // rave4's own pattern jump repeats from inside the song, not its intro.
    decoder.0.with_dependent(|_, player| {
        assert!(player.position_seconds() < player.duration_seconds() / 2.0);
    });
}

#[test]
fn selection_matches_the_original_web_groups_and_bonus_tunes() {
    let mut game = Game::default();
    let mut level = Level::default();
    for (number, track) in [
        (1, 4),
        (16, 4),
        (17, 1),
        (32, 1),
        (33, 5),
        (48, 5),
        (49, 6),
        (64, 6),
        (65, 4),
        (80, 4),
        (81, 1),
        (96, 1),
        (97, 5),
        (99, 5),
    ] {
        level.number = number;
        assert_eq!(selected(&game, &level), track);
    }
    game.status = Status::Bonus;
    for (kind, track) in [
        (ZoneKind::GateFlight, 2),
        (ZoneKind::Tunnel, 3),
        (ZoneKind::Rings, 2),
    ] {
        game.bonus = Some(crate::warp::Session::new(kind, 0, None, 0));
        assert_eq!(selected(&game, &level), track);
    }
    game.status = Status::GameOver;
    assert_eq!(selected(&game, &level), 0);
}

#[test]
fn same_track_is_preserved_and_new_tracks_replace_it_after_a_fade() {
    let mut app = app();
    app.update();
    let original = current(&mut app);
    assert_eq!(original.1, Selection::Original(4));
    app.world_mut().resource_mut::<Level>().number = 2;
    for _ in 0..100 {
        app.update();
    }
    assert_eq!(current(&mut app), original);
    app.world_mut().resource_mut::<Level>().number = 17;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(500));
    app.update();
    assert_eq!(current(&mut app), original);
    let settings = app.world().get::<PlaybackSettings>(original.0).unwrap();
    assert_eq!(settings.speed, 1.0);
    assert_eq!(settings.volume, Volume::Linear(0.175));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(500));
    app.update();
    let next = current(&mut app);
    assert_eq!(next.1, Selection::Original(1));
    assert_ne!(next.0, original.0);
    assert!(app.world().get_entity(original.0).is_err());
    app.world_mut().resource_mut::<MusicVolume>().0 = 0.0;
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&Playing>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn welcome_and_configuration_use_the_original_title_screen_module() {
    let mut app = app();
    app.insert_resource(crate::menu::FrontEnd::new(
        crate::menu::Screen::Welcome,
        true,
        Vec::new(),
    ));
    app.update();
    assert_eq!(current(&mut app).1, Selection::Original(0));
    app.world_mut()
        .resource_mut::<crate::menu::FrontEnd>()
        .screen = crate::menu::Screen::Configuration;
    app.update();
    assert_eq!(current(&mut app).1, Selection::Original(0));
}

pub(super) fn wav() -> Vec<u8> {
    let samples = [0_i16, 1000, -1000, 2000];
    let size = (samples.len() * 2) as u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes()); // PCM
    bytes.extend(1_u16.to_le_bytes()); // mono
    bytes.extend(8000_u32.to_le_bytes());
    bytes.extend(16000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for sample in samples {
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

#[test]
fn external_music_loops_and_stays_live_across_levels_using_the_same_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.wav");
    std::fs::write(&path, wav()).unwrap();
    let mut app = app();
    app.world_mut().resource_mut::<Level>().soundtrack = Some(Soundtrack::File(path.clone()));
    app.world_mut().resource_mut::<crate::level::Levels>().0[0].soundtrack =
        Some(Soundtrack::File(path));
    app.update();
    let original = current(&mut app);
    let Selection::Custom(handle) = &original.1 else {
        panic!("expected custom music")
    };
    let source = app
        .world()
        .resource::<Assets<AudioSource>>()
        .get(handle)
        .unwrap();
    assert_eq!(source.decoder().collect::<Vec<_>>(), [0, 1000, -1000, 2000]);
    assert!(matches!(
        app.world()
            .get::<PlaybackSettings>(original.0)
            .unwrap()
            .mode,
        bevy::audio::PlaybackMode::Loop
    ));
    app.world_mut().resource_mut::<Level>().number = 17;
    app.update();
    assert_eq!(current(&mut app), original);
    // Warp and game-over music must override the custom level track.
    app.world_mut().resource_mut::<Game>().bonus =
        Some(crate::warp::Session::new(ZoneKind::Tunnel, 0, None, 0));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    assert_eq!(current(&mut app).1, Selection::Original(3));
    app.world_mut().resource_mut::<Game>().status = Status::GameOver;
    app.update();
    assert_eq!(current(&mut app).1, Selection::Original(0));
}

#[test]
fn missing_or_invalid_music_uses_the_levels_embedded_track() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.mp3");
    std::fs::write(&invalid, b"not an audio file").unwrap();
    for path in [
        directory.path().join("missing.flac"),
        invalid,
        directory.path().to_owned(),
    ] {
        let mut app = app();
        app.world_mut().resource_mut::<Level>().soundtrack = Some(Soundtrack::File(path.clone()));
        app.world_mut().resource_mut::<Level>().number = 33;
        app.world_mut().resource_mut::<crate::level::Levels>().0[0].soundtrack =
            Some(Soundtrack::File(path));
        app.update();
        assert_eq!(current(&mut app).1, Selection::Original(5));
    }
}

#[test]
fn soundtrack_downloads_do_not_block_startup() {
    let mut app = app();
    let url = Soundtrack::Url("http://127.0.0.1:1/song.wav".into());
    app.world_mut().resource_mut::<Level>().soundtrack = Some(url.clone());
    app.world_mut().resource_mut::<crate::level::Levels>().0[0].soundtrack = Some(url);
    app.add_systems(Update, receive.before(update));
    app.update();
    assert_eq!(current(&mut app).1, Selection::Original(4));
    while app.world().resource::<Catalog>().downloads.is_some() {
        app.update();
    }
    assert!(app.world().resource::<Catalog>().custom.is_empty());
}

#[test]
fn music_registers_audio_sources_without_the_native_audio_plugin() {
    // The browser build's audio plugin leaves this to the game.
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    add_systems(&mut app);
    assert!(app.world().contains_resource::<Assets<AudioSource>>());
}
