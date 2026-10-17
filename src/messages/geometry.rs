//! Clean font outlines, rounded announcement contours, bevels, and studio shading.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers, math::point,
    path::Path,
};
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) type Triangle = [(Vec3, Color); 3];

/// Width of the primary window in the 240-line logical space, 384 without a window.
pub(crate) fn viewport_width(windows: &Query<&Window, With<bevy::window::PrimaryWindow>>) -> f32 {
    windows
        .iter()
        .next()
        .map_or(384., |w| 240. * w.width() / w.height().max(1.))
}
pub(crate) const ADVANCE: f32 = 17.;

#[derive(Component)]
pub(crate) struct OverlayCamera;

pub(crate) type GameCameras = (With<Camera3d>, Without<OverlayCamera>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Style {
    Chrome,
    Rainbow,
    Gold,
}

#[derive(Clone)]
pub(super) struct Fragment {
    pub triangle: Triangle,
    pub center: Vec2,
}

pub(crate) struct Glyph {
    pub triangles: Vec<Triangle>,
    fragments: OnceLock<Box<[Fragment]>>,
}

impl Glyph {
    pub(super) fn fragments(&self) -> &[Fragment] {
        self.fragments
            .get_or_init(|| fragments(&self.triangles).into_boxed_slice())
    }
}

const CHARACTERS: &str = " !(),-.:?0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Extruded glyphs, built on first use except those gameplay draws, so menu- and
/// pause-only styles cost neither startup time nor memory until shown.
#[derive(Resource)]
pub(crate) struct Glyphs {
    glyphs: HashMap<(char, Style), OnceLock<Glyph>>,
    announcements: HashMap<char, Vec<Vec<Vec2>>>,
}

impl Default for Glyphs {
    fn default() -> Self {
        let glyphs = Self {
            glyphs: CHARACTERS
                .chars()
                .flat_map(|c| {
                    [Style::Chrome, Style::Rainbow, Style::Gold].map(|s| ((c, s), OnceLock::new()))
                })
                .collect(),
            announcements: super::lettering::contours(),
        };
        // Announcements and score digits appear mid-game, where a build could cost a frame.
        for character in CHARACTERS.chars() {
            glyphs.get(character, Style::Chrome);
        }
        for digit in '0'..='9' {
            glyphs.get(digit, Style::Rainbow);
        }
        glyphs
    }
}

fn build(character: char, style: Style, announcements: &HashMap<char, Vec<Vec<Vec2>>>) -> Glyph {
    let contours = outline(character);
    let contours = match style {
        Style::Chrome => announcements
            .get(&character)
            .cloned()
            .unwrap_or_else(|| contours.iter().map(|p| rounded(p, 1.25)).collect()),
        Style::Gold if character == 'A' => vec![
            polygon(&[
                (0., 18.),
                (0., 4.),
                (4., 0.),
                (11., 0.),
                (15., 4.),
                (15., 18.),
                (11., 18.),
                (11., 12.),
                (4., 12.),
                (4., 18.),
            ]),
            rect(4., 4., 7., 4.),
        ],
        _ => contours,
    };
    extrude(&contours, &Paint::new(style))
}

impl Glyphs {
    /// Appends a line of glyphs starting at `origin`, `advance` apart, with glyph Y pointing down.
    pub(crate) fn place(
        &self,
        triangles: &mut Vec<Triangle>,
        text: &str,
        style: Style,
        origin: Vec3,
        scale: Vec3,
        advance: f32,
    ) {
        for (index, character) in text.chars().enumerate() {
            let x = index as f32 * advance;
            triangles.extend(self.get(character, style).triangles.iter().map(|triangle| {
                triangle.map(|(p, color)| {
                    (
                        origin + Vec3::new(x + p.x * scale.x, -p.y * scale.y, p.z * scale.z),
                        color,
                    )
                })
            }));
        }
    }

    pub fn get(&self, character: char, style: Style) -> &Glyph {
        let key = (character.to_ascii_uppercase(), style);
        let (key, glyph) = self
            .glyphs
            .get_key_value(&key)
            .unwrap_or_else(|| self.glyphs.get_key_value(&('?', style)).unwrap());
        glyph.get_or_init(|| build(key.0, key.1, &self.announcements))
    }
}

pub(super) struct Paint {
    style: Style,
}

impl Paint {
    pub fn new(style: Style) -> Self {
        Self { style }
    }

    fn front(&self, y: f32) -> Color {
        let stops: &[(f32, [f32; 3])] = match self.style {
            Style::Chrome => &[
                (0., [0.18, 0.40, 0.83]),
                (0.12, [0.76, 0.89, 1.]),
                (0.20, [0.26, 0.56, 0.91]),
                (0.48, [0.29, 0.22, 0.66]),
                (0.72, [0.58, 0.16, 0.66]),
                (1., [0.31, 0.05, 0.40]),
            ],
            Style::Rainbow => &[
                (0., [0.96, 1., 0.02]),
                (0.17, [0.70, 0.93, 0.07]),
                (0.24, [0.98, 1., 0.80]),
                (0.29, [0.40, 0.92, 0.41]),
                (0.65, [0.04, 0.90, 0.91]),
                (0.73, [0.90, 0.99, 1.]),
                (0.82, [0.86, 0.40, 0.60]),
                (1., [0.64, 0.18, 0.38]),
            ],
            Style::Gold => &[(0., [0.64, 0.015, 0.008]), (1., [0.43, 0.01, 0.008])],
        };
        let t = (y / 18.).clamp(0., 1.);
        for pair in stops.windows(2) {
            if t <= pair[1].0 {
                return mix(
                    Color::srgb_from_array(pair[0].1),
                    Color::srgb_from_array(pair[1].1),
                    (t - pair[0].0) / (pair[1].0 - pair[0].0),
                );
            }
        }
        Color::srgb_from_array(stops.last().unwrap().1)
    }

    fn surface(&self, y: f32, normal: Vec3, bevel: bool) -> Color {
        let light = Vec3::new(-0.45, 0.70, 0.85).normalize();
        let diffuse = normal.dot(light).max(0.);
        let base = match (self.style, bevel) {
            (Style::Gold, true) => Color::srgb(1., 0.73, 0.08),
            (Style::Gold, false) => Color::srgb(0.14, 0.23, 0.08),
            (Style::Rainbow, false) => Color::srgb(0.64, 0.23, 0.48),
            (Style::Chrome, false) => Color::srgb(0.17, 0.06, 0.31),
            _ => self.front(y),
        }
        .to_linear();
        let strength = if bevel {
            0.42 + diffuse * 0.65
        } else {
            0.36 + diffuse * 0.42
        };
        let glint = if bevel { diffuse.powi(14) * 0.65 } else { 0. };
        Color::linear_rgb(
            (base.red * strength + glint).min(1.),
            (base.green * strength + glint).min(1.),
            (base.blue * strength + glint).min(1.),
        )
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    Color::linear_rgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

fn polygon(points: &[(f32, f32)]) -> Vec<Vec2> {
    points.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<Vec2> {
    polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}
fn ring() -> Vec<Vec2> {
    polygon(&[
        (3., 0.),
        (12., 0.),
        (15., 3.),
        (15., 15.),
        (12., 18.),
        (3., 18.),
        (0., 15.),
        (0., 3.),
    ])
}
fn counter() -> Vec<Vec2> {
    polygon(&[
        (5., 4.),
        (10., 4.),
        (11., 5.),
        (11., 13.),
        (10., 14.),
        (5., 14.),
        (4., 13.),
        (4., 5.),
    ])
}

fn outline(character: char) -> Vec<Vec<Vec2>> {
    let p: &[(f32, f32)] = match character {
        ' ' => return Vec::new(),
        '0' | 'O' => return vec![ring(), counter()],
        '8' | 'B' => return vec![ring(), rect(4., 4., 7., 3.), rect(4., 11., 7., 3.)],
        '1' => &[
            (5., 0.),
            (10., 0.),
            (10., 14.),
            (14., 14.),
            (14., 18.),
            (1., 18.),
            (1., 14.),
            (5., 14.),
            (5., 5.),
            (1., 7.),
            (0., 4.),
        ],
        '2' => &[
            (1., 0.),
            (12., 0.),
            (15., 3.),
            (15., 7.),
            (4., 14.),
            (15., 14.),
            (15., 18.),
            (0., 18.),
            (0., 12.),
            (11., 5.),
            (11., 4.),
            (0., 4.),
        ],
        '3' => &[
            (0., 0.),
            (11., 0.),
            (15., 3.),
            (15., 15.),
            (12., 18.),
            (0., 18.),
            (0., 14.),
            (10., 14.),
            (11., 13.),
            (11., 11.),
            (5., 11.),
            (5., 7.),
            (11., 7.),
            (11., 5.),
            (10., 4.),
            (0., 4.),
        ],
        '4' => {
            return vec![
                polygon(&[
                    (7., 0.),
                    (15., 0.),
                    (15., 18.),
                    (11., 18.),
                    (11., 12.),
                    (0., 12.),
                    (0., 8.),
                ]),
                polygon(&[(4., 8.), (11., 8.), (11., 3.), (9., 3.)]),
            ];
        }
        '5' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (4., 4.),
            (4., 7.),
            (11., 7.),
            (15., 10.),
            (15., 15.),
            (12., 18.),
            (0., 18.),
            (0., 14.),
            (10., 14.),
            (11., 13.),
            (11., 12.),
            (10., 11.),
            (0., 11.),
        ],
        '6' | '9' => {
            let mut result = vec![
                polygon(&[
                    (4., 0.),
                    (15., 0.),
                    (15., 4.),
                    (5., 4.),
                    (4., 5.),
                    (4., 7.),
                    (12., 7.),
                    (15., 10.),
                    (15., 15.),
                    (12., 18.),
                    (3., 18.),
                    (0., 15.),
                    (0., 4.),
                ]),
                rect(4., 11., 7., 3.),
            ];
            if character == '9' {
                for contour in &mut result {
                    for p in contour {
                        *p = Vec2::new(15., 18.) - *p;
                    }
                }
            }
            return result;
        }
        '7' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (7., 18.),
            (2., 18.),
            (10., 4.),
            (0., 4.),
        ],
        'A' => {
            return vec![
                polygon(&[
                    (0., 18.),
                    (4., 0.),
                    (11., 0.),
                    (15., 18.),
                    (11.5, 18.),
                    (10.5, 13.),
                    (4.5, 13.),
                    (3.5, 18.),
                ]),
                polygon(&[(5.3, 9.), (9.7, 9.), (8.7, 4.), (6.3, 4.)]),
            ];
        }
        'C' => &[
            (4., 0.),
            (15., 0.),
            (15., 4.),
            (5., 4.),
            (4., 5.),
            (4., 13.),
            (5., 14.),
            (15., 14.),
            (15., 18.),
            (4., 18.),
            (0., 14.),
            (0., 4.),
        ],
        'D' => {
            return vec![
                polygon(&[
                    (0., 0.),
                    (11., 0.),
                    (15., 4.),
                    (15., 14.),
                    (11., 18.),
                    (0., 18.),
                ]),
                counter(),
            ];
        }
        'E' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (4., 4.),
            (4., 7.),
            (12., 7.),
            (12., 11.),
            (4., 11.),
            (4., 14.),
            (15., 14.),
            (15., 18.),
            (0., 18.),
        ],
        'F' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (4., 4.),
            (4., 7.),
            (12., 7.),
            (12., 11.),
            (4., 11.),
            (4., 18.),
            (0., 18.),
        ],
        'G' => &[
            (4., 0.),
            (15., 0.),
            (15., 4.),
            (5., 4.),
            (4., 5.),
            (4., 13.),
            (5., 14.),
            (11., 14.),
            (11., 11.),
            (8., 11.),
            (8., 7.),
            (15., 7.),
            (15., 18.),
            (4., 18.),
            (0., 14.),
            (0., 4.),
        ],
        'H' => &[
            (0., 0.),
            (4., 0.),
            (4., 7.),
            (11., 7.),
            (11., 0.),
            (15., 0.),
            (15., 18.),
            (11., 18.),
            (11., 11.),
            (4., 11.),
            (4., 18.),
            (0., 18.),
        ],
        'I' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (9.5, 4.),
            (9.5, 14.),
            (15., 14.),
            (15., 18.),
            (0., 18.),
            (0., 14.),
            (5.5, 14.),
            (5.5, 4.),
            (0., 4.),
        ],
        'J' => &[
            (4., 0.),
            (15., 0.),
            (15., 14.),
            (11., 18.),
            (4., 18.),
            (0., 14.),
            (0., 10.),
            (4., 10.),
            (4., 13.),
            (5., 14.),
            (10., 14.),
            (11., 13.),
            (11., 4.),
            (4., 4.),
        ],
        'K' => &[
            (0., 0.),
            (4., 0.),
            (4., 7.),
            (10., 0.),
            (15., 0.),
            (8., 9.),
            (15., 18.),
            (10., 18.),
            (4., 11.),
            (4., 18.),
            (0., 18.),
        ],
        'L' => &[
            (0., 0.),
            (4., 0.),
            (4., 14.),
            (15., 14.),
            (15., 18.),
            (0., 18.),
        ],
        'M' => &[
            (0., 0.),
            (4., 0.),
            (7.5, 6.),
            (11., 0.),
            (15., 0.),
            (15., 18.),
            (11., 18.),
            (11., 7.),
            (7.5, 12.),
            (4., 7.),
            (4., 18.),
            (0., 18.),
        ],
        'N' => &[
            (0., 0.),
            (4., 0.),
            (11., 11.),
            (11., 0.),
            (15., 0.),
            (15., 18.),
            (11., 18.),
            (4., 7.),
            (4., 18.),
            (0., 18.),
        ],
        'P' | 'R' => {
            return vec![
                polygon(if character == 'P' {
                    &[
                        (0., 0.),
                        (11., 0.),
                        (15., 4.),
                        (15., 8.),
                        (11., 12.),
                        (4., 12.),
                        (4., 18.),
                        (0., 18.),
                    ]
                } else {
                    &[
                        (0., 0.),
                        (11., 0.),
                        (15., 4.),
                        (15., 8.),
                        (11., 11.),
                        (15., 18.),
                        (10., 18.),
                        (6., 12.),
                        (4., 12.),
                        (4., 18.),
                        (0., 18.),
                    ]
                }),
                rect(4., 4., 7., 4.),
            ];
        }
        'Q' => {
            return vec![
                polygon(&[
                    (3., 0.),
                    (12., 0.),
                    (15., 3.),
                    (15., 14.),
                    (13., 16.),
                    (15., 18.),
                    (10., 18.),
                    (8., 17.),
                    (3., 18.),
                    (0., 15.),
                    (0., 3.),
                ]),
                counter(),
            ];
        }
        'S' => &[
            (3., 0.),
            (15., 0.),
            (15., 4.),
            (5., 4.),
            (4., 5.),
            (4., 7.),
            (11., 7.),
            (15., 11.),
            (15., 15.),
            (12., 18.),
            (0., 18.),
            (0., 14.),
            (10., 14.),
            (11., 13.),
            (11., 12.),
            (10., 11.),
            (4., 11.),
            (0., 7.),
            (0., 3.),
        ],
        'T' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (9.5, 4.),
            (9.5, 18.),
            (5.5, 18.),
            (5.5, 4.),
            (0., 4.),
        ],
        'U' => &[
            (0., 0.),
            (4., 0.),
            (4., 13.),
            (5., 14.),
            (10., 14.),
            (11., 13.),
            (11., 0.),
            (15., 0.),
            (15., 14.),
            (11., 18.),
            (4., 18.),
            (0., 14.),
        ],
        'V' => &[
            (0., 0.),
            (4., 0.),
            (7.5, 12.),
            (11., 0.),
            (15., 0.),
            (9.5, 18.),
            (5.5, 18.),
        ],
        'W' => &[
            (0., 0.),
            (3.5, 0.),
            (5., 12.),
            (6., 8.),
            (9., 8.),
            (10., 12.),
            (11.5, 0.),
            (15., 0.),
            (12., 18.),
            (8.5, 18.),
            (7.5, 14.),
            (6.5, 18.),
            (3., 18.),
        ],
        'X' => &[
            (0., 0.),
            (5., 0.),
            (7.5, 5.5),
            (10., 0.),
            (15., 0.),
            (10., 9.),
            (15., 18.),
            (10., 18.),
            (7.5, 12.5),
            (5., 18.),
            (0., 18.),
            (5., 9.),
        ],
        'Y' => &[
            (0., 0.),
            (4.5, 0.),
            (7.5, 6.5),
            (10.5, 0.),
            (15., 0.),
            (9.5, 11.),
            (9.5, 18.),
            (5.5, 18.),
            (5.5, 11.),
        ],
        'Z' => &[
            (0., 0.),
            (15., 0.),
            (15., 4.),
            (5., 14.),
            (15., 14.),
            (15., 18.),
            (0., 18.),
            (0., 14.),
            (10., 4.),
            (0., 4.),
        ],
        '!' => return vec![rect(5., 0., 4., 11.), rect(5., 14., 4., 4.)],
        '.' => return vec![rect(5., 14., 4., 4.)],
        ':' => return vec![rect(5., 4., 4., 4.), rect(5., 14., 4., 4.)],
        ',' => &[(5., 14.), (9., 14.), (9., 18.), (5., 20.), (5., 18.)],
        '-' => return vec![rect(2., 7., 11., 4.)],
        '(' => &[
            (10., 0.),
            (5., 0.),
            (1., 4.),
            (1., 14.),
            (5., 18.),
            (10., 18.),
            (10., 14.),
            (6., 14.),
            (5., 13.),
            (5., 5.),
            (6., 4.),
            (10., 4.),
        ],
        ')' => {
            return outline('(')
                .into_iter()
                .map(|p| p.into_iter().map(|p| Vec2::new(15. - p.x, p.y)).collect())
                .collect();
        }
        _ => {
            return vec![
                polygon(&[
                    (0., 0.),
                    (12., 0.),
                    (15., 3.),
                    (15., 7.),
                    (9., 12.),
                    (5., 12.),
                    (5., 9.),
                    (11., 5.),
                    (11., 4.),
                    (0., 4.),
                ]),
                rect(5., 14., 4., 4.),
            ];
        }
    };
    vec![polygon(p)]
}

fn area(contour: &[Vec2]) -> f32 {
    (0..contour.len())
        .map(|i| contour[i].perp_dot(contour[(i + 1) % contour.len()]))
        .sum::<f32>()
        / 2.
}

fn contains(contour: &[Vec2], p: Vec2) -> bool {
    let mut inside = false;
    for i in 0..contour.len() {
        let a = contour[i];
        let b = contour[(i + 1) % contour.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Winds outer contours and holes in opposite directions.
pub(super) fn orient(contours: &mut [Vec<Vec2>]) {
    for i in 0..contours.len() {
        let hole = (0..contours.len())
            .filter(|&j| i != j && contains(&contours[j], contours[i][0]))
            .count()
            % 2
            == 1;
        if (area(&contours[i]) < 0.) != hole {
            contours[i].reverse();
        }
    }
}

pub(super) fn inset(contour: &[Vec2], amount: f32) -> Vec<Vec2> {
    (0..contour.len())
        .map(|i| {
            let a = contour[(i + contour.len() - 1) % contour.len()];
            let p = contour[i];
            let b = contour[(i + 1) % contour.len()];
            let n0 = (p - a).normalize_or_zero().perp();
            let n1 = (b - p).normalize_or_zero().perp();
            let bisector = (n0 + n1).normalize_or_zero();
            p + bisector * (amount / bisector.dot(n1).max(0.35))
        })
        .collect()
}

fn rounded(contour: &[Vec2], radius: f32) -> Vec<Vec2> {
    let mut points = Vec::new();
    for i in 0..contour.len() {
        let a = contour[(i + contour.len() - 1) % contour.len()];
        let p = contour[i];
        let b = contour[(i + 1) % contour.len()];
        let incoming = (p - a).normalize_or_zero();
        let outgoing = (b - p).normalize_or_zero();
        let turn = incoming.perp_dot(outgoing).atan2(incoming.dot(outgoing));
        if turn.abs() < 0.01 || turn.abs() > 3.13 {
            points.push(p);
            continue;
        }
        let tangent = (turn.abs() / 2.).tan();
        let trim = (radius * tangent)
            .min(p.distance(a) * 0.45)
            .min(p.distance(b) * 0.45);
        let actual_radius = trim / tangent;
        let start = p - incoming * trim;
        let end = p + outgoing * trim;
        let center = start + incoming.perp() * turn.signum() * actual_radius;
        let first = (start - center).to_angle();
        // Circular arcs with less than 0.015 logical-pixel chord error at 4K.
        let step = (2.
            * (1. - 0.015 / actual_radius.max(0.015))
                .clamp(-1., 1.)
                .acos())
        .max(0.05);
        let segments = (turn.abs() / step).ceil().max(1.) as usize;
        points.push(start);
        for segment in 1..segments {
            let angle = first + turn * segment as f32 / segments as f32;
            points.push(center + Vec2::from_angle(angle) * actual_radius);
        }
        points.push(end);
    }
    points
}

fn fill(contours: &[Vec<Vec2>]) -> Vec<[Vec2; 3]> {
    let mut path = Path::builder();
    for contour in contours {
        path.begin(point(contour[0].x, contour[0].y));
        for p in &contour[1..] {
            path.line_to(point(p.x, p.y));
        }
        path.end(true);
    }
    let mut geometry = VertexBuffers::<Vec2, u32>::new();
    FillTessellator::new()
        .tessellate_path(
            &path.build(),
            &FillOptions::default().with_fill_rule(FillRule::EvenOdd),
            &mut BuffersBuilder::new(&mut geometry, |v: FillVertex| {
                Vec2::new(v.position().x, v.position().y)
            }),
        )
        .unwrap();
    geometry
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|face| face.map(|i| geometry.vertices[i as usize]))
        .collect()
}

pub(super) fn extrude(contours: &[Vec<Vec2>], paint: &Paint) -> Glyph {
    if contours.is_empty() {
        return Glyph {
            triangles: Vec::new(),
            fragments: OnceLock::new(),
        };
    }
    let mut contours = contours.to_vec();
    orient(&mut contours);
    let front: Vec<_> = contours.iter().map(|p| inset(p, 0.55)).collect();
    let (offset, depth) = if paint.style == Style::Rainbow {
        (Vec2::new(1.6, 2.4), 3.2)
    } else {
        (Vec2::new(0.85, 1.15), 1.5)
    };
    let mut triangles = Vec::new();
    for mut face in fill(&contours) {
        if (face[1] - face[0]).perp_dot(face[2] - face[0]) < 0. {
            face.swap(1, 2);
        }
        push(
            &mut triangles,
            face.map(|p| {
                (
                    (p + offset).extend(-depth),
                    paint.surface(p.y, -Vec3::Z, false),
                )
            }),
        );
    }
    for (contour, inner) in contours.iter().zip(&front) {
        for i in 0..contour.len() {
            let j = (i + 1) % contour.len();
            let a = contour[i];
            let b = contour[j];
            for (vertices, bevel) in [
                (
                    [
                        a.extend(0.),
                        b.extend(0.),
                        (b + offset).extend(-depth),
                        (a + offset).extend(-depth),
                    ],
                    false,
                ),
                (
                    [
                        a.extend(0.),
                        b.extend(0.),
                        inner[j].extend(0.42),
                        inner[i].extend(0.42),
                    ],
                    true,
                ),
            ] {
                let faces = if bevel {
                    [[0, 2, 1], [0, 3, 2]]
                } else {
                    [[0, 1, 2], [0, 2, 3]]
                };
                let source_normal = (vertices[faces[0][1]] - vertices[0])
                    .cross(vertices[faces[0][2]] - vertices[0])
                    .normalize_or_zero();
                let normal = Vec3::new(-source_normal.x, source_normal.y, -source_normal.z);
                let corners = vertices.map(|p| (p, paint.surface(p.y, normal, bevel)));
                for face in faces {
                    push(&mut triangles, face.map(|i| corners[i]));
                }
            }
        }
    }
    // Split fronts at analytic colour-band boundaries, not a source bitmap grid.
    let bands = 36;
    let lo_y = front
        .iter()
        .flatten()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);
    let hi_y = front
        .iter()
        .flatten()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max);
    for face in fill(&front) {
        for band in 0..bands {
            let lo = lo_y + (hi_y - lo_y) * band as f32 / bands as f32;
            let hi = lo_y + (hi_y - lo_y) * (band + 1) as f32 / bands as f32;
            let mut points: Vec<_> = face
                .iter()
                .map(|p| (p.extend(0.42), paint.front(p.y)))
                .collect();
            points = clip(&points, 1, lo, 1.);
            points = clip(&points, 1, hi, -1.);
            for i in 1..points.len().saturating_sub(1) {
                let a = points[0].0;
                let b = points[i + 1].0;
                let c = points[i].0;
                let mut triangle = [a, b, c].map(|p| (p, paint.front(p.y)));
                if (triangle[1].0 - triangle[0].0)
                    .cross(triangle[2].0 - triangle[0].0)
                    .z
                    > 0.
                {
                    triangle.swap(1, 2);
                }
                push(&mut triangles, triangle);
            }
        }
    }
    triangles.shrink_to_fit();
    Glyph {
        triangles,
        fragments: if paint.style == Style::Chrome {
            OnceLock::new()
        } else {
            OnceLock::from(Box::default())
        },
    }
}

fn push(triangles: &mut Vec<Triangle>, triangle: Triangle) {
    if (triangle[1].0 - triangle[0].0)
        .cross(triangle[2].0 - triangle[0].0)
        .length_squared()
        > 1e-10
    {
        triangles.push(triangle);
    }
}

fn clip(points: &[(Vec3, Color)], axis: usize, bound: f32, sign: f32) -> Vec<(Vec3, Color)> {
    let mut output = Vec::new();
    for i in 0..points.len() {
        let (a, ca) = points[i];
        let (b, cb) = points[(i + 1) % points.len()];
        let da = (a[axis] - bound) * sign;
        let db = (b[axis] - bound) * sign;
        if da >= 0. {
            output.push((a, ca));
        }
        if (da >= 0.) != (db >= 0.) {
            let t = da / (da - db);
            output.push((a.lerp(b, t), mix(ca, cb, t)));
        }
    }
    output
}

fn fragments(triangles: &[Triangle]) -> Vec<Fragment> {
    let mut output = Vec::new();
    for triangle in triangles {
        let min = triangle
            .iter()
            .map(|(p, _)| p.truncate())
            .fold(Vec2::splat(f32::INFINITY), Vec2::min)
            .floor();
        let max = triangle
            .iter()
            .map(|(p, _)| p.truncate())
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
            .ceil();
        for y in min.y as i32..max.y as i32 {
            for x in min.x as i32..max.x as i32 {
                let mut points = triangle.to_vec();
                for (axis, bound, sign) in [
                    (0, x as f32, 1.),
                    (0, x as f32 + 1., -1.),
                    (1, y as f32, 1.),
                    (1, y as f32 + 1., -1.),
                ] {
                    points = clip(&points, axis, bound, sign);
                }
                for i in 1..points.len().saturating_sub(1) {
                    let triangle = [points[0], points[i], points[i + 1]];
                    if (triangle[1].0 - triangle[0].0)
                        .cross(triangle[2].0 - triangle[0].0)
                        .length_squared()
                        > 1e-10
                    {
                        output.push(Fragment {
                            triangle,
                            center: Vec2::new(x as f32 + 0.5, y as f32 + 0.5),
                        });
                    }
                }
            }
        }
    }
    output
}

pub(crate) fn mesh(triangles: Vec<Triangle>) -> Mesh {
    let mut positions = Vec::with_capacity(triangles.len() * 3);
    let mut normals = Vec::with_capacity(triangles.len() * 3);
    let mut colors = Vec::with_capacity(triangles.len() * 3);
    for triangle in triangles {
        let normal = (triangle[1].0 - triangle[0].0)
            .cross(triangle[2].0 - triangle[0].0)
            .normalize_or_zero()
            .to_array();
        for (position, color) in triangle {
            positions.push(position.to_array());
            normals.push(normal);
            colors.push(color.to_linear().to_f32_array());
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

#[cfg(test)]
#[path = "../tests/geometry.rs"]
mod tests;
