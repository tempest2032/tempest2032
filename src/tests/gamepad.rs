use super::*;

#[test]
fn every_action_has_a_controller_binding() {
    for (button, key) in [
        (GamepadButton::South, KeyCode::KeyZ),
        (GamepadButton::East, KeyCode::KeyX),
        (GamepadButton::West, KeyCode::KeyC),
        (GamepadButton::Start, KeyCode::KeyP),
        (GamepadButton::DPadLeft, KeyCode::ArrowLeft),
        (GamepadButton::DPadRight, KeyCode::ArrowRight),
        (GamepadButton::DPadUp, KeyCode::ArrowUp),
        (GamepadButton::DPadDown, KeyCode::ArrowDown),
        (GamepadButton::LeftTrigger, KeyCode::KeyX),
        (GamepadButton::RightTrigger, KeyCode::KeyC),
        (GamepadButton::LeftTrigger2, KeyCode::KeyX),
        (GamepadButton::RightTrigger2, KeyCode::KeyC),
    ] {
        assert_eq!(button_key(button, false), Some(key), "{button:?}");
    }
    for button in [
        GamepadButton::North,
        GamepadButton::Select,
        GamepadButton::Mode,
        GamepadButton::LeftThumb,
    ] {
        assert_eq!(button_key(button, false), None, "{button:?}");
    }
}

#[test]
fn steam_gamepad_output_uses_gameplay_actions_and_menu_confirm_back() {
    assert_eq!(button_key(GamepadButton::South, true), Some(KeyCode::Enter));
    assert_eq!(button_key(GamepadButton::East, true), Some(KeyCode::Escape));
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::game::Game>()
        .init_resource::<crate::difficulty::EnemyClock>()
        .init_resource::<Time<Virtual>>()
        .add_systems(Update, map_controllers);
    let mut pad = Gamepad::default();
    for button in [
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::West,
        GamepadButton::DPadUp,
        GamepadButton::DPadDown,
    ] {
        pad.digital_mut().press(button);
    }
    app.world_mut().spawn(pad);
    app.update();
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    for key in [
        KeyCode::KeyZ,
        KeyCode::KeyX,
        KeyCode::KeyC,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
    ] {
        assert!(keys.pressed(key), "{key:?}");
    }
    assert!(!keys.pressed(KeyCode::Enter));
    assert!(!keys.pressed(KeyCode::Escape));
}

#[test]
fn remapped_buttons_require_release_but_fresh_presses_work_on_pause_and_game_over() {
    for game_over in [false, true] {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::game::Game>()
            .init_resource::<crate::difficulty::EnemyClock>()
            .init_resource::<Time<Virtual>>()
            .add_systems(Update, map_controllers);
        let mut pad = Gamepad::default();
        pad.digital_mut().press(GamepadButton::South);
        pad.digital_mut().press(GamepadButton::East);
        let entity = app.world_mut().spawn(pad).id();
        app.update();
        if game_over {
            app.world_mut().resource_mut::<crate::game::Game>().status =
                crate::game::Status::GameOver;
        } else {
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
        }
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        for key in [
            KeyCode::KeyZ,
            KeyCode::KeyX,
            KeyCode::Enter,
            KeyCode::Escape,
        ] {
            assert!(!keys.pressed(key), "{key:?}");
        }
        app.world_mut()
            .get_mut::<Gamepad>(entity)
            .unwrap()
            .digital_mut()
            .reset_all();
        app.update();
        app.world_mut()
            .get_mut::<Gamepad>(entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::South);
        app.world_mut()
            .get_mut::<Gamepad>(entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::East);
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Enter)
        );
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Escape)
        );
        app.world_mut().resource_mut::<crate::game::Game>().status = crate::game::Status::Playing;
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyZ)
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyX)
        );
    }
}

#[test]
fn sticks_become_directions_outside_the_deadzone() {
    for (value, expected) in [
        (-1., Some(KeyCode::ArrowLeft)),
        (-DEADZONE, None),
        (0., None),
        (DEADZONE, None),
        (1., Some(KeyCode::ArrowRight)),
    ] {
        assert_eq!(
            axis_key(GamepadAxis::LeftStickX, value),
            expected,
            "x = {value}"
        );
    }
    for (value, expected) in [
        (-1., Some(KeyCode::ArrowDown)),
        (0., None),
        (1., Some(KeyCode::ArrowUp)),
    ] {
        assert_eq!(
            axis_key(GamepadAxis::LeftStickY, value),
            expected,
            "y = {value}"
        );
    }
    assert_eq!(axis_key(GamepadAxis::RightStickX, 1.), None);
}
