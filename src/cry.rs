//! Jaguar CRY color decoding, converting chroma and intensity into RGB colors.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// The Jaguar feedback bitmap size shared by the warp melts.
pub(crate) const FEEDBACK_WIDTH: usize = 384;
pub(crate) const FEEDBACK_HEIGHT: usize = 240;
/// Like gofeed, retain 15 Hz checkpoints and interpolate their presentation at 60 Hz.
pub(crate) const TICKS_PER_FRAME: u16 = 4;

/// A black, nearest-sampled feedback bitmap.
pub(crate) fn feedback_image() -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: FEEDBACK_WIDTH as u32,
            height: FEEDBACK_HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

/// Overlay-camera depth for full-screen bitmaps and rewards, behind its HUD and messages.
pub(crate) const BEHIND_HUD: f32 = -200.;

/// Scales of a unit quad for the black backdrop and the feedback bitmap, which
/// fills the 240-line height and never exceeds its original size.
pub(crate) fn feedback_scales(viewport: f32) -> [Vec3; 2] {
    let fit = (viewport / FEEDBACK_WIDTH as f32).min(1.);
    [
        Vec3::new(viewport, 240., 1.),
        Vec3::new(
            FEEDBACK_WIDTH as f32 * fit,
            FEEDBACK_HEIGHT as f32 * fit,
            1.,
        ),
    ]
}

/// Chooses `ofade`'s -3..4-brad angle from the scene without consuming gameplay randomness.
pub(crate) fn ofade_angle(source: u32) -> i32 {
    ((source.wrapping_mul(0x9e3779b9) >> 16) & 7) as i32 - 3
}

/// Settles a melt's rotation one brad toward zero every four one-based frames.
pub(crate) fn settle(angle: i32, frame: u32) -> i32 {
    angle.signum() * (angle.abs() - (frame / 4) as i32).max(0)
}

// Adapted from Virtual Jaguar's tom.c under GPLv3, with red row 8 corrected to match MAME.
// (C) 2010 Underground Software; upstream authors and source are listed in NOTICE.
#[rustfmt::skip]
pub(crate) const RED: [[u8; 16]; 16] = [
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    [34,34,34,34,34,34,34,34,34,34,34,34,34,34,19,0],
    [68,68,68,68,68,68,68,68,68,68,68,68,64,43,21,0],
    [102,102,102,102,102,102,102,102,102,102,102,95,71,47,23,0],
    [135,135,135,135,135,135,135,135,135,135,130,104,78,52,26,0],
    [169,169,169,169,169,169,169,169,169,170,141,113,85,56,28,0],
    [203,203,203,203,203,203,203,203,203,183,153,122,91,61,30,0],
    [237,237,237,237,237,237,237,237,230,197,164,131,98,65,32,0],
    [255,255,255,255,255,255,255,255,247,214,181,148,115,82,49,17],
    [255,255,255,255,255,255,255,255,255,235,204,173,143,112,81,51],
    [255,255,255,255,255,255,255,255,255,255,227,198,170,141,113,85],
    [255,255,255,255,255,255,255,255,255,255,249,223,197,171,145,119],
    [255,255,255,255,255,255,255,255,255,255,255,248,224,200,177,153],
    [255,255,255,255,255,255,255,255,255,255,255,255,252,230,208,187],
    [255,255,255,255,255,255,255,255,255,255,255,255,255,255,240,221],
    [255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],
];

#[rustfmt::skip]
pub(crate) const GREEN: [[u8; 16]; 16] = [
    [0,17,34,51,68,85,102,119,136,153,170,187,204,221,238,255],
    [0,19,38,57,77,96,115,134,154,173,192,211,231,250,255,255],
    [0,21,43,64,86,107,129,150,172,193,215,236,255,255,255,255],
    [0,23,47,71,95,119,142,166,190,214,238,255,255,255,255,255],
    [0,26,52,78,104,130,156,182,208,234,255,255,255,255,255,255],
    [0,28,56,85,113,141,170,198,226,255,255,255,255,255,255,255],
    [0,30,61,91,122,153,183,214,244,255,255,255,255,255,255,255],
    [0,32,65,98,131,164,197,230,255,255,255,255,255,255,255,255],
    [0,32,65,98,131,164,197,230,255,255,255,255,255,255,255,255],
    [0,30,61,91,122,153,183,214,244,255,255,255,255,255,255,255],
    [0,28,56,85,113,141,170,198,226,255,255,255,255,255,255,255],
    [0,26,52,78,104,130,156,182,208,234,255,255,255,255,255,255],
    [0,23,47,71,95,119,142,166,190,214,238,255,255,255,255,255],
    [0,21,43,64,86,107,129,150,172,193,215,236,255,255,255,255],
    [0,19,38,57,77,96,115,134,154,173,192,211,231,250,255,255],
    [0,17,34,51,68,85,102,119,136,153,170,187,204,221,238,255],
];

#[rustfmt::skip]
pub(crate) const BLUE: [[u8; 16]; 16] = [
    [255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],
    [255,255,255,255,255,255,255,255,255,255,255,255,255,255,240,221],
    [255,255,255,255,255,255,255,255,255,255,255,255,252,230,208,187],
    [255,255,255,255,255,255,255,255,255,255,255,248,224,200,177,153],
    [255,255,255,255,255,255,255,255,255,255,249,223,197,171,145,119],
    [255,255,255,255,255,255,255,255,255,255,227,198,170,141,113,85],
    [255,255,255,255,255,255,255,255,255,235,204,173,143,112,81,51],
    [255,255,255,255,255,255,255,255,247,214,181,148,115,82,49,17],
    [237,237,237,237,237,237,237,237,230,197,164,131,98,65,32,0],
    [203,203,203,203,203,203,203,203,203,183,153,122,91,61,30,0],
    [169,169,169,169,169,169,169,169,169,170,141,113,85,56,28,0],
    [135,135,135,135,135,135,135,135,135,135,130,104,78,52,26,0],
    [102,102,102,102,102,102,102,102,102,102,102,95,71,47,23,0],
    [68,68,68,68,68,68,68,68,68,68,68,68,64,43,21,0],
    [34,34,34,34,34,34,34,34,34,34,34,34,34,34,19,0],
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];

pub(crate) fn color(hue: u8, intensity: u8) -> Color {
    let cyan = (hue >> 4) as usize;
    let red = (hue & 15) as usize;
    let channel =
        |table: &[[u8; 16]; 16]| ((u16::from(table[cyan][red]) * u16::from(intensity)) >> 8) as u8;
    Color::srgb_u8(channel(&RED), channel(&GREEN), channel(&BLUE))
}

#[derive(Default)]
pub(crate) struct FeedbackPreview {
    key: Option<(u64, u32)>,
    pixels: Vec<u16>,
}

impl FeedbackPreview {
    pub(crate) fn get_or_update(
        &mut self,
        key: (u64, u32),
        build: impl FnOnce() -> Vec<u16>,
    ) -> &[u16] {
        if self.key != Some(key) {
            self.pixels = build();
            self.key = Some(key);
        }
        &self.pixels
    }
}

// Blend decoded RGB, never packed CRY hues. Every fourth tick is an exact feedback frame.
pub(crate) fn write_feedback(image: &mut Image, pixels: &[u16], next: &[u16], phase: u32) {
    for ((pixel, next), rgba) in pixels
        .iter()
        .zip(next)
        .zip(image.data.as_mut().unwrap().as_chunks_mut::<4>().0)
    {
        for (channel, table) in [&RED, &GREEN, &BLUE].iter().enumerate() {
            let decode = |pixel: u16| {
                (u32::from(table[(pixel >> 12) as usize][((pixel >> 8) & 15) as usize])
                    * u32::from(pixel & 255))
                    >> 8
            };
            rgba[channel] = ((decode(*pixel) * (4 - phase) + decode(*next) * phase) / 4) as u8;
        }
        rgba[3] = 255;
    }
}

pub(crate) fn feedback(
    pixels: &mut Vec<u16>,
    scratch: &mut Vec<u16>,
    size: UVec2,
    angle: i32,
    scale: Vec2,
    fade: i32,
) {
    feedback_from(pixels, scratch, size, angle, scale, fade, Vec2::ZERO);
}

pub(crate) fn feedback_from(
    pixels: &mut Vec<u16>,
    scratch: &mut Vec<u16>,
    size: UVec2,
    angle: i32,
    scale: Vec2,
    fade: i32,
    offset: Vec2,
) {
    let sin = crate::sines::sine(angle) as f32 / 256.;
    let cos = crate::sines::sine(angle + 64) as f32 / 256.;
    let center = size.as_vec2() / 2.;
    let x_step = Vec2::new(cos, sin) * scale.x;
    for y in 0..size.y as usize {
        let source = Vec2::new(-center.x, y as f32 - center.y);
        let mut source = Vec2::new(
            source.x * cos * scale.x - source.y * sin * scale.y,
            source.x * sin * scale.x + source.y * cos * scale.y,
        ) + center
            + offset;
        for x in 0..size.x as usize {
            let sx = source.x.floor() as i32;
            let sy = source.y.floor() as i32;
            let pixel = if (0..size.x as i32).contains(&sx) && (0..size.y as i32).contains(&sy) {
                pixels[sy as usize * size.x as usize + sx as usize]
            } else {
                0
            };
            let intensity = ((i32::from(pixel & 255) * 65536 + fade) >> 16).clamp(0, 255) as u16;
            scratch[y * size.x as usize + x] = (pixel & 0xff00) | intensity;
            source += x_step;
        }
    }
    std::mem::swap(pixels, scratch);
}

#[cfg(test)]
#[path = "tests/cry.rs"]
mod tests;
