//! Nebula Heart (星雲の心臓) — pulsing nebula cores in deep space.

use crate::ui::themes::{plot_dot, stamp_disc, hash, mul_color, Theme, TAU};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_VOID: (u8, u8, u8) = (0x02, 0x02, 0x05);
const C_NEBULA: (u8, u8, u8) = (0x80, 0x00, 0x80);
const C_NEBULA_PINK: (u8, u8, u8) = (0xFF, 0x80, 0xC0);
const C_NEBULA_BLUE: (u8, u8, u8) = (0x80, 0xC0, 0xFF);
const C_STAR: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

#[derive(Copy, Clone)]
struct Core {
    x: f32,
    y: f32,
    phase: f32,
    pulse: f32,
    color: (u8, u8, u8),
}

pub struct NebulaHeart {
    cores: Vec<Core>,
    stars: [(f32, f32); 50],
    time: f32,
    width: u16,
    height: u16,
}

impl NebulaHeart {
    pub fn new() -> Self {
        Self {
            cores: Vec::new(),
            stars: [(0.0, 0.0); 50],
            time: 0.0,
            width: 80,
            height: 24,
        }
    }
}

impl Theme for NebulaHeart {
    fn name(&self) -> &'static str {
        "nebula-heart"
    }

    fn description(&self) -> &'static str {
        "星雲の心臓 — pulsing nebula cores in deep space"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.cores.clear();
        self.time = 0.0;
        for i in 0..50 {
            self.stars[i] = (
                (i * 3) as f32 % self.width as f32,
                (i * 5) as f32 % self.height as f32,
            );
        }
        for i in 0..3 {
            self.cores.push(Core {
                x: self.width as f32 * 0.25 + i as f32 * self.width as f32 * 0.25,
                y: self.height as f32 * 0.3 + i as f32 * self.height as f32 * 0.2,
                phase: i as f32 * 2.0,
                pulse: 0.0,
                color: if i == 0 { C_NEBULA } else if i == 1 { C_NEBULA_PINK } else { C_NEBULA_BLUE },
            });
        }
    }

    fn update(&mut self, dt: f32, _rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        for c in &mut self.cores {
            c.pulse += dt * 2.0;
        }
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.7 + 0.3 * (self.time * 0.2).sin();

        // Void background
        for x in 0..area.width {
            for y in 0..area.height - 2 {
                let color = mul_color(C_VOID, breath * 0.4);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // Stars
        for &(sx, sy) in &self.stars {
            let twinkle = 0.3 + 0.7 * ((self.time * 1.5 + sx * 0.3).sin()).max(0.0);
            plot_dot(buf, area, (sx as i32) * 2, (sy as i32) * 4, mul_color(C_STAR, twinkle * breath));
        }

        // Nebula cores
        for c in &self.cores {
            let intensity = 0.3 + 0.7 * ((self.time + c.phase).sin()).max(0.0);
            let radius = 2.0 + intensity * 2.0;
            let color = mul_color(c.color, intensity * breath);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    if dx * dx + dy * dy <= 4 {
                        plot_dot(
                            buf,
                            area,
                            (c.x * 2.0 + dx as f32) as i32,
                            (c.y * 4.0 + dy as f32) as i32,
                            color,
                        );
                    }
                }
            }
            stamp_disc(buf, area, c.x * 2.0, c.y * 4.0, 1.0 + intensity, color);
        }
    }
}
