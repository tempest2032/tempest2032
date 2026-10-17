//! Event-driven native-resolution announcements and independent text effects.

mod catalog;
mod font;
mod game_over;
pub(crate) mod geometry;
mod lettering;
mod render;
mod solid;

use bevy::prelude::*;
pub(crate) use catalog::Id;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Event {
    pub(crate) id: Id,
    pub(crate) origin: Option<Vec3>,
}

impl From<Id> for Event {
    fn from(id: Id) -> Self {
        Self { id, origin: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Active {
    pub(super) event: Event,
    pub(super) age: u32,
}

impl Active {
    pub(super) fn scale(self) -> Vec2 {
        Vec2::new(
            1.,
            1. + self.age.saturating_sub(u32::from(self.event.id.hold()) + 1) as f32 * 0.5,
        )
    }

    pub(super) fn alive(self) -> bool {
        match self.event.id {
            Id::GreenTrack => self.age < 200,
            Id::WarpFive => self.age < 401,
            Id::GameOver => true,
            _ => self.age < u32::from(self.event.id.hold()) + 52,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct Messages {
    pub(super) banner: Option<Active>,
    pub(super) rewards: Vec<Active>,
    pub(super) game_over: Option<Active>,
    pub(super) clock: u32,
    pub(crate) events: Vec<Event>,
    epoch: u32,
}

#[derive(Resource)]
pub(crate) struct Preview {
    pub(crate) id: Id,
    pub(crate) capture_tick: Option<u16>,
    pub(crate) elapsed: u32,
}

impl Messages {
    pub(crate) fn send(&mut self, id: Id) {
        self.events.push(id.into());
    }
    pub(super) fn step(&mut self) {
        self.clock = self.clock.wrapping_add(1);
        if let Some(active) = &mut self.banner {
            active.age += 1;
        }
        self.banner = self.banner.filter(|active| active.alive());
        if let Some(active) = &mut self.game_over {
            active.age = active.age.saturating_add(1);
        }
        for active in &mut self.rewards {
            active.age += 1;
        }
        self.rewards.retain(|active| {
            active.age
                < if matches!(
                    active.event.id,
                    Id::Points250 | Id::Points500 | Id::Points750
                ) {
                    80
                } else {
                    151
                }
        });
        for event in self.events.drain(..) {
            if event.id == Id::GameOver {
                self.banner = None;
                self.rewards.clear();
                self.game_over = Some(Active { event, age: 0 });
            } else if event.id.reward() {
                if self.rewards.len() < 32 {
                    self.rewards.push(Active { event, age: 0 });
                }
            } else {
                self.banner = Some(Active { event, age: 0 });
            }
        }
    }
}

pub(crate) fn add_systems(app: &mut App) {
    app.init_resource::<Messages>()
        .init_resource::<font::Font>()
        .add_systems(Startup, render::setup)
        .add_systems(FixedUpdate, advance.after(crate::game::load_level))
        .add_systems(Update, render::draw.after(crate::warp::render::draw));
    game_over::add_systems(app);
    solid::add_systems(app);
}

pub(crate) fn running(preview: Option<Res<Preview>>) -> bool {
    preview.is_none_or(|preview| {
        preview
            .capture_tick
            .is_none_or(|tick| preview.elapsed <= u32::from(tick))
    })
}

fn advance(
    mut commands: Commands,
    mut messages: ResMut<Messages>,
    mut preview: Option<ResMut<Preview>>,
    mut game: ResMut<crate::game::Game>,
    mut upgrades: ResMut<crate::powerups::Upgrades>,
) {
    if messages.epoch != game.message_epoch {
        if let Some(preview) = &mut preview {
            preview.elapsed = 0;
        }
        *messages = Messages {
            epoch: game.message_epoch,
            ..default()
        };
    }
    if preview.is_none() {
        for id in upgrades.announcements.drain(..) {
            messages.send(id);
        }
        messages.events.append(&mut game.announcements);
    } else {
        upgrades.announcements.clear();
        game.announcements.clear();
    }
    if let Some(preview) = &mut preview {
        if preview
            .capture_tick
            .is_some_and(|tick| preview.elapsed > u32::from(tick))
        {
            return;
        }
        if preview.elapsed == 0 {
            messages.send(preview.id);
        }
        preview.elapsed += 1;
        if preview.capture_tick.is_none() && preview.elapsed > u32::from(preview.id.hold()) + 90 {
            preview.elapsed = 0;
        }
    }
    for event in &messages.events {
        if game.status != crate::game::Status::Bonus
            || !matches!(event.id, Id::Points250 | Id::Points500 | Id::Points750)
        {
            crate::sound::announcement(&mut commands, event.id);
        }
    }
    messages.step();
    if game.bonus.as_ref().is_some_and(|session| {
        session.kind == crate::warp::ZoneKind::Tunnel
            && matches!(session.phase, crate::warp::Phase::Fade(_))
    }) && messages
        .banner
        .is_some_and(|active| active.event.id == Id::WarpFive)
    {
        // _tunn hides its separate text strip before calling ofade.
        messages.banner = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_completion_hides_its_text_strip_when_the_final_melt_starts() {
        for kind in [
            crate::warp::ZoneKind::Tunnel,
            crate::warp::ZoneKind::GateFlight,
            crate::warp::ZoneKind::Rings,
        ] {
            let mut app = App::new();
            app.init_resource::<Messages>()
                .init_resource::<crate::game::Game>()
                .init_resource::<crate::powerups::Upgrades>()
                .add_systems(Update, advance);
            let mut game = app.world_mut().resource_mut::<crate::game::Game>();
            game.status = crate::game::Status::Bonus;
            let mut session = crate::warp::Session::new(kind, 0, Some(1), 0);
            session.phase = crate::warp::Phase::Victory(1);
            game.bonus = Some(session);
            game.announce(Id::WarpFive);
            app.update();
            assert!(app.world().resource::<Messages>().banner.is_some());
            app.world_mut()
                .resource_mut::<crate::game::Game>()
                .bonus
                .as_mut()
                .unwrap()
                .phase = crate::warp::Phase::Fade(151);
            app.update();
            assert_eq!(
                app.world().resource::<Messages>().banner.is_none(),
                kind == crate::warp::ZoneKind::Tunnel
            );
        }
    }

    #[test]
    fn frozen_previews_keep_exact_age_and_score_lives_emit_one_effect_each() {
        let mut app = App::new();
        app.init_resource::<Messages>()
            .init_resource::<crate::game::Game>()
            .init_resource::<crate::powerups::Upgrades>()
            .insert_resource(Preview {
                id: Id::ParticleLaser,
                capture_tick: Some(130),
                elapsed: 0,
            })
            .add_systems(Update, advance);
        for _ in 0..200 {
            app.update();
        }
        assert_eq!(app.world().resource::<Messages>().banner.unwrap().age, 130);
        assert_eq!(app.world().resource::<Preview>().elapsed, 131);
        app.world_mut().remove_resource::<Preview>();
        app.update();
        let mut game = app.world_mut().resource_mut::<crate::game::Game>();
        game.add_score(40_000);
        game.lives -= 1;
        app.update();
        assert_eq!(app.world().resource::<Messages>().rewards.len(), 2);
        assert_eq!(
            app.world().resource::<Messages>().rewards[0].event.id,
            Id::OneUp
        );
        app.update();
        assert_eq!(app.world().resource::<Messages>().rewards.len(), 2);
    }
    #[test]
    fn tunnel_instructions_expire_at_200_ticks_and_game_over_persists() {
        let mut messages = Messages::default();
        messages.send(Id::GreenTrack);
        messages.step();
        for _ in 0..199 {
            messages.step();
        }
        assert!(messages.banner.is_some());
        messages.step();
        assert!(messages.banner.is_none());
        messages.send(Id::GameOver);
        messages.step();
        for _ in 0..1000 {
            messages.step();
        }
        assert!(messages.game_over.is_some());
        assert!(messages.banner.is_none());
    }
    #[test]
    fn reward_effects_coexist_with_announcements_and_expire_independently() {
        let mut messages = Messages::default();
        messages.send(Id::ParticleLaser);
        messages.send(Id::Excellent);
        messages.send(Id::Points250);
        messages.step();
        assert_eq!(messages.banner.unwrap().event.id, Id::ParticleLaser);
        assert_eq!(messages.rewards.len(), 2);
        for _ in 0..80 {
            messages.step();
        }
        assert_eq!(messages.rewards.len(), 1);
        assert_eq!(messages.rewards[0].event.id, Id::Excellent);
        for _ in 0..71 {
            messages.step();
        }
        assert!(messages.rewards.is_empty());
    }
    #[test]
    fn game_events_are_consumed_once_and_restart_clears_old_messages() {
        let mut app = App::new();
        app.init_resource::<Messages>()
            .init_resource::<crate::game::Game>()
            .init_resource::<crate::powerups::Upgrades>()
            .add_systems(Update, advance);
        app.world_mut()
            .resource_mut::<crate::game::Game>()
            .announce(Id::CaughtYou);
        app.update();
        assert_eq!(
            app.world().resource::<Messages>().banner.unwrap().event.id,
            Id::CaughtYou
        );
        assert!(
            app.world()
                .resource::<crate::game::Game>()
                .announcements
                .is_empty()
        );
        app.update();
        assert_eq!(app.world().resource::<Messages>().banner.unwrap().age, 1);
        app.world_mut()
            .resource_mut::<crate::game::Game>()
            .message_epoch += 1;
        app.update();
        assert!(app.world().resource::<Messages>().banner.is_none());
    }
    #[test]
    fn announcements_hold_then_stretch_with_original_inclusive_expiry() {
        let mut messages = Messages::default();
        messages.send(Id::ParticleLaser);
        messages.step();
        for _ in 0..101 {
            messages.step();
        }
        assert_eq!(messages.banner.unwrap().scale(), Vec2::ONE);
        messages.step();
        assert_eq!(messages.banner.unwrap().scale(), Vec2::new(1., 1.5));
        for _ in 0..50 {
            messages.step();
        }
        assert!(messages.banner.is_none());
        messages.send(Id::AvoidSpikes);
        messages.step();
        for _ in 0..251 {
            messages.step();
        }
        assert_eq!(messages.banner.unwrap().scale(), Vec2::ONE);
    }
    #[test]
    fn replacement_and_identical_announcements_restart_the_timer() {
        let mut messages = Messages::default();
        messages.send(Id::ParticleLaser);
        messages.step();
        messages.step();
        messages.send(Id::ParticleLaser);
        messages.step();
        assert_eq!(messages.banner.unwrap().age, 0);
        messages.send(Id::AiDroid);
        messages.send(Id::CaughtYou);
        messages.step();
        assert_eq!(messages.banner.unwrap().event.id, Id::CaughtYou);
    }
}
