use super::*;
use clap::ValueEnum;

#[test]
fn glyph_cache_starts_without_fragmentation_and_keeps_no_spare_triangle_capacity() {
    let glyphs = Glyphs::default();
    for glyph in glyphs.glyphs.values().filter_map(OnceLock::get) {
        assert_eq!(glyph.triangles.len(), glyph.triangles.capacity());
        assert!(glyph.fragments.get().is_none_or(|f| f.is_empty()));
    }
}

#[test]
fn score_bodies_have_more_depth_and_visible_extrusion_than_announcements() {
    let contours = outline('0');
    let score = extrude(&contours, &Paint::new(Style::Rainbow));
    let message = extrude(&contours, &Paint::new(Style::Chrome));
    let bounds = |glyph: &Glyph| {
        glyph.triangles.iter().flatten().map(|(p, _)| *p).fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        )
    };
    let (score_lo, score_hi) = bounds(&score);
    let (message_lo, message_hi) = bounds(&message);
    assert!(score_lo.z < message_lo.z * 2.);
    assert!(score_hi.x > message_hi.x + 0.5);
    assert!(score_hi.y > message_hi.y + 1.);
    assert_eq!(score_hi.z, message_hi.z);
}

#[test]
fn rounded_contours_follow_circular_arcs_with_crisp_opaque_geometry() {
    let square = rect(0., 0., 15., 18.);
    let curve = rounded(&square, 1.25);
    assert!(curve.len() > square.len() * 3);
    let center = Vec2::splat(1.25);
    let corner: Vec<_> = curve
        .iter()
        .filter(|p| p.x <= 1.25 && p.y <= 1.25)
        .collect();
    assert!(corner.len() > 3);
    assert!(
        corner
            .iter()
            .all(|p| (p.distance(center) - 1.25).abs() < 1e-5)
    );
    let original = super::super::font::original_contours();
    assert!(rounded(&original[&'O'][0], 1.25).len() > original[&'O'][0].len());
}

#[test]
fn all_catalog_letters_have_opaque_finite_beveled_geometry_with_real_depth() {
    let glyphs = Glyphs::default();
    let announcements = super::super::lettering::contours();
    for id in crate::messages::Id::value_variants() {
        for c in id.text().chars().filter(|c| !c.is_whitespace()) {
            assert!(
                announcements.contains_key(&c),
                "missing rounded announcement glyph {c}"
            );
            assert!(
                glyphs.glyphs.contains_key(&(c, Style::Chrome)),
                "missing {c}"
            );
            for style in [Style::Chrome, Style::Rainbow, Style::Gold] {
                let glyph = glyphs.get(c, style);
                assert!(!glyph.triangles.is_empty(), "empty {c:?}/{style:?}");
                assert!(
                    glyph
                        .triangles
                        .iter()
                        .flatten()
                        .all(|(p, color)| p.is_finite() && color.to_linear().alpha == 1.)
                );
                assert!(glyph.triangles.iter().flatten().any(|(p, _)| p.z > 0.));
                assert!(glyph.triangles.iter().flatten().any(|(p, _)| p.z < -1.));
            }
        }
    }
    assert!(glyphs.get(' ', Style::Chrome).triangles.is_empty());
}

#[test]
fn digit_and_letter_counters_stay_open_after_beveling() {
    let glyphs = Glyphs::default();
    let inside = |p: Vec2, t: &Triangle| {
        let signs: Vec<_> = (0..3)
            .map(|i| {
                (t[(i + 1) % 3].0 - t[i].0)
                    .truncate()
                    .perp_dot(p - t[i].0.truncate())
            })
            .collect();
        signs.iter().all(|&v| v >= 0.) || signs.iter().all(|&v| v <= 0.)
    };
    for (c, p) in [
        ('0', Vec2::new(7.5, 9.)),
        ('O', Vec2::new(7.5, 9.)),
        ('A', Vec2::new(7.5, 6.)),
        ('8', Vec2::new(7.5, 5.5)),
        ('8', Vec2::new(7.5, 12.5)),
        ('P', Vec2::new(7.5, 6.)),
    ] {
        let glyph = glyphs.get(c, Style::Chrome);
        assert!(
            !glyph
                .triangles
                .iter()
                .filter(|t| t.iter().all(|(p, _)| p.z > 0.4))
                .any(|t| inside(p, t)),
            "filled counter in {c}"
        );
    }
}

#[test]
fn fragmentation_preserves_surface_area_depth_and_fixed_cell_bounds() {
    let glyphs = Glyphs::default();
    let glyph = glyphs.get('A', Style::Chrome);
    let area = |t: &Triangle| (t[1].0 - t[0].0).cross(t[2].0 - t[0].0).length() / 2.;
    let whole: f32 = glyph.triangles.iter().map(area).sum();
    assert!(glyph.fragments.get().is_none());
    let pieces: f32 = glyph.fragments().iter().map(|f| area(&f.triangle)).sum();
    assert!((whole - pieces).abs() < whole * 0.0001);
    assert!(glyph.fragments().iter().all(|f| {
        f.triangle
            .iter()
            .all(|(p, _)| (p.truncate() - f.center).abs().max_element() <= 0.501)
    }));
    assert!(
        glyph
            .fragments()
            .iter()
            .flat_map(|f| f.triangle)
            .any(|(p, _)| p.z < 0.)
    );
    assert!(
        glyph
            .fragments()
            .iter()
            .flat_map(|f| f.triangle)
            .any(|(p, _)| p.z > 0.)
    );
}

#[test]
fn opaque_mesh_has_finite_normals_and_has_no_low_resolution_texture_coordinates() {
    use bevy::mesh::VertexAttributeValues;
    let glyphs = Glyphs::default();
    let mesh = mesh(
        glyphs
            .get('0', Style::Rainbow)
            .triangles
            .iter()
            .map(|t| t.map(|(p, c)| (Vec3::new(p.x, -p.y, p.z), c)))
            .collect(),
    );
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!()
    };
    assert!(normals.iter().all(|p| Vec3::from_array(*p).is_finite()));
    assert!(normals.iter().any(|p| p[2] > 0.99));
    assert!(normals.iter().any(|p| p[2] < -0.99));
    assert!(normals.iter().any(|p| p[0].abs() > 0.5));
    assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_none());
}

#[test]
fn only_glyphs_drawn_during_play_are_built_upfront() {
    let glyphs = Glyphs::default();
    let built = |c, style| glyphs.glyphs[&(c, style)].get().is_some();
    assert!(built('S', Style::Chrome) && built('7', Style::Rainbow));
    assert!(!built('S', Style::Rainbow) && !built('P', Style::Gold));
    assert!(!glyphs.get('P', Style::Gold).triangles.is_empty());
    assert!(built('P', Style::Gold));
}
