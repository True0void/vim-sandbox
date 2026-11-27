//! Aurora Void (極光の虚空) — cosmic aurora curtains pulsing in deep space.

use crate::ui::themes::{plot_dot, stamp_disc, hash, mul_color, Theme, TAU};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_VOID: (u8, u8, u8) = (0x02, 0x02, 0x08);
const C_AURORA: (u8, u8, u8) = (0x00, 0xFF, 0x80);
const C_AURORA_PURPLE: (u8, u8, u8) = (0x80, 0x00, 0xFF);
const C_STAR: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

#[derive(Copy, Clone)]
struct Curtain {
    x: f32,
    phase: f32,
    wave: f32,
    color: (u8, u8, u8),
}

pub struct AuroraVoid {
    curtains: Vec<Curtain>,
    stars: [(f32, f32); 40],
    time: f32,
    width: u16,
    height: u16,
}

impl AuroraVoid {
    pub fn new() -> Self {
        Self {
            curtains: Vec::new(),
            stars: [(0.0, 0.0); 40],
            time: 0.0,
            width: 80,
            height: 24,
        }
    }
}

impl Theme for AuroraVoid {
    fn name(&self) -> &'static str {
        "aurora-void"
    }

    fn description(&self) -> &'static str {
        "極光の虚空 — cosmic aurora curtains pulsing in deep space"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.curtains.clear();
        self.time = 0.0;
        for i in 0..40 {
            self.stars[i] = (i as f32 * 2.0, (i * 7) as f32 % self.height as f32);
        }
    }

    fn update(&mut self, dt: f32, _rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.curtains.len() < 4 {
            self.curtains.push(Curtain {
                x: self.width as f32 * 0.2 + self.curtains.len() as f32 * self.width as f32 * 0.2,
                phase: self.curtains.len() as f32 * 2.0,
                wave: 0.0,
                color: if self.curtains.len() % 2 == 0 { C_AURORA } else { C_AURORA_PURPLE },
            });
        }

        for c in &mut self.curtains {
            c.wave += dt * 0.5;
        }
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.7 + 0.3 * (self.time * 0.25).sin();

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
            let twinkle = 0.4 + 0.6 * ((self.time * 2.0 + sx * 0.5).sin()).max(0.0);
            plot_dot(buf, area, (sx as i32) * 2, (sy as i32) * 4, mul_color(C_STAR, twinkle * breath));
        }

        // Aurora curtains
        for c in &self.curtains {
            let intensity = 0.4 + 0.6 * ((self.time + c.wave).sin()).max(0.0);
            let color = mul_color(c.color, intensity * breath);
            for i in 0..20 {
                let t = i as f32 / 20.0;
                let y = (area.height as f32 - 2.0) * (1.0 - t);
                let x = c.x + c.wave.sin() * 3.0 * t + (self.time * 0.3 + t * 5.0).sin() * 1.0;
                if x >= 0.0 && x < self.width as f32 {
                    stamp_disc(buf, area, x * 2.0, y * 4.0, 0.5, color);
                }
            }
        }
    }
}
