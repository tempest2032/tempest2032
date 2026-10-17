use super::*;

fn app(eligible: bool) -> (App, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Game>()
        .init_resource::<crate::messages::Messages>()
        .init_resource::<Difficulty>()
        .init_resource::<Levels>()
        .insert_resource(Store::open(Some(directory.path().join("player.toml"))))
        .insert_resource(FrontEnd::new(
            Screen::Welcome,
            eligible,
            vec![crate::level::Level::default()],
        ))
        .add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .add_message::<Ime>()
        .add_message::<AppExit>()
        .add_systems(Update, input);
    (app, directory)
}

fn key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
}

fn key_catalog(app: &mut App) {
    let mut front = app.world_mut().resource_mut::<FrontEnd>();
    front.originals = (1..=100)
        .map(|number| crate::level::Level {
            number,
            ..default()
        })
        .collect();
    front.start_level = 1;
}

#[test]
fn welcome_selects_odd_starting_webs_within_fresh_and_key_ranges() {
    let (mut app, _directory) = app(true);
    key_catalog(&mut app);
    for (initials, level) in [
        ("AAA", 17),
        ("BBB", 19),
        ("CCC", 21),
        ("DDD", 23),
        ("EEE", 25),
        ("FFF", 27),
    ] {
        app.world_mut()
            .resource_mut::<Store>()
            .data
            .save_key(Difficulty::Normal, level, initials);
    }
    app.world_mut()
        .resource_mut::<Store>()
        .data
        .save_key(Difficulty::Low, 61, "AAA");
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 1);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 1);
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 3);
    for selected in 1..=6 {
        key(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<FrontEnd>().start_selected, selected);
    }
    key(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().resource::<FrontEnd>().start_selected, 0);
    key(&mut app, KeyCode::ArrowLeft);
    key(&mut app, KeyCode::KeyP);
    key(&mut app, KeyCode::Escape);
    assert_eq!(app.world().resource::<FrontEnd>().start_selected, 0);
    key(&mut app, KeyCode::ArrowLeft);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 27);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 25);
    key(&mut app, KeyCode::ArrowUp);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.world().resource::<Game>().load,
        Some(LoadReason::StartLevel { index: 26, initials: Some(initials) }) if initials == *b"FFF"));
    assert!(app.world().resource::<FrontEnd>().eligible);
    app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Welcome;
    key(&mut app, KeyCode::KeyP);
    key(&mut app, KeyCode::ArrowLeft);
    key(&mut app, KeyCode::Escape);
    assert_eq!(app.world().resource::<FrontEnd>().start_selected, 0);
    key(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 61);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 59);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.world().resource::<Game>().load,
        Some(LoadReason::StartLevel { index: 58, initials: Some(initials) }) if initials == *b"AAA"));
}

#[test]
fn fresh_run_can_choose_an_odd_starting_web_up_to_fifteen() {
    let (mut app, _) = app(true);
    key_catalog(&mut app);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 1);
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 1);
    for _ in 0..7 {
        key(&mut app, KeyCode::ArrowUp);
    }
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 15);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 13);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(
        app.world().resource::<Game>().load,
        Some(LoadReason::StartLevel {
            index: 12,
            initials: None
        })
    ));
}

#[test]
fn an_odd_key_checkpoint_is_selectable_and_the_fresh_web_range_is_not_advertised() {
    let (mut app, _) = app(true);
    key_catalog(&mut app);
    app.world_mut()
        .resource_mut::<Store>()
        .data
        .save_key(Difficulty::Normal, 31, "ABC");
    app.add_systems(Update, draw);
    app.init_resource::<Assets<Image>>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Startup, setup);
    app.update();
    let footer = app
        .world_mut()
        .query::<(&Label, &Text)>()
        .iter(app.world())
        .find_map(|(label, text)| matches!(label, Label::Footer).then_some(text.0.clone()))
        .unwrap();
    assert!(!footer.contains("1-15"));
    key(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 31);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<FrontEnd>().preview_level(), 29);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.world().resource::<Game>().load,
        Some(LoadReason::StartLevel { index: 28, initials: Some(initials) }) if initials == *b"ABC"));
}

#[test]
fn key_only_game_over_reuses_initials_entry_without_inserting_a_nonqualifying_score() {
    let (mut app, directory) = app(true);
    for score in 10_000..10_010 {
        app.world_mut().resource_mut::<Store>().data.insert(
            Difficulty::Normal,
            score,
            "AAA".into(),
        );
    }
    app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::GameOver;
    game.score = 1234;
    game.completed_level = 24;
    key(&mut app, KeyCode::Enter);
    let front = app.world().resource::<FrontEnd>();
    assert_eq!(front.screen, Screen::Initials);
    assert!(!front.high_score);
    assert_eq!(front.key_level, Some(23));
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .type_letters("xyz");
    key(&mut app, KeyCode::Enter);
    app.update();
    let store = app.world().resource::<Store>();
    assert_eq!(store.data.tables[1].len(), 10);
    assert!(
        store.data.tables[1]
            .iter()
            .all(|entry| entry.score >= 10_000)
    );
    assert_eq!(store.data.keys.len(), 1);
    assert_eq!(store.data.keys[0].initials, "XYZ");
    assert_eq!(store.data.keys[0].levels, [0, 23, 0]);
    assert_eq!(app.world().resource::<FrontEnd>().start_selected, 0);
    assert_eq!(
        Store::open(Some(directory.path().join("player.toml")))
            .data
            .keys[0]
            .levels,
        [0, 23, 0]
    );
}

#[test]
fn one_initials_submission_saves_both_a_score_and_key_only_for_eligible_runs() {
    for eligible in [false, true] {
        let (mut app, _directory) = app(eligible);
        app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::GameOver;
        game.score = 1234;
        game.completed_level = 19;
        key(&mut app, KeyCode::Enter);
        if eligible {
            let front = app.world().resource::<FrontEnd>();
            assert!(front.high_score);
            assert_eq!(front.key_level, Some(19));
            app.world_mut()
                .resource_mut::<FrontEnd>()
                .type_letters("xyz");
            key(&mut app, KeyCode::Enter);
        }
        app.update();
        let store = app.world().resource::<Store>();
        assert_eq!(store.data.keys.len(), usize::from(eligible));
        assert_eq!(store.data.tables[1].len(), usize::from(eligible));
        if eligible {
            assert_eq!(
                store.data.keys[0].initials,
                store.data.tables[1][0].initials
            );
        }
    }
}

#[test]
fn ineligible_key_runs_and_initials_previews_cannot_save_progress() {
    let (mut app, directory) = app(false);
    app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
    let mut game = app.world_mut().resource_mut::<Game>();
    game.status = Status::GameOver;
    game.score = 258_000;
    game.completed_level = 32;
    game.key_initials = Some(*b"XYZ");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .preview_key_initials();
    key(&mut app, KeyCode::Enter);
    let store = app.world().resource::<Store>();
    assert!(store.data.keys.is_empty());
    assert!(store.data.tables.iter().all(Vec::is_empty));
    assert!(!directory.path().join("player.toml").exists());
}

#[test]
fn key_started_runs_save_under_their_identity_without_another_prompt_or_held_confirm_restart() {
    let (mut app, _directory) = app(true);
    key_catalog(&mut app);
    app.world_mut()
        .resource_mut::<Store>()
        .data
        .save_key(Difficulty::Normal, 21, "XYZ");
    app.add_systems(Update, crate::gamepad::map_controllers.before(input));
    let mut pad = Gamepad::default();
    pad.digital_mut().press(GamepadButton::DPadRight);
    let pad = app.world_mut().spawn(pad).id();
    app.update();
    app.world_mut()
        .get_mut::<Gamepad>(pad)
        .unwrap()
        .digital_mut()
        .release(GamepadButton::DPadRight);
    app.world_mut()
        .get_mut::<Gamepad>(pad)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::South);
    app.update();
    assert!(matches!(app.world().resource::<Game>().load,
        Some(LoadReason::StartLevel { index: 20, initials: Some(initials) }) if initials == *b"XYZ"));
    app.update();
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyZ)
    );
    app.world_mut()
        .get_mut::<Gamepad>(pad)
        .unwrap()
        .digital_mut()
        .release(GamepadButton::South);
    app.update();
    let mut game = app.world_mut().resource_mut::<Game>();
    game.load = None;
    game.key_initials = Some(*b"XYZ");
    game.status = Status::GameOver;
    game.completed_level = 32;
    game.score = 12_345;
    app.world_mut()
        .get_mut::<Gamepad>(pad)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::South);
    app.update();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
    assert_eq!(app.world().resource::<FrontEnd>().start_selected, 0);
    let store = app.world().resource::<Store>();
    assert_eq!(store.data.keys[0].levels, [0, 31, 0]);
    assert_eq!(store.data.tables[1].len(), 1);
    assert_eq!(store.data.tables[1][0].initials, "XYZ");
    assert!(app.world().resource::<Game>().load.is_none());
}

#[test]
fn welcome_and_initials_share_the_scalable_panel_and_restore_other_menu_screens() {
    let (mut app, _directory) = app(true);
    app.init_resource::<Assets<Image>>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<crate::messages::geometry::Glyphs>()
        .add_systems(Startup, setup)
        .add_systems(Update, draw.after(input));
    lettering::add_systems(&mut app);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(1280, 800)
                    .with_scale_factor_override(1.),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ))
        .id();
    app.world_mut().resource_mut::<Store>().data.tables[1] = vec![crate::scores::Entry {
        initials: "MAX".into(),
        score: u32::MAX,
    }];
    app.world_mut().resource_mut::<FrontEnd>().highlight = Some((Difficulty::Normal, 0));
    app.update();
    // Stand in for UI layout, which places the canvas the lettering camera follows.
    let canvas = app
        .world_mut()
        .query_filtered::<Entity, With<lettering::Canvas>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(canvas).insert((
        ComputedNode {
            size: Vec2::new(800., 400.),
            ..default()
        },
        bevy::ui::UiGlobalTransform::from_xy(640., 400.),
    ));
    app.update();
    let scores = app
        .world_mut()
        .query_filtered::<Entity, With<lettering::Canvas>>()
        .single(app.world())
        .unwrap();
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<lettering::PanelCamera>>()
        .single(app.world())
        .unwrap();
    let panel = app.world().get::<Camera>(camera).unwrap();
    assert!(panel.is_active);
    let viewport = panel.viewport.as_ref().unwrap();
    assert!(viewport.physical_size.x > 0 && viewport.physical_size.y > 0);
    let mesh = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<lettering::LetteringMesh>>()
        .single(app.world())
        .unwrap();
    let mesh = app.world().resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
    let bevy::mesh::VertexAttributeValues::Float32x3(points) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        panic!("welcome scores have no positions");
    };
    assert!(!points.is_empty());
    assert!(
        points
            .iter()
            .all(|p| p[0].abs() <= 280. && p[1].abs() <= 225.)
    );
    let original = points.clone();
    key_catalog(&mut app);
    app.world_mut()
        .resource_mut::<Store>()
        .data
        .save_key(Difficulty::Normal, 97, "MAX");
    key(&mut app, KeyCode::ArrowRight);
    let mesh = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<lettering::LetteringMesh>>()
        .single(app.world())
        .unwrap()
        .0
        .clone();
    let bevy::mesh::VertexAttributeValues::Float32x3(selected) = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .unwrap()
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
    else {
        panic!("Key selector has no positions");
    };
    assert_ne!(selected, &original);
    assert!(
        selected
            .iter()
            .all(|p| p[0].abs() <= 280. && p[1].abs() <= 225.)
    );
    for (width, height, scale) in [(1280., 800., 1.), (2560., 1600., 2.)] {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(width, height);
        app.update();
        let node = app
            .world_mut()
            .query_filtered::<&Node, With<Panel>>()
            .single(app.world())
            .unwrap();
        assert_eq!((node.width, node.height), (percent(90), percent(86)));
        assert_eq!(node.padding, UiRect::all(px(20. * scale)));
        for (label, font) in app
            .world_mut()
            .query::<(&Label, &TextFont)>()
            .iter(app.world())
        {
            if matches!(label, Label::Footer) {
                assert_eq!(font.font_size, 16. * scale);
            }
        }
    }
    key(&mut app, KeyCode::KeyP);
    assert_eq!(
        app.world().get::<Node>(scores).unwrap().display,
        Display::None
    );
    assert!(!app.world().get::<Camera>(camera).unwrap().is_active);
    key(&mut app, KeyCode::Escape);
    assert_eq!(
        app.world().get::<Node>(scores).unwrap().display,
        Display::Flex
    );
    assert!(app.world().get::<Camera>(camera).unwrap().is_active);
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .enter_initials(u32::MAX, "AAA");
    app.world_mut().resource_mut::<FrontEnd>().key_level = Some(97);
    app.update();
    let node = app
        .world_mut()
        .query_filtered::<&Node, With<Panel>>()
        .single(app.world())
        .unwrap();
    assert_eq!((node.width, node.height), (percent(70), percent(86)));
    assert!(app.world().get::<Camera>(camera).unwrap().is_active);
    let mesh = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<lettering::LetteringMesh>>()
        .single(app.world())
        .unwrap()
        .0
        .clone();
    let positions = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .unwrap()
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .clone();
    key(&mut app, KeyCode::ArrowRight);
    assert_ne!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap(),
        &positions
    );
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
    key(&mut app, KeyCode::Enter);
    assert!(!app.world().get::<Camera>(camera).unwrap().is_active);
}

#[test]
fn menus_do_not_compound_difficulty_and_consume_transition_keys() {
    let (mut app, _directory) = app(true);
    key(&mut app, KeyCode::KeyP);
    assert_eq!(
        app.world().resource::<FrontEnd>().screen,
        Screen::Configuration
    );
    for _ in 0..3 {
        key(&mut app, KeyCode::ArrowRight);
    }
    assert_eq!(*app.world().resource::<Difficulty>(), Difficulty::Normal);
    key(&mut app, KeyCode::ArrowLeft);
    assert_eq!(*app.world().resource::<Difficulty>(), Difficulty::Low);
    key(&mut app, KeyCode::Escape);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Gameplay);
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Enter)
    );
    let mut expected = crate::level::Level::default();
    expected.scale_difficulty(Difficulty::Low.scale());
    assert!(app.world().resource::<Levels>().0[0] == expected);
    app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Welcome;
    key(&mut app, KeyCode::Enter);
    assert!(app.world().resource::<Levels>().0[0] == expected);
}

#[test]
fn escape_abandons_gameplay_without_saving_or_exiting_but_resumes_when_paused() {
    for eligible in [false, true] {
        let (mut app, _directory) = app(eligible);
        app.insert_resource(Time::<Virtual>::default())
            .init_resource::<crate::pause::Menu>()
            .add_systems(Update, crate::pause::input.after(input));
        app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
        app.world_mut().resource_mut::<Game>().score = 1234;
        app.world_mut().resource_mut::<Game>().completed_level = 32;
        app.world_mut().resource_mut::<Game>().message_epoch = 7;
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        key(&mut app, KeyCode::Escape);
        assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Gameplay);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(app.world().resource::<Game>().score, 1234);
        key(&mut app, KeyCode::Escape);
        assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
        assert_eq!(app.world().resource::<Game>().score, 0);
        assert_eq!(app.world().resource::<Game>().message_epoch, 7);
        assert!(app.world().resource::<Game>().load.is_none());
        assert!(app.world().resource::<Store>().data.keys.is_empty());
        assert!(
            app.world()
                .resource::<Store>()
                .data
                .tables
                .iter()
                .all(Vec::is_empty)
        );
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        app.update();
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        key(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
    }
}

#[test]
fn only_eligible_completed_runs_prompt_and_save_exactly_once() {
    for eligible in [false, true] {
        let (mut app, _directory) = app(eligible);
        app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
        let mut game = app.world_mut().resource_mut::<Game>();
        game.status = Status::GameOver;
        game.score = 1234;
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<FrontEnd>().screen,
            if eligible {
                Screen::Initials
            } else {
                Screen::Welcome
            }
        );
        assert!(app.world().resource::<Store>().data.tables[1].is_empty());
        if eligible {
            app.world_mut().write_message(Ime::Commit {
                window: Entity::PLACEHOLDER,
                value: "pRz".into(),
            });
            app.update();
            assert_eq!(app.world().resource::<FrontEnd>().initials, *b"PRZ");
            key(&mut app, KeyCode::Enter);
            app.update();
            assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
            let table = &app.world().resource::<Store>().data.tables[1];
            assert_eq!(table.len(), 1);
            assert_eq!(table[0].initials, "PRZ");
            assert_eq!(table[0].score, 1234);
        }
    }
}

#[test]
fn controller_initials_wrap_and_backspace_allows_retyping() {
    let (mut app, _directory) = app(true);
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .enter_initials(42, "AAA");
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::ArrowRight);
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"ZBA");
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .type_letters("xy");
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"ZXY");
    key(&mut app, KeyCode::Backspace);
    app.world_mut().resource_mut::<FrontEnd>().type_letters("q");
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"ZXQ");
}

#[test]
fn bundled_definitions_qualify_but_modified_and_extra_levels_do_not() {
    let mut levels = crate::test_support::levels();
    assert!(levels.is_standard());
    levels.0[0].flippers.count += 1;
    assert!(!levels.is_standard());
    levels.0[0].flippers.count -= 1;
    levels.0.push(crate::level::Level::default());
    assert!(!levels.is_standard());
}

#[test]
fn held_controller_confirm_does_not_fire_or_submit_the_next_screen() {
    let (mut app, _directory) = app(true);
    app.add_systems(Update, crate::gamepad::map_controllers.before(input));
    let mut pad = Gamepad::default();
    pad.digital_mut().press(GamepadButton::South);
    let entity = app.world_mut().spawn(pad).id();
    app.update();
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Gameplay);
    for _ in 0..5 {
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::Enter)
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyZ)
        );
    }
    app.world_mut().resource_mut::<Store>().data.initials = "XYZ".into();
    app.world_mut()
        .get_mut::<Gamepad>(entity)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::East);
    app.update();
    app.world_mut().resource_mut::<Game>().status = Status::GameOver;
    app.world_mut().resource_mut::<Game>().score = 42;
    app.update();
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Gameplay);
    app.world_mut()
        .get_mut::<Gamepad>(entity)
        .unwrap()
        .digital_mut()
        .release(GamepadButton::South);
    app.update();
    app.world_mut()
        .get_mut::<Gamepad>(entity)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::South);
    app.update();
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Initials);
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"XYZ");
    assert!(app.world().resource::<Store>().data.tables[1].is_empty());
}

#[test]
fn physical_text_handles_shortcut_letters_and_transition_auto_repeat() {
    let (mut app, _directory) = app(true);
    fn event(app: &mut App, key: KeyCode, value: Option<&str>, state: ButtonState, repeat: bool) {
        if state == ButtonState::Pressed {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(key);
        }
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: bevy::input::keyboard::Key::Unidentified(
                bevy::input::keyboard::NativeKey::Unidentified,
            ),
            state,
            text: value.map(Into::into),
            repeat,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    event(&mut app, KeyCode::Enter, None, ButtonState::Pressed, false);
    event(&mut app, KeyCode::Enter, None, ButtonState::Pressed, true);
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Enter)
    );
    event(&mut app, KeyCode::Enter, None, ButtonState::Released, false);
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .enter_initials(42, "AAA");
    for (key, value) in [
        (KeyCode::KeyP, "p"),
        (KeyCode::KeyR, "r"),
        (KeyCode::KeyZ, "z"),
    ] {
        event(&mut app, key, Some(value), ButtonState::Pressed, false);
    }
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"PRZ");
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Initials);
}

#[test]
fn initials_follow_os_characters_instead_of_physical_key_positions() {
    let (mut app, _directory) = app(true);
    app.world_mut()
        .resource_mut::<FrontEnd>()
        .enter_initials(42, "AAA");
    for (physical, character, text) in [
        (KeyCode::KeyQ, "a", Some("a")),
        (KeyCode::KeyA, "q", None),
        (KeyCode::KeyY, "z", Some("z")),
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(physical);
        app.world_mut().write_message(KeyboardInput {
            key_code: physical,
            logical_key: Key::Character(character.into()),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    assert_eq!(app.world().resource::<FrontEnd>().initials, *b"AQZ");
    assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Initials);
    assert!(app.world().resource::<Game>().load.is_none());
}

#[test]
fn pause_exit_returns_to_welcome_without_a_held_confirm_starting_another_run() {
    for controller in [false, true] {
        let (mut app, _directory) = app(true);
        let mut time = Time::<Virtual>::default();
        time.pause();
        app.insert_resource(time)
            .insert_resource(crate::pause::Menu { selected: 2 })
            .add_systems(Update, crate::pause::input.after(input));
        app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
        app.world_mut().resource_mut::<Game>().score = 1234;
        let pad = if controller {
            app.add_systems(Update, crate::gamepad::map_controllers.before(input));
            let mut pad = Gamepad::default();
            pad.digital_mut().press(GamepadButton::South);
            Some(app.world_mut().spawn(pad).id())
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Enter);
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Enter,
                logical_key: bevy::input::keyboard::Key::Enter,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            None
        };
        app.update();
        assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(app.world().resource::<Game>().score, 0);
        assert!(app.world().resource::<Game>().load.is_none());
        assert!(
            app.world()
                .resource::<Store>()
                .data
                .tables
                .iter()
                .all(Vec::is_empty)
        );
        for _ in 0..3 {
            if !controller {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::Enter);
            }
            app.update();
            assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Welcome);
        }
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        if let Some(pad) = pad {
            app.world_mut()
                .get_mut::<Gamepad>(pad)
                .unwrap()
                .digital_mut()
                .release(GamepadButton::South);
            app.update();
            app.world_mut()
                .get_mut::<Gamepad>(pad)
                .unwrap()
                .digital_mut()
                .press(GamepadButton::South);
            app.update();
        } else {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Enter,
                logical_key: bevy::input::keyboard::Key::Enter,
                state: ButtonState::Released,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
            key(&mut app, KeyCode::Enter);
        }
        assert_eq!(app.world().resource::<FrontEnd>().screen, Screen::Gameplay);
    }
}

#[test]
fn menus_stop_rendering_the_game_underneath() {
    let (mut app, _directory) = app(false);
    app.add_systems(Update, cover_gameplay);
    let web = app
        .world_mut()
        .spawn((Camera::default(), crate::camera::WebCamera))
        .id();
    let effects = app.world_mut().spawn((Camera::default(), Camera2d)).id();
    let overlay = app.world_mut().spawn(Camera::default()).id();
    app.update();
    let active = |app: &App, entity| app.world().get::<Camera>(entity).unwrap().is_active;
    assert!(!active(&app, web) && !active(&app, effects));
    assert!(
        active(&app, overlay),
        "the UI camera keeps rendering the menus"
    );
    app.world_mut().resource_mut::<FrontEnd>().screen = Screen::Gameplay;
    app.update();
    assert!(active(&app, web) && active(&app, effects));
}
