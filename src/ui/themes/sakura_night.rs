//! Sakura Night (夜桜) — cherry blossoms at night under moonlight.
//!
//! Pink petals drifting over a dark indigo sky, with a faint moon glow.

use super::{hash, mul_color, stamp_disc, TAU};
use crate::ui::themes::{plot_dot, draw_dot_line, smoothstep, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_SKY: (u8, u8, u8) = (0x0A, 0x0A, 0x1E);
const C_PINK: (u8, u8, u8) = (0xFF, 0xC0, 0xCB);
const C_PINK_DEEP: (u8, u8, u8) = (0xFF, 0x69, 0xB4);
const C_MOON: (u8, u8, u8) = (0xF0, 0xF0, 0xE0);
const C_BRANCH: (u8, u8, u8) = (0x2A, 0x1A, 0x1A);

#[derive(Copy, Clone)]
struct Blossom {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    angle: f32,
    spin: f32,
    phase: f32,
    life: f32,
    max_life: f32,
    size: f32,
}

pub struct SakuraNight {
    blossoms: Vec<Blossom>,
    time: f32,
    width: u16,
    height: u16,
    moon_x: f32,
    moon_y: f32,
}

impl SakuraNight {
    pub fn new() -> Self {
        Self {
            blossoms: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
            moon_x: 0.0,
            moon_y: 0.0,
        }
    }

    fn spawn(&mut self, rng: &mut ChaCha8Rng) {
        let x = rng.gen_range(0.0..self.width as f32);
        self.blossoms.push(Blossom {
            x,
            y: -2.0,
            vx: rng.gen_range(-0.2..0.2),
            vy: rng.gen_range(0.15..0.45),
            angle: rng.gen_range(0.0..TAU),
            spin: rng.gen_range(-0.3..0.3),
            phase: rng.gen_range(0.0..TAU),
            life: 0.0,
            max_life: rng.gen_range(20.0..50.0),
            size: rng.gen_range(0.5..1.2),
        });
    }
}

impl Theme for SakuraNight {
    fn name(&self) -> &'static str {
        "sakura-night"
    }

    fn description(&self) -> &'static str {
        "夜桜 — cherry blossoms drifting under a moonlight sky"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.blossoms.clear();
        self.time = 0.0;
        self.moon_x = self.width as f32 * 0.75;
        self.moon_y = self.height as f32 * 0.25;
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.blossoms.len() < 12 && rng.gen::<f32>() < 0.15 {
            self.spawn(rng);
        }

        self.blossoms.retain_mut(|b| {
            b.life += dt;
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.angle += b.spin * dt;
            b.life < b.max_life && b.y < self.height as f32 + 2.0
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        // Night sky gradient — dark indigo
        let breath = 0.9 + 0.1 * (self.time * 0.2).sin();

        // Moon
        let moon_color = mul_color(C_MOON, 0.6 * breath);
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx * dx + dy * dy <= 4 {
                    let xx = (self.moon_x * 2.0 + dx as f32) as i32;
                    let yy = (self.moon_y * 4.0 + dy as f32) as i32;
                    if xx >= 0 && xx < area.width as i32 * 2 && yy >= 0 && yy < area.height as i32 * 4 {
                        plot_dot(buf, area, xx, yy, moon_color);
                    }
                }
            }
        }

        // Blossoms
        for b in &self.blossoms {
            let life = (b.life / b.max_life).clamp(0.0, 1.0);
            let alpha = (1.0 - life).powf(0.7);
            let color = mul_color(C_PINK, alpha * breath);
            let len = 1.0 + b.size;
            let dx = b.angle.cos();
            let dy = b.angle.sin();
            for k in 0..3 {
                let offset = (k as f32 / 2.0 - 0.5) * len;
                stamp_disc(
                    buf,
                    area,
                    b.x * 2.0 + dx * offset,
                    b.y * 4.0 + dy * offset,
                    0.4,
                    color,
                );
            }
        }
    }
}
