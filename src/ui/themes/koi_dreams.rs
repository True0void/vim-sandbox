//! Koi Dreams (錦鯉の夢) — koi fish gliding over water lilies in a moonlit pond.

use crate::ui::themes::{plot_dot, stamp_disc, draw_dot_line, hash, mul_color, Theme, TAU};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_POND: (u8, u8, u8) = (0x05, 0x0A, 0x15);
const C_KOI_ORANGE: (u8, u8, u8) = (0xFF, 0x8C, 0x00);
const C_KOI_WHITE: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);
const C_KOI_BLACK: (u8, u8, u8) = (0x10, 0x10, 0x10);
const C_LILY: (u8, u8, u8) = (0x20, 0x40, 0x20);
const C_FLOWER: (u8, u8, u8) = (0xFF, 0x40, 0x80);

#[derive(Copy, Clone)]
struct Koi {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    color: (u8, u8, u8),
}

pub struct KoiDreams {
    koi: Vec<Koi>,
    lilies: [(f32, f32); 6],
    time: f32,
    width: u16,
    height: u16,
}

impl KoiDreams {
    pub fn new() -> Self {
        Self {
            koi: Vec::new(),
            lilies: [(0.0, 0.0); 6],
            time: 0.0,
            width: 80,
            height: 24,
        }
    }

    fn spawn_koi(&mut self, rng: &mut ChaCha8Rng) {
        let colors = [C_KOI_ORANGE, C_KOI_WHITE, C_KOI_BLACK];
        self.koi.push(Koi {
            x: rng.gen_range(0.0..self.width as f32),
            y: self.height as f32 * 0.5 + rng.gen_range(0.0..self.height as f32 * 0.3),
            vx: rng.gen_range(0.2..0.6),
            vy: rng.gen_range(-0.1..0.1),
            phase: rng.gen_range(0.0..TAU),
            color: colors[rng.gen_range(0..3)],
        });
    }
}

impl Theme for KoiDreams {
    fn name(&self) -> &'static str {
        "koi-dreams"
    }

    fn description(&self) -> &'static str {
        "錦鯉の夢 — koi fish, water lilies, ripples in a moonlit pond"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.koi.clear();
        self.time = 0.0;
        for i in 0..6 {
            self.lilies[i] = (
                rng_range_deterministic(i as u32, 0.0..self.width as f32),
                self.height as f32 * 0.7 + (i as f32 * 0.5),
            );
        }
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.koi.len() < 5 && rng.gen::<f32>() < 0.08 {
            self.spawn_koi(rng);
        }

        for k in &mut self.koi {
            k.x += k.vx * dt;
            k.y += k.vy * dt;
            k.phase += dt * 2.0;
            if k.x > self.width as f32 + 2.0 {
                k.x = -2.0;
                k.y = self.height as f32 * 0.5 + rng.gen_range(0.0..self.height as f32 * 0.3);
            }
        }
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.85 + 0.15 * (self.time * 0.2).sin();

        // Pond water
        for x in 0..area.width {
            for y in 0..area.height - 2 {
                let h = hash((x * 3 + y * 7) as u32);
                let depth = 0.4 + 0.2 * (y as f32 / area.height as f32);
                let color = mul_color(C_POND, depth * breath);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // Water lilies
        for &(lx, ly) in &self.lilies {
            if ly > area.height as f32 - 2.0 {
                continue;
            }
            stamp_disc(buf, area, lx * 2.0, ly * 4.0, 1.2, C_LILY);
            stamp_disc(buf, area, lx * 2.0 + 1.0, ly * 4.0 + 1.0, 0.5, C_FLOWER);
        }

        // Koi fish
        for k in &self.koi {
            let glow = 0.6 + 0.4 * k.phase.sin();
            let color = mul_color(k.color, glow * breath);
            stamp_disc(buf, area, k.x * 2.0, k.y * 4.0, 0.8, color);
            stamp_disc(buf, area, k.x * 2.0 + 0.5, k.y * 4.0 + 0.5, 0.4, mul_color(color, 0.5));
        }
    }
}

fn rng_range_deterministic(seed: u32, range: std::ops::Range<f32>) -> f32 {
    let h = hash(seed);
    let t = (h % 1000) as f32 / 1000.0;
    range.start + t * (range.end - range.start)
}
