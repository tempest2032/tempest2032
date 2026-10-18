//! Decode actual effects and check event timing, bounded playback, and loop cleanup headlessly.

use super::*;
use crate::{
    game::Game,
    test_support::{firing_app, ticks},
};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Sounds>()
        .init_resource::<Assets<Sample>>()
        .init_resource::<Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<Time>()
        .insert_resource(GlobalVolume::new(Volume::Linear(0.6)))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_yes_pitch, update).chain());
    app
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut().query::<&T>().iter(app.world()).count()
}

#[test]
fn effects_and_voices_keep_original_rates_and_cleanup_deadlines() {
    let mut app = app();
    let effects = [Effect::Shot, Effect::Recharge];
    app.world_mut()
        .resource_mut::<Sounds>()
        .pending
        .extend(effects.map(|effect| Request {
            effect,
            period: None,
            exit_yes: false,
        }));
    app.update();
    let mut clips = app
        .world_mut()
        .query::<(&AudioPlayer<Sample>, &PlaybackSettings, &Voice)>();
    for (player, settings, voice) in clips.iter(app.world()) {
        let catalog = app.world().resource::<Catalog>();
        let effect = effects
            .iter()
            .find(|&&effect| player.0 == catalog.shots[effect as usize])
            .unwrap();
        assert_eq!(settings.speed, 1.0);
        let sample = &catalog.samples[*effect as usize];
        let expected =
            sample.decoder().total_duration().unwrap().as_secs_f64() / f64::from(settings.speed);
        assert!((voice.deadline - expected).abs() < 0.001);
    }
    assert_eq!(clips.iter(app.world()).count(), effects.len());
}

#[test]
fn effect_rates_and_durations_match_the_jaguar_dsp_not_amiga_periods() {
    assert_eq!(samples()[5].decoder().sample_rate(), 16_114);
    assert_eq!(samples()[22].decoder().sample_rate(), 19_302);
    assert_eq!(samples()[7].decoder().sample_rate(), 8_057);
    let recharge = samples()[22]
        .decoder()
        .total_duration()
        .unwrap()
        .as_secs_f64();
    assert!((recharge - 2.246).abs() < 0.001);
    assert!(playback_speed(512, 568) > 1.0);
    assert!(playback_speed(512, 428) < 1.0);
    assert_eq!(playback_speed(512, 512), 1.0);
    assert!(playback_speed(428, 20) > playback_speed(428, 10));
}

#[test]
fn spike_hits_use_the_original_sample_with_rising_pitch_as_the_spike_shrinks() {
    use crate::enemies::spikers::Spike;
    use crate::weapons::Bullet;
    use crate::web::{LANE_UNIT, WORLD_UNIT};

    let mut hits = crate::test_support::app();
    hits.init_resource::<Sounds>()
        .add_systems(FixedUpdate, crate::collisions::resolve_hits);
    let mut playback = app();
    playback.update();
    let sample = &playback.world().resource::<Catalog>().samples[Effect::Spike as usize];
    assert_eq!((sample.period, sample.data.len()), (254, 1038));
    let mut previous_frequency = 0.;
    for (length, pitch, expected_frequency) in [
        (150, 674, 1827.),
        (30, 554, 4666.),
        (3, 527, 5146.),
        (0, 524, 5203.),
    ] {
        let spike = hits
            .world_mut()
            .spawn(Spike {
                lane: 7,
                length: length * LANE_UNIT,
            })
            .id();
        hits.world_mut().spawn((
            Bullet { lane: 7 },
            Transform::from_xyz(0., 0., 0.06 - (160 - length) as f32 * WORLD_UNIT),
        ));
        ticks(&mut hits, 1);
        let requests = std::mem::take(&mut hits.world_mut().resource_mut::<Sounds>().pending);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].effect, Effect::Spike);
        assert_eq!(requests[0].period, Some(pitch));
        if length > 0 {
            assert_eq!(
                hits.world().get::<Spike>(spike).unwrap().length,
                (length - 3) * LANE_UNIT
            );
            hits.world_mut().despawn(spike);
        } else {
            assert!(hits.world().get_entity(spike).is_err());
        }
        playback.world_mut().resource_mut::<Sounds>().pending = requests;
        playback.update();
        let mut voices =
            playback
                .world_mut()
                .query::<(Entity, &AudioPlayer<Sample>, &PlaybackSettings, &Voice)>();
        let (entity, player, settings, voice) = voices.single(playback.world()).unwrap();
        assert_eq!(settings.speed, 1.);
        let clip = playback
            .world()
            .resource::<Assets<Sample>>()
            .get(&player.0)
            .unwrap();
        assert_eq!(clip.dsp_pitch, Some(pitch));
        let mut decoder = clip.decoder();
        let rate = decoder.sample_rate();
        assert_eq!(rate, 14_839);
        let duration = decoder.total_duration().unwrap();
        let pcm: Vec<_> = decoder.by_ref().collect();
        assert!((duration.as_secs_f64() - pcm.len() as f64 / f64::from(rate)).abs() < 1e-8);
        assert!(
            (voice.deadline - duration.as_secs_f64() / f64::from(settings.speed)).abs() < 0.001
        );
        let frequency = dominant_frequency(&pcm, rate) * f64::from(settings.speed);
        assert!(
            (frequency - expected_frequency).abs() < 50.,
            "length {length}: {frequency} Hz"
        );
        assert!(frequency > previous_frequency);
        previous_frequency = frequency;
        playback.world_mut().despawn(entity);
    }
}

fn dominant_frequency(pcm: &[f32], rate: u32) -> f64 {
    let windowed: Vec<_> = pcm
        .iter()
        .enumerate()
        .map(|(i, sample)| {
            f64::from(*sample)
                * (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (pcm.len() - 1) as f64).cos())
        })
        .collect();
    (1000..=7000)
        .step_by(5)
        .map(|frequency| {
            let (real, imaginary) =
                windowed
                    .iter()
                    .enumerate()
                    .fold((0., 0.), |(real, imaginary), (i, sample)| {
                        let phase =
                            std::f64::consts::TAU * frequency as f64 * i as f64 / f64::from(rate);
                        (
                            real + sample * phase.cos(),
                            imaginary + sample * phase.sin(),
                        )
                    });
            (frequency, real * real + imaginary * imaginary)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0 as f64
}

#[test]
fn all_effects_decode_as_finite_signed_pcm_at_their_original_periods() {
    for (index, sample) in samples().into_iter().enumerate() {
        if sample.data.is_empty() {
            assert!([3, 4].contains(&index));
            continue;
        }
        let mut decoder = sample.decoder();
        assert!(decoder.sample_rate() >= 4_000);
        assert!(decoder.total_duration().unwrap() > Duration::ZERO);
        for &byte in sample.data {
            assert_eq!(decoder.next(), Some(f32::from(byte as i8) / 128.));
        }
        assert_eq!(decoder.next(), None);
    }
    assert_eq!(samples()[5].data.len(), 1956);
    assert_eq!(samples()[26].data.len(), 11348);
}

#[test]
fn engine_and_yes_loops_play_the_intro_once_then_repeat_the_original_region() {
    for index in [0, 2, Effect::Yes as usize] {
        let mut sample = samples()[index].clone();
        sample.looping = true;
        let mut decoder = sample.decoder();
        assert_eq!(decoder.total_duration(), None);
        let end = sample.repeat + sample.repeat_length;
        let initial: Vec<_> = decoder.by_ref().take(end).collect();
        for _ in 0..8 {
            let repeat: Vec<_> = decoder.by_ref().take(sample.repeat_length).collect();
            assert_eq!(repeat, initial[sample.repeat..end]);
        }
    }
}

#[test]
fn yes_preview_loops_without_stacking_and_stops_when_closed() {
    for (id, looping) in [
        (crate::messages::Id::YesYesYes, true),
        (crate::messages::Id::WarpEnabled, false),
    ] {
        let mut app = app();
        app.insert_resource(crate::messages::Preview {
            id,
            capture_tick: None,
            elapsed: 0,
        });
        announcement(&mut app.world_mut().commands(), id);
        app.world_mut().flush();
        app.update();
        let (player, settings) = app
            .world_mut()
            .query::<(&AudioPlayer<Sample>, &PlaybackSettings)>()
            .single(app.world())
            .unwrap();
        let sample = app
            .world()
            .resource::<Assets<Sample>>()
            .get(&player.0)
            .unwrap();
        assert_eq!(sample.looping, looping);
        assert_eq!(sample.decoder().total_duration().is_none(), looping);
        assert_eq!(settings.speed, 1.);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1.5));
        app.update();
        assert_eq!(count::<AudioPlayer<Sample>>(&mut app), usize::from(looping));
        if looping {
            let settings = app
                .world_mut()
                .query_filtered::<&PlaybackSettings, With<PitchRise>>()
                .single(app.world())
                .unwrap();
            assert_eq!(
                settings.speed,
                rate_for_increment(333 + 90) as f32 / playback_rate(512) as f32
            );
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(10));
        app.update();
        assert_eq!(count::<AudioPlayer<Sample>>(&mut app), usize::from(looping));
        if looping {
            announcement(&mut app.world_mut().commands(), id);
            app.world_mut().flush();
            app.update();
            assert_eq!(count::<PitchRise>(&mut app), 1);
            reset(&mut app.world_mut().commands());
            app.world_mut().flush();
            app.update();
            assert_eq!(count::<PitchRise>(&mut app), 0);
            announcement(&mut app.world_mut().commands(), id);
            app.world_mut().flush();
            app.update();
            app.world_mut()
                .remove_resource::<crate::messages::Preview>();
            app.update();
            assert_eq!(count::<AudioPlayer<Sample>>(&mut app), 0);
        }
    }
}

#[test]
fn exit_yes_pitch_adds_one_dsp_increment_per_simulation_tick() {
    let mut app = app();
    let level = crate::level::Level::default();
    let mut exit = crate::animations::level_exit::Exit::new(
        level.web.glider_transform(level.web.start()).translation,
        &level.web,
    );
    exit.ticks = 73;
    app.world_mut().resource_mut::<Game>().exit = Some(exit);
    announcement(
        &mut app.world_mut().commands(),
        crate::messages::Id::YesYesYes,
    );
    app.world_mut().flush();
    app.update();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    let mut settings = app
        .world_mut()
        .query_filtered::<&PlaybackSettings, With<PitchRise>>();
    assert_eq!(settings.single(app.world()).unwrap().speed, 1.);
    app.world_mut()
        .resource_mut::<Game>()
        .exit
        .as_mut()
        .unwrap()
        .ticks += 60;
    app.update();
    let speed = rate_for_increment(333 + 60) as f32 / playback_rate(512) as f32;
    assert_eq!(settings.single(app.world()).unwrap().speed, speed);
    assert!(speed > 1.17 && speed < 1.19);
    for _ in 0..100 {
        app.update();
    }
    assert_eq!(settings.single(app.world()).unwrap().speed, speed);
    app.world_mut()
        .resource_mut::<Game>()
        .exit
        .as_mut()
        .unwrap()
        .ticks += 1;
    app.update();
    assert_eq!(
        settings.single(app.world()).unwrap().speed,
        rate_for_increment(394) as f32 / playback_rate(512) as f32
    );
    app.world_mut().resource_mut::<Game>().exit = None;
    app.update();
    assert_eq!(count::<PitchRise>(&mut app), 0);
}

#[test]
fn yes_replaces_engines_and_loops_until_the_flash_clears_effects_and_plays_warp() {
    use crate::animations::level_exit;
    let mut app = crate::test_support::app();
    app.init_resource::<Sounds>()
        .init_resource::<Assets<Sample>>()
        .init_resource::<Time>()
        .insert_resource(GlobalVolume::new(Volume::Linear(0.6)))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_yes_pitch, update).chain())
        .add_systems(
            FixedUpdate,
            level_exit::animate.before(crate::game::load_level),
        );
    let level = crate::level::Level::default();
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = crate::game::Status::Exiting;
    game.exit = Some(level_exit::Exit::new(
        level.web.glider_transform(level.web.start()).translation,
        &level.web,
    ));
    app.update();
    assert_eq!(count::<Engine>(&mut app), 2);
    announcement(
        &mut app.world_mut().commands(),
        crate::messages::Id::YesYesYes,
    );
    app.world_mut().flush();
    app.update();
    assert_eq!(count::<Engine>(&mut app), 0);
    for frame in 0..500 {
        if frame == 300 {
            app.world_mut()
                .resource_mut::<Sounds>()
                .pending
                .extend((0..MAX_VOICES).map(|_| Request {
                    effect: Effect::Recharge,
                    period: None,
                    exit_yes: false,
                }));
            app.update();
            assert_eq!(count::<Voice>(&mut app), MAX_VOICES);
        }
        ticks(&mut app, 1);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1. / 60.));
        app.update();
        if app.world().resource::<Game>().exit.unwrap().flash > 0 {
            assert!(
                frame > 216,
                "the voice must outlast the old three-repeat deadline"
            );
            break;
        }
        assert_eq!(count::<PitchRise>(&mut app), 1);
        assert_eq!(count::<Engine>(&mut app), 0);
    }
    assert!(app.world().resource::<Game>().exit.unwrap().flash > 0);
    assert_eq!(count::<PitchRise>(&mut app), 0);
    let player = app
        .world_mut()
        .query::<&AudioPlayer<Sample>>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        player.0,
        app.world().resource::<Catalog>().shots[Effect::Warp as usize]
    );
    for _ in 0..level_exit::FLASH_TICKS {
        ticks(&mut app, 1);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1. / 60.));
        app.update();
    }
    assert!(app.world().resource::<Game>().exit.is_none());
    assert!(!app.world().resource::<Sounds>().exit_yes);
    assert_eq!(count::<PitchRise>(&mut app), 0);
    assert_eq!(count::<Engine>(&mut app), 0);
}

#[test]
fn shots_emit_only_when_a_bullet_is_actually_fired() {
    let mut app = firing_app(0, &[KeyCode::Space]);
    app.init_resource::<Sounds>();
    ticks(&mut app, 1);
    assert_eq!(app.world().resource::<Sounds>().pending.len(), 1);
    assert_eq!(
        app.world().resource::<Sounds>().pending[0].effect,
        Effect::Shot
    );
    ticks(&mut app, 2);
    assert_eq!(app.world().resource::<Sounds>().pending.len(), 1);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Space);
    ticks(&mut app, 30);
    assert_eq!(app.world().resource::<Sounds>().pending.len(), 1);
}

#[test]
fn spike_hits_and_laser_shots_each_replace_their_single_voice_instead_of_stacking() {
    for (effect, period) in [(Effect::Spike, Some(554)), (Effect::Laser, None)] {
        let mut app = app();
        let request = Request {
            effect,
            period,
            exit_yes: false,
        };
        app.world_mut()
            .resource_mut::<Sounds>()
            .pending
            .push(request);
        app.update();
        let mut voices = app.world_mut().query::<(Entity, &Voice)>();
        let (first, _) = voices.single(app.world()).unwrap();
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<Sounds>()
                .pending
                .push(request);
            app.update();
        }
        let (last, voice) = voices.single(app.world()).unwrap();
        assert!(app.world().get_entity(first).is_err(), "{effect:?}");
        assert_ne!(first, last);
        assert_eq!(
            voice.slot,
            DEDICATED.iter().position(|e| *e == effect).unwrap()
        );
        assert!(app.world().resource::<Sounds>().pending.is_empty());
        // One voice no longer stacks copies; only the louder laser is trimmed a little.
        assert_eq!(
            app.world().get::<PlaybackSettings>(last).unwrap().volume,
            Volume::Linear(0.4 * effect.gain())
        );
    }
}

#[test]
fn explosions_and_spike_hits_overlap_without_stealing_each_others_voices() {
    let mut app = app();
    app.world_mut().resource_mut::<Sounds>().pending.extend(
        [
            Effect::Explosion,
            Effect::LargeExplosion,
            Effect::Shot,
            Effect::Recharge,
        ]
        .into_iter()
        .map(|effect| Request {
            effect,
            period: None,
            exit_yes: false,
        }),
    );
    app.update();
    let mut voices = app.world_mut().query::<(Entity, &Voice)>();
    let others: Vec<_> = voices.iter(app.world()).map(|(entity, _)| entity).collect();
    assert_eq!(others.len(), 4);

    for _ in 0..10 {
        app.world_mut()
            .resource_mut::<Sounds>()
            .pending
            .push(Request {
                effect: Effect::Spike,
                period: Some(554),
                exit_yes: false,
            });
        app.update();
    }
    assert_eq!(voices.iter(app.world()).count(), 5);
    assert!(
        others
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_ok())
    );
    let spikes: Vec<_> = voices
        .iter(app.world())
        .filter(|(_, voice)| voice.slot == 0)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(spikes.len(), 1);

    app.world_mut()
        .resource_mut::<Sounds>()
        .pending
        .extend((0..32).map(|_| Request {
            effect: Effect::Explosion,
            period: None,
            exit_yes: false,
        }));
    app.update();
    assert_eq!(voices.iter(app.world()).count(), MAX_VOICES + 1);
    assert!(
        spikes
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_ok())
    );
}

#[test]
fn sustained_fire_is_bounded_and_high_priority_voices_replace_shots() {
    let mut app = app();
    app.update();
    for tick in 0..100 {
        app.world_mut()
            .resource_mut::<Sounds>()
            .pending
            .extend((0..10).map(|_| Request {
                effect: Effect::Shot,
                period: None,
                exit_yes: false,
            }));
        app.update();
        assert_eq!(
            app.world_mut().query::<&Voice>().iter(app.world()).count(),
            ((tick + 1) * 10).min(MAX_VOICES)
        );
    }
    app.world_mut()
        .resource_mut::<Sounds>()
        .pending
        .push(Request {
            effect: Effect::Recharge,
            period: None,
            exit_yes: false,
        });
    app.update();
    assert!(
        app.world_mut()
            .query::<&Voice>()
            .iter(app.world())
            .any(|v| v.priority == 101)
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(20));
    app.update();
    assert_eq!(
        app.world_mut().query::<&Voice>().iter(app.world()).count(),
        0
    );
}

#[test]
fn exit_engines_are_reused_then_removed_when_exit_ends() {
    let mut app = app();
    let level = crate::level::Level::default();
    app.world_mut().resource_mut::<Game>().exit = Some(crate::animations::level_exit::Exit::new(
        level.web.glider_transform(level.web.start()).translation,
        &level.web,
    ));
    app.update();
    assert_eq!(
        app.world_mut().query::<&Engine>().iter(app.world()).count(),
        2
    );
    for _ in 0..100 {
        app.update();
    }
    assert_eq!(
        app.world_mut().query::<&Engine>().iter(app.world()).count(),
        2
    );
    app.world_mut().resource_mut::<Game>().exit = None;
    app.update();
    assert_eq!(
        app.world_mut().query::<&Engine>().iter(app.world()).count(),
        0
    );
}

#[test]
fn reset_stops_old_voices_and_muting_drops_new_requests() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<Sounds>()
        .pending
        .push(Request {
            effect: Effect::Recharge,
            period: None,
            exit_yes: false,
        });
    app.update();
    assert_eq!(
        app.world_mut().query::<&Voice>().iter(app.world()).count(),
        1
    );
    let mut commands = app.world_mut().commands();
    play(&mut commands, Effect::Shot);
    reset(&mut commands);
    app.world_mut().flush();
    assert!(app.world().resource::<Sounds>().pending.is_empty());
    app.update();
    assert_eq!(
        app.world_mut().query::<&Voice>().iter(app.world()).count(),
        0
    );
    app.insert_resource(GlobalVolume::new(Volume::Linear(0.)));
    app.world_mut()
        .resource_mut::<Sounds>()
        .pending
        .push(Request {
            effect: Effect::Shot,
            period: None,
            exit_yes: false,
        });
    app.update();
    assert!(app.world().resource::<Sounds>().pending.is_empty());
    assert_eq!(
        app.world_mut().query::<&Voice>().iter(app.world()).count(),
        0
    );
}

#[test]
fn completed_flips_tink_through_the_aliasing_effect_output_stage() {
    use crate::enemies::flippers::{Motion, move_enemies};

    let mut flips = crate::test_support::app();
    flips
        .init_resource::<Sounds>()
        .add_systems(FixedUpdate, move_enemies);
    let motion = Motion::Flip {
        from: 3,
        direction: 1,
        angle: 0,
    };
    crate::test_support::enemy(&mut flips, 4, 0, motion);
    let requests = (0..60)
        .find_map(|_| {
            ticks(&mut flips, 1);
            let pending = std::mem::take(&mut flips.world_mut().resource_mut::<Sounds>().pending);
            (!pending.is_empty()).then_some(pending)
        })
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].effect, Effect::Tink);
    let mut playback = app();
    playback.update();
    playback.world_mut().resource_mut::<Sounds>().pending = requests;
    playback.update();
    let mut voices = playback
        .world_mut()
        .query::<(&AudioPlayer<Sample>, &PlaybackSettings)>();
    let (player, settings) = voices.single(playback.world()).unwrap();
    assert_eq!(settings.speed, 1.);
    let clip = playback
        .world()
        .resource::<Assets<Sample>>()
        .get(&player.0)
        .unwrap();
    assert_eq!((clip.data.len(), clip.dsp_pitch), (1486, Some(512)));
    let mut decoder = clip.decoder();
    let rate = decoder.sample_rate();
    let pcm: Vec<_> = decoder.by_ref().collect();
    // Played at its nominal 19.3 kHz rate, the tink would peak near 8.5 kHz instead.
    let frequency = dominant_frequency(&pcm, rate);
    assert!((frequency - 6356.).abs() < 50., "{frequency} Hz");
}
