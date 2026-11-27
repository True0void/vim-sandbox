//! Bamboo Whisper (竹の囁き) — bamboo forest swaying in the breeze.

use crate::ui::themes::{plot_dot, stamp_disc, draw_dot_line, hash, mul_color, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_FOREST: (u8, u8, u8) = (0x0A, 0x1A, 0x0A);
const C_BAMBOO: (u8, u8, u8) = (0x4A, 0x7C, 0x3A);
const C_LEAF: (u8, u8, u8) = (0x8A, 0xC0, 0x6A);
const C_STALK: (u8, u8, u8) = (0x2A, 0x4A, 0x2A);

#[derive(Copy, Clone)]
struct Stalk {
    x: f32,
    height: f32,
    phase: f32,
    sway: f32,
}

pub struct BambooWhisper {
    stalks: Vec<Stalk>,
    time: f32,
    width: u16,
    height: u16,
}

impl BambooWhisper {
    pub fn new() -> Self {
        Self {
            stalks: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
        }
    }
}

impl Theme for BambooWhisper {
    fn name(&self) -> &'static str {
        "bamboo-whisper"
    }

    fn description(&self) -> &'static str {
        "竹の囁き — bamboo forest swaying in the breeze"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.stalks.clear();
        self.time = 0.0;
    }

    fn update(&mut self, dt: f32, _rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.8 + 0.2 * (self.time * 0.25).sin();

        // Forest background
        for x in 0..area.width {
            for y in 0..area.height - 2 {
                let color = mul_color(C_FOREST, breath * 0.5);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // Bamboo stalks
        if self.stalks.is_empty() {
            for i in 0..8 {
                self.stalks.push(Stalk {
                    x: i as f32 * self.width as f32 / 8.0 + 5.0,
                    height: self.height as f32 * 0.7,
                    phase: i as f32,
                    sway: 0.0,
                });
            }
        }

        for s in &self.stalks {
            let sway = (self.time * 0.5 + s.phase).sin() * 0.5;
            let sx = s.x + sway;
            for y in 0..(s.height as i32) {
                let color = if y % 4 == 0 { C_STALK } else { C_BAMBOO };
                plot_dot(buf, area, sx as i32 * 2, (area.height as i32 - 2 - y) * 4, color);
            }
            // Leaves
            for i in 0..3 {
                let ly = (s.height * 0.7 + i as f32 * 0.3) as i32;
                if ly > 0 && ly < area.height as i32 - 2 {
                    let lx = sx + (i as f32 - 1.0) * 1.5;
                    stamp_disc(buf, area, lx * 2.0, ly as f32 * 4.0, 0.5, C_LEAF);
                }
            }
        }
    }
}
