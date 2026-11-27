//! Willow Tears (垂柳の涙) — weeping willow branches with falling droplets.

use crate::ui::themes::{plot_dot, stamp_disc, draw_dot_line, hash, mul_color, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_WILLOW: (u8, u8, u8) = (0x1A, 0x2A, 0x1A);
const C_LEAF: (u8, u8, u8) = (0x4A, 0x7C, 0x3A);
const C_LEAF_LIGHT: (u8, u8, u8) = (0x8A, 0xC0, 0x6A);
const C_DROP: (u8, u8, u8) = (0xC0, 0xE0, 0xFF);
const C_BARK: (u8, u8, u8) = (0x3A, 0x2A, 0x1A);

#[derive(Copy, Clone)]
struct Branch {
    x: f32,
    height: f32,
    phase: f32,
    droop: f32,
}

#[derive(Copy, Clone)]
struct Tear {
    x: f32,
    y: f32,
    vy: f32,
    life: f32,
}

pub struct WillowTears {
    branches: Vec<Branch>,
    tears: Vec<Tear>,
    time: f32,
    width: u16,
    height: u16,
}

impl WillowTears {
    pub fn new() -> Self {
        Self {
            branches: Vec::new(),
            tears: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
        }
    }
}

impl Theme for WillowTears {
    fn name(&self) -> &'static str {
        "willow-tears"
    }

    fn description(&self) -> &'static str {
        "垂柳の涙 — weeping willow branches with falling droplets"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.branches.clear();
        self.tears.clear();
        self.time = 0.0;
        for i in 0..5 {
            self.branches.push(Branch {
                x: self.width as f32 * 0.2 + i as f32 * self.width as f32 * 0.15,
                height: self.height as f32 * 0.8,
                phase: i as f32,
                droop: rng_branch(i as u32),
            });
        }
    }

    fn update(&mut self, dt: f32, _rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.tears.len() < 10 && (self.time * 10.0) as u32 % 7 == 0 {
            self.tears.push(Tear {
                x: self.branches[self.tears.len() % self.branches.len()].x,
                y: self.height as f32 * 0.8,
                vy: 0.3,
                life: 0.0,
            });
        }

        for t in &mut self.tears {
            t.y += t.vy * dt;
            t.life += dt;
        }
        self.tears.retain(|t| t.life < 8.0 && t.y < self.height as f32);
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.85 + 0.15 * (self.time * 0.2).sin();

        // Background
        for x in 0..area.width {
            for y in 0..area.height - 2 {
                let color = mul_color(C_WILLOW, breath * 0.6);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // Branches and leaves
        for b in &self.branches {
            let sway = (self.time * 0.4 + b.phase).sin() * b.droop;
            let top = (area.height as f32 - 2.0) as i32;
            for y in 0..(b.height as i32) {
                let bx = b.x + sway * (y as f32 / b.height);
                let color = if y % 3 == 0 { C_BARK } else { C_WILLOW };
                plot_dot(buf, area, bx as i32 * 2, (top - y) * 4, color);
            }
            // Leaves hanging down
            for i in 0..6 {
                let ly = (b.height * 0.5 + i as f32 * 0.4) as i32;
                if ly > 0 && ly < area.height as i32 - 2 {
                    let lx = b.x + sway + (i as f32 - 2.5) * 0.8;
                    stamp_disc(buf, area, lx * 2.0, ly as f32 * 4.0, 0.4, C_LEAF);
                    stamp_disc(buf, area, lx * 2.0 + 0.5, ly as f32 * 4.0 + 0.5, 0.3, C_LEAF_LIGHT);
                }
            }
        }

        // Tears falling
        for t in &self.tears {
            let alpha = 1.0 - (t.life / 8.0);
            let color = mul_color(C_DROP, alpha * breath);
            stamp_disc(buf, area, t.x * 2.0, t.y * 4.0, 0.4, color);
        }
    }
}

fn rng_branch(seed: u32) -> f32 {
    let h = hash(seed);
    0.5 + (h % 100) as f32 / 100.0
}
