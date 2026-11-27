//! Abyssal Bloom (深海発光) — bioluminescent deep-sea creatures.
//! Glowing jellyfish and plankton pulsing in the abyss.

use super::{hash, mul_color, stamp_disc, TAU};
use crate::ui::themes::{plot_dot, draw_dot_line, smoothstep, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_DEEP: (u8, u8, u8) = (0x00, 0x05, 0x0A);
const C_CYAN: (u8, u8, u8) = (0x00, 0xFF, 0xB3);
const C_CYAN_DEEP: (u8, u8, u8) = (0x00, 0x80, 0x80);
const C_MAGENTA: (u8, u8, u8) = (0xFF, 0x00, 0xFF);
const C_GOLD: (u8, u8, u8) = (0xFF, 0xD7, 0x00);

#[derive(Copy, Clone)]
struct Jelly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    pulse: f32,
    life: f32,
    max_life: f32,
    size: f32,
    color: (u8, u8, u8),
}

pub struct AbyssalBloom {
    jellies: Vec<Jelly>,
    time: f32,
    width: u16,
    height: u16,
}

impl AbyssalBloom {
    pub fn new() -> Self {
        Self {
            jellies: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
        }
    }

    fn spawn(&mut self, rng: &mut ChaCha8Rng) {
        let x = rng.gen_range(0.0..self.width as f32);
        let color = if rng.gen::<bool>() { C_CYAN } else { C_MAGENTA };
        self.jellies.push(Jelly {
            x,
            y: -2.0,
            vx: rng.gen_range(-0.15..0.15),
            vy: rng.gen_range(0.1..0.3),
            phase: rng.gen_range(0.0..TAU),
            pulse: rng.gen_range(0.0..TAU),
            life: 0.0,
            max_life: rng.gen_range(30.0..60.0),
            size: rng.gen_range(0.8..1.6),
            color,
        });
    }
}

impl Theme for AbyssalBloom {
    fn name(&self) -> &'static str {
        "abyssal-bloom"
    }

    fn description(&self) -> &'static str {
        "深海発光 — bioluminescent deep-sea creatures glowing in the dark"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.jellies.clear();
        self.time = 0.0;
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.jellies.len() < 8 && rng.gen::<f32>() < 0.1 {
            self.spawn(rng);
        }

        self.jellies.retain_mut(|j| {
            j.life += dt;
            j.x += j.vx * dt;
            j.y += j.vy * dt;
            j.pulse += dt * 3.0;
            j.life < j.max_life && j.y < self.height as f32 + 2.0
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.85 + 0.15 * (self.time * 0.3).sin();

        // Deep water background — very dark blue/black
        let depth_color = mul_color(C_DEEP, breath * 0.3);

        for j in &self.jellies {
            let life = (j.life / j.max_life).clamp(0.0, 1.0);
            let pulse = 0.6 + 0.4 * j.pulse.sin();
            let alpha = (1.0 - life).powf(0.7) * pulse;
            let color = mul_color(j.color, alpha * breath);

            // Glow halo
            for r in 0..3 {
                let rr = j.size + r as f32 * 0.5;
                let fade = 1.0 - r as f32 * 0.3;
                stamp_disc(
                    buf,
                    area,
                    j.x * 2.0,
                    j.y * 4.0,
                    rr,
                    mul_color(j.color, alpha * fade * 0.4),
                );
            }

            // Core
            stamp_disc(buf, area, j.x * 2.0, j.y * 4.0, j.size * 0.6, color);

            // Tentacles
            for k in 0..4 {
                let angle = j.pulse + k as f32 * 1.57;
                let len = 1.5 + j.size;
                let tx = j.x + angle.cos() * len * 0.5;
                let ty = j.y + 1.0 + angle.sin() * len * 0.25;
                plot_dot(buf, area, (tx * 2.0) as i32, (ty * 4.0) as i32, mul_color(j.color, alpha * 0.5));
            }
        }
    }
}
