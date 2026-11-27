//! Multi-theme background system for vim-sandbox.
//!
//! Each theme lives in its own file under this directory and implements the
//! [`Theme`] trait.  [`ThemeRegistry`] holds all registered themes and lets
//! the user cycle through them with `:theme`, `:themenext` / `:themeprev`.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

// ============================================================================
// Theme trait
// ============================================================================

/// Trait for an animated background theme.
pub trait Theme: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng);
    fn draw(&mut self, buf: &mut Buffer, area: Rect);
    fn resize(&mut self, area: Rect);
}

// ============================================================================
// Theme registry — holds multiple themes and lets the user cycle through them
// ============================================================================

pub struct ThemeRegistry {
    themes: Vec<Box<dyn Theme>>,
    current: usize,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self {
            themes: vec![
                Box::new(crate::ui::themes::spider_lily::SpiderLily::new()),
                Box::new(crate::ui::themes::sakura_night::SakuraNight::new()),
                Box::new(crate::ui::themes::abyssal_bloom::AbyssalBloom::new()),
                Box::new(crate::ui::themes::firefly_dusk::FireflyDusk::new()),
                Box::new(crate::ui::themes::aurora_void::AuroraVoid::new()),
                Box::new(crate::ui::themes::koi_dreams::KoiDreams::new()),
                Box::new(crate::ui::themes::bamboo_whisper::BambooWhisper::new()),
                Box::new(crate::ui::themes::nebula_heart::NebulaHeart::new()),
            ],
            current: 0,
        }
    }

    /// Register an additional theme.
    pub fn add_theme(&mut self, theme: Box<dyn Theme>) {
        self.themes.push(theme);
    }

    pub fn current(&self) -> &dyn Theme {
        self.themes[self.current].as_ref()
    }

    pub fn current_mut(&mut self) -> &mut dyn Theme {
        self.themes[self.current].as_mut()
    }

    pub fn current_name(&self) -> &'static str {
        self.themes[self.current].name()
    }

    pub fn resize_all(&mut self, area: Rect) {
        for theme in &mut self.themes {
            theme.resize(area);
        }
    }

    /// Change le thème actif par nom. Retourne `true` si trouvé et appliqué.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if let Some(pos) = self.themes.iter().position(|t| t.name() == name) {
            self.current = pos;
            return true;
        }
        false
    }

    /// Passe au thème suivant (boucle).
    pub fn next_theme(&mut self) {
        self.current = (self.current + 1) % self.themes.len();
    }

    /// Passe au thème précédent (boucle).
    pub fn prev_theme(&mut self) {
        self.current = if self.current == 0 {
            self.themes.len() - 1
        } else {
            self.current - 1
        };
    }
}

// ============================================================================
// Braille canvas — shared drawing primitives
// ============================================================================
// One terminal cell is 2 braille dots wide and 4 dots tall.  Drawing directly
// in dot space keeps circles physically round and lets many strokes share the
// same cell.

const BRAILLE_BASE: u32 = 0x2800;

const BRAILLE_BITS: [[u8; 4]; 2] = [
    [0x01, 0x08, 0x10, 0x40], // left column: dots 1, 3, 5, 7
    [0x02, 0x04, 0x20, 0x80], // right column: dots 2, 4, 6, 8
];

#[inline]
fn plot_dot(buf: &mut Buffer, area: Rect, dx: i32, dy: i32, color: (u8, u8, u8)) {
    plot_dot_impl(buf, area, dx, dy, color, false);
}

/// Like [`plot_dot`], but always replaces the colour of the cell, even with a
/// darker one. Used for the flower heart, which must stay dark.
#[inline]
fn plot_dot_force(buf: &mut Buffer, area: Rect, dx: i32, dy: i32, color: (u8, u8, u8)) {
    plot_dot_impl(buf, area, dx, dy, color, true);
}

fn plot_dot_impl(
    buf: &mut Buffer,
    area: Rect,
    dx: i32,
    dy: i32,
    color: (u8, u8, u8),
    force: bool,
) {
    if dx < 0 || dy < 0 {
        return;
    }
    let cx = dx >> 1;
    let cy = dy >> 2;
    if cx >= area.width as i32 || cy >= area.height as i32 {
        return;
    }

    let bit = BRAILLE_BITS[(dx & 1) as usize][(dy & 3) as usize];
    let cell = &mut buf[(cx as u16, cy as u16)];

    // Keep the brightest stroke when two strokes share a cell: a dark petal
    // base must never punch a hole through a glowing tip.
    if !force {
        if let Color::Rgb(r, g, b) = cell.fg {
            let old = r as u16 + g as u16 + b as u16;
            let new = color.0 as u16 + color.1 as u16 + color.2 as u16;
            if old > new {
                return;
            }
        }
    }

    let mut pattern = cell
        .symbol()
        .chars()
        .next()
        .and_then(|ch| {
            let value = ch as u32;
            if (BRAILLE_BASE..=BRAILLE_BASE + 0xFF).contains(&value) {
                Some((value - BRAILLE_BASE) as u8)
            } else {
                None
            }
        })
        .unwrap_or(0);

    pattern |= bit;
    cell.set_char(char::from_u32(BRAILLE_BASE + pattern as u32).unwrap_or(' '));
    cell.set_fg(Color::Rgb(color.0, color.1, color.2));
}

/// Paint a round dot of `radius` dots at dot-space position (dx, dy).
#[inline]
fn stamp_disc(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
) {
    stamp_disc_impl(buf, area, dx, dy, radius, color, false);
}

/// A disc that is allowed to darken the dots it covers.
#[inline]
fn stamp_disc_force(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
) {
    stamp_disc_impl(buf, area, dx, dy, radius, color, true);
}

fn stamp_disc_impl(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
    force: bool,
) {
    let r = radius.max(0.5);
    let ri = r.ceil() as i32;
    let cx = dx.round() as i32;
    let cy = dy.round() as i32;
    let r2 = r * r;

    for oy in -ri..=ri {
        for ox in -ri..=ri {
            if (ox * ox + oy * oy) as f32 <= r2 {
                if force {
                    plot_dot_force(buf, area, cx + ox, cy + oy, color);
                } else {
                    plot_dot(buf, area, cx + ox, cy + oy, color);
                }
            }
        }
    }
}

/// Draw a tapered line in dot space.
#[allow(clippy::too_many_arguments)]
fn draw_dot_line(
    buf: &mut Buffer,
    area: Rect,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r0: f32,
    r1: f32,
    c0: (u8, u8, u8),
    c1: (u8, u8, u8),
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let distance = (dx * dx + dy * dy).sqrt();
    let steps = distance.ceil().max(1.0) as usize;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let radius = r0 + (r1 - r0) * t;
        let color = lerp_color(c0, c1, t);
        stamp_disc(buf, area, x0 + dx * t, y0 + dy * t, radius, color);
    }
}

// ============================================================================
// Small math / color helpers
// ============================================================================

const TAU: f32 = std::f32::consts::TAU;

#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

#[inline]
fn mul_color(c: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    (
        (c.0 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * factor).clamp(0.0, 255.0) as u8,
    )
}

#[inline]
fn tint(c: (u8, u8, u8), red: f32, green_blue: f32) -> (u8, u8, u8) {
    (
        (c.0 as f32 * red).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * green_blue).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * green_blue).clamp(0.0, 255.0) as u8,
    )
}

#[inline]
fn hash(mut x: u32) -> u32 {
    x = x.wrapping_mul(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x
}

// ============================================================================
// Shared palette — base colors every theme can reuse
// ============================================================================

pub const C_DEEP_BG: (u8, u8, u8) = (0x05, 0x05, 0x08);
pub const C_MOSS: (u8, u8, u8) = (0x0E, 0x1E, 0x10);
pub const C_MOSS_LIGHT: (u8, u8, u8) = (0x1C, 0x30, 0x1C);
pub const C_STEM_DARK: (u8, u8, u8) = (0x12, 0x2A, 0x16);
pub const C_STEM_LIGHT: (u8, u8, u8) = (0x30, 0x58, 0x32);
pub const C_PETAL_DEEP: (u8, u8, u8) = (0x5E, 0x04, 0x0A);
pub const C_PETAL_MID: (u8, u8, u8) = (0xC0, 0x10, 0x1C);
pub const C_PETAL_TIP: (u8, u8, u8) = (0xFF, 0x36, 0x3A);
pub const C_PETAL_HOT: (u8, u8, u8) = (0xFF, 0x8A, 0x70);
pub const C_STAMEN: (u8, u8, u8) = (0x8A, 0x5C, 0x24);
pub const C_ANTHER: (u8, u8, u8) = (0xFF, 0xDC, 0x84);
pub const C_GLOW: (u8, u8, u8) = (0x46, 0x04, 0x0A);
pub const C_FALLING: (u8, u8, u8) = (0x94, 0x0C, 0x12);
pub const C_SKY: (u8, u8, u8) = (0x0A, 0x0A, 0x1E);
pub const C_PINK: (u8, u8, u8) = (0xFF, 0xC0, 0xCB);
pub const C_PINK_DEEP: (u8, u8, u8) = (0xFF, 0x69, 0xB4);
pub const C_MOON: (u8, u8, u8) = (0xF0, 0xF0, 0xE0);
pub const C_BRANCH: (u8, u8, u8) = (0x2A, 0x1A, 0x1A);
pub const C_CYAN: (u8, u8, u8) = (0x00, 0xFF, 0xB3);
pub const C_CYAN_DEEP: (u8, u8, u8) = (0x00, 0x80, 0x80);
pub const C_MAGENTA: (u8, u8, u8) = (0xFF, 0x00, 0xFF);
pub const C_GOLD: (u8, u8, u8) = (0xFF, 0xD7, 0x00);

// ============================================================================
// Module declarations — one file per theme
// ============================================================================

pub mod spider_lily;
pub mod sakura_night;
pub mod abyssal_bloom;
pub mod firefly_dusk;
pub mod aurora_void;
pub mod koi_dreams;
pub mod bamboo_whisper;
pub mod nebula_heart;
