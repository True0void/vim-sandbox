//! Rain Glass (雨のガラス) — raindrops on glass with city lights behind.

use crate::ui::themes::{plot_dot, stamp_disc, hash, mul_color, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_GLASS: (u8, u8, u8) = (0x0A, 0x0A, 0x1A);
const C_RAIN: (u8, u8, u8) = (0x80, 0xC0, 0xFF);
const C_LIGHT: (u8, u8, u8) = (0xFF, 0xD0, 0x80);
const C_LIGHT_RED: (u8, u8, u8) = (0xFF, 0x80, 0x80);
const C_LIGHT_BLUE: (u8, u8, u8) = (0x80, 0xC0, 0xFF);

#[derive(Copy, Clone)]
struct Drop {
    x: f32,
    y: f32,
    vy: f32,
    life: f32,
}

pub struct RainGlass {
    drops: Vec<Drop>,
    lights: [(f32, f32, (u8, u8, u8)); 12],
    time: f32,
    width: u16,
    height: u16,
}

impl RainGlass {
    pub fn new() -> Self {
        Self {
            drops: Vec::new(),
            lights: [(0.0, 0.0, C_LIGHT); 12],
            time: 0.0,
            width: 80,
            height: 24,
        }
    }

    fn spawn_drop(&mut self, rng: &mut ChaCha8Rng) {
        self.drops.push(Drop {
            x: rng.gen_range(0.0..self.width as f32),
            y: -1.0,
            vy: rng.gen_range(0.3..0.8),
            life: 0.0,
        });
    }
}

impl Theme for RainGlass {
    fn name(&self) -> &'static str {
        "rain-glass"
    }

    fn description(&self) -> &'static str {
        "雨のガラス — raindrops on glass with city lights behind"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.drops.clear();
        self.time = 0.0;
        for i in 0..12 {
            self.lights[i] = (
                rng_range(i as u32, 0.0..self.width as f32),
                rng_range(i as u32 + 1, 0.0..self.height as f32 * 0.6),
                if i % 3 == 0 { C_LIGHT } else if i % 3 == 1 { C_LIGHT_RED } else { C_LIGHT_BLUE },
            );
        }
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.drops.len() < 20 && rng.gen::<f32>() < 0.2 {
            self.spawn_drop(rng);
        }

        self.drops.retain_mut(|d| {
            d.y += d.vy * dt;
            d.life += dt;
            d.life < 15.0 && d.y < self.height as f32
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.8 + 0.2 * (self.time * 0.25).sin();

        // Glass background
        for x in 0..area.width {
            for y in 0..area.height - 2 {
                let h = hash((x * 5 + y * 3) as u32);
                let depth = 0.3 + 0.1 * (h % 10) as f32 / 10.0;
                let color = mul_color(C_GLASS, depth * breath);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // City lights behind glass
        for &(lx, ly, lc) in &self.lights {
            if ly > area.height as f32 - 2.0 {
                continue;
            }
            let flicker = 0.7 + 0.3 * ((self.time * 2.0 + lx * 0.5).sin()).max(0.0);
            stamp_disc(buf, area, lx * 2.0, ly * 4.0, 0.8, mul_color(lc, flicker * breath * 0.6));
        }

        // Rain drops
        for d in &self.drops {
            let color = mul_color(C_RAIN, breath * 0.8);
            stamp_disc(buf, area, d.x * 2.0, d.y * 4.0, 0.4, color);
            if d.y > 0.5 {
                stamp_disc(buf, area, d.x * 2.0, (d.y - 0.5) * 4.0, 0.3, mul_color(color, 0.5));
            }
        }
    }
}

fn rng_range(seed: u32, range: std::ops::Range<f32>) -> f32 {
    let h = hash(seed);
    let t = (h % 1000) as f32 / 1000.0;
    range.start + t * (range.end - range.start)
}
