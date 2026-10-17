//! Native-resolution game lettering for Welcome and high-score initials entry.

use super::{FrontEnd, Screen};
use crate::{
    difficulty::Difficulty,
    messages::geometry::{self, ADVANCE, Glyphs, OverlayCamera, Style},
    scores::{Entry, Store},
};
use bevy::{
    camera::{ScalingMode, Viewport, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    ui::UiGlobalTransform,
};

#[derive(Component)]
pub(super) struct Canvas;

#[derive(Component)]
pub(super) struct PanelCamera;

#[derive(Component)]
pub(super) struct LetteringMesh;

#[derive(Resource)]
struct Graphics {
    mesh: Handle<Mesh>,
    previous: Option<Frame>,
}

#[derive(PartialEq)]
enum Frame {
    Welcome(
        Difficulty,
        Vec<Entry>,
        Option<usize>,
        Option<(String, u8)>,
        usize,
        bool,
    ),
    Initials(Difficulty, u32, [u8; 3], usize, bool, Option<u8>),
}

pub(super) fn add_systems(app: &mut App) {
    app.add_systems(Update, draw.after(super::draw));
}

pub(super) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Draws into the canvas node's rectangle of the window, after the UI.
    commands.spawn((
        PanelCamera,
        OverlayCamera,
        Camera3d::default(),
        Camera {
            order: 4,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 560.,
                min_height: 390.,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0., 0., 100.),
        RenderLayers::layer(4),
        bevy::render::view::NoIndirectDrawing,
        Tonemapping::None,
        Msaa::Sample4,
    ));
    let mesh = meshes.add(geometry::mesh(Vec::new()));
    commands.spawn((
        LetteringMesh,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        RenderLayers::layer(4),
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.insert_resource(Graphics {
        mesh,
        previous: None,
    });
}

/// The canvas node's rectangle in physical window pixels, once laid out.
fn viewport(node: &ComputedNode, transform: &UiGlobalTransform, window: UVec2) -> Option<Viewport> {
    let position = (transform.translation - node.size() / 2.)
        .round()
        .max(Vec2::ZERO)
        .as_uvec2();
    let size = node
        .size()
        .round()
        .as_uvec2()
        .min(window.saturating_sub(position));
    (size.x > 0 && size.y > 0).then(|| Viewport {
        physical_position: position,
        physical_size: size,
        ..default()
    })
}

#[allow(clippy::too_many_arguments)]
fn draw(
    front: Res<FrontEnd>,
    store: Res<Store>,
    difficulty: Res<Difficulty>,
    glyphs: Res<Glyphs>,
    mut graphics: ResMut<Graphics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut camera: Single<(&mut Camera, &mut Projection), With<PanelCamera>>,
    canvas: Single<(&ComputedNode, &UiGlobalTransform), With<Canvas>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut scaled: Local<Option<Screen>>,
) {
    let window = windows
        .iter()
        .next()
        .map_or(UVec2::ZERO, |w| w.physical_size());
    let viewport = viewport(canvas.0, canvas.1, window)
        .filter(|_| matches!(front.screen, Screen::Welcome | Screen::Initials));
    let active = viewport.is_some();
    let rect = |v: &Option<Viewport>| v.as_ref().map(|v| (v.physical_position, v.physical_size));
    if rect(&camera.0.viewport) != rect(&viewport) {
        camera.0.viewport = viewport;
    }
    if camera.0.is_active != active {
        camera.0.is_active = active;
    }
    if !active {
        return;
    }
    // ScalingMode has no equality, so only rewrite it when the screen changes.
    if *scaled != Some(front.screen)
        && let Projection::Orthographic(projection) = &mut *camera.1
    {
        *scaled = Some(front.screen);
        projection.scaling_mode = ScalingMode::AutoMin {
            min_width: 560.,
            min_height: if front.screen == Screen::Welcome {
                450.
            } else {
                390.
            },
        };
    }
    let entries = &store.data.tables[difficulty.index()];
    let highlight = front
        .highlight
        .filter(|(level, _)| *level == *difficulty)
        .map(|(_, rank)| rank);
    let frame = if front.screen == Screen::Welcome {
        let start = front
            .selected_key(&store.data, *difficulty)
            .map(|key| (key.initials.clone(), key.levels[difficulty.index()]));
        let has_keys = front
            .available_keys(&store.data, *difficulty)
            .next()
            .is_some();
        let selected_level = front.preview_level();
        if matches!(&graphics.previous, Some(Frame::Welcome(level, scores, marked, selected, old_level, available))
            if *level == *difficulty && scores == entries && *marked == highlight
                && *selected == start && *old_level == selected_level && *available == has_keys)
        {
            return;
        }
        Frame::Welcome(
            *difficulty,
            entries.clone(),
            highlight,
            start,
            selected_level,
            has_keys,
        )
    } else {
        let frame = Frame::Initials(
            *difficulty,
            front.score,
            front.initials,
            front.cursor,
            front.high_score,
            front.key_level,
        );
        if graphics.previous.as_ref() == Some(&frame) {
            return;
        }
        frame
    };
    let mut triangles = Vec::new();
    let mut text = |value: &str, x: f32, y: f32, style: Style| {
        glyphs.place(
            &mut triangles,
            value,
            style,
            Vec3::new(x, y, 0.),
            Vec3::splat(1.2),
            ADVANCE * 1.2,
        );
    };
    if front.screen == Screen::Welcome {
        text("HIGH SCORES", -11. * ADVANCE * 0.6, 181., Style::Chrome);
        text(
            difficulty.label(),
            -(difficulty.label().len() as f32) * ADVANCE * 0.6,
            148.,
            Style::Chrome,
        );
        for rank in 0..crate::scores::LIMIT {
            let y = 102. - rank as f32 * 25.;
            text(&format!("{:2}", rank + 1), -235., y, Style::Chrome);
            let entry = entries.get(rank);
            text(
                entry.map_or("---", |entry| entry.initials.as_str()),
                -125.,
                y,
                Style::Chrome,
            );
            let score = entry.map_or_else(|| "--------".into(), |entry| entry.score.to_string());
            text(
                &score,
                260. - score.len() as f32 * ADVANCE * 1.2,
                y,
                Style::Rainbow,
            );
        }
        let selection = front.selected_key(&store.data, *difficulty).map_or_else(
            || format!("LEVEL {}", front.preview_level()),
            |key| format!("{} - LEVEL {}", key.initials, front.preview_level()),
        );
        let width = selection.len() as f32 * ADVANCE * 1.2;
        text(&selection, -width / 2., -160., Style::Chrome);
        let bonus = format!(
            "{} BONUS",
            crate::game::Game::starting_bonus(front.preview_level())
        );
        text(
            &bonus,
            -(bonus.len() as f32) * ADVANCE * 0.6,
            -200.,
            Style::Rainbow,
        );
        if front
            .available_keys(&store.data, *difficulty)
            .next()
            .is_some()
        {
            for direction in [-1., 1.] {
                let x = direction * (width / 2. + 18.);
                let color = Color::srgb(1., 0.88, 0.48);
                triangles.push([
                    (Vec3::new(x, -171., 1.), color),
                    (Vec3::new(x - direction * 9., -164., 1.), color),
                    (Vec3::new(x - direction * 9., -178., 1.), color),
                ]);
            }
        }
        if let Some(rank) = highlight {
            let y = 91. - rank as f32 * 25.;
            triangles.push([
                (Vec3::new(-269., y + 8., 1.), Color::srgb(1., 0.88, 0.48)),
                (Vec3::new(-258., y, 1.), Color::srgb(1., 0.88, 0.48)),
                (Vec3::new(-269., y - 8., 1.), Color::srgb(1., 0.88, 0.48)),
            ]);
        }
    } else {
        let heading = if front.high_score {
            "NEW HIGH SCORE"
        } else {
            "KEY EARNED"
        };
        text(
            heading,
            -(heading.len() as f32) * ADVANCE * 0.6,
            181.,
            Style::Chrome,
        );
        if front.high_score {
            text(&front.score.to_string(), -235., 148., Style::Rainbow);
        }
        text(
            difficulty.label(),
            if front.high_score {
                235. - difficulty.label().len() as f32 * ADVANCE * 1.2
            } else {
                -(difficulty.label().len() as f32) * ADVANCE * 0.6
            },
            148.,
            Style::Chrome,
        );
        text(
            "ENTER YOUR INITIALS",
            -19. * ADVANCE * 0.6,
            116.,
            Style::Chrome,
        );
        // Keep the selected initials above the Steam Deck keyboard's lower half.
        for (index, letter) in front.initials.iter().enumerate() {
            text(
                &char::from(*letter).to_string(),
                index as f32 * 80. - 89.,
                85.,
                if index == front.cursor {
                    Style::Rainbow
                } else {
                    Style::Chrome
                },
            );
        }
        if let Some(level) = front.key_level {
            let prefix = "KEY LEVEL ";
            let number = level.to_string();
            let x = -((prefix.len() + number.len()) as f32) * ADVANCE * 0.6;
            text(prefix, x, 25., Style::Chrome);
            text(
                &number,
                x + prefix.len() as f32 * ADVANCE * 1.2,
                25.,
                Style::Rainbow,
            );
        }
        let x = front.cursor as f32 * 80. - 80.;
        let corners = [
            Vec3::new(x - 15., 61., 1.),
            Vec3::new(x + 15., 61., 1.),
            Vec3::new(x + 15., 58., 1.),
            Vec3::new(x - 15., 58., 1.),
        ];
        triangles.extend(
            [[0, 1, 2], [0, 2, 3]]
                .map(|face| face.map(|index| (corners[index], Color::srgb(1., 0.88, 0.48)))),
        );
    }
    *meshes.get_mut(&graphics.mesh).unwrap() = geometry::mesh(triangles);
    graphics.previous = Some(frame);
}
