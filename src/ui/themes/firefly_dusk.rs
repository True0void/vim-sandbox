//! Firefly Dusk (蛍の黄昏) — fireflies dancing over grass at dusk.
//! Warm yellow-green lights drifting in a cooling evening field.

use crate::ui::themes::{plot_dot, stamp_disc, draw_dot_line, hash, mul_color, Theme, TAU};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const C_DUSK: (u8, u8, u8) = (0x1A, 0x1A, 0x2E);
const C_GRASS: (u8, u8, u8) = (0x2D, 0x4A, 0x1E);
const C_FIREFLY: (u8, u8, u8) = (0xFF, 0xF0, 0x80);
const C_FIREFLY_DIM: (u8, u8, u8) = (0x8A, 0x70, 0x20);

#[derive(Copy, Clone)]
struct Firefly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    speed: f32,
    life: f32,
}

pub struct FireflyDusk {
    fireflies: Vec<Firefly>,
    time: f32,
    width: u16,
    height: u16,
}

impl FireflyDusk {
    pub fn new() -> Self {
        Self {
            fireflies: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
        }
    }

    fn spawn(&mut self, rng: &mut ChaCha8Rng) {
        self.fireflies.push(Firefly {
            x: rng.gen_range(0.0..self.width as f32),
            y: self.height as f32 * 0.6 + rng.gen_range(0.0..self.height as f32 * 0.4),
            vx: rng.gen_range(-0.3..0.3),
            vy: rng.gen_range(-0.2..0.2),
            phase: rng.gen_range(0.0..TAU),
            speed: rng.gen_range(0.5..1.5),
            life: rng.gen_range(5.0..20.0),
        });
    }
}

impl Theme for FireflyDusk {
    fn name(&self) -> &'static str {
        "firefly-dusk"
    }

    fn description(&self) -> &'static str {
        "蛍の黄昏 — fireflies dancing, grass swaying at dusk"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
        self.fireflies.clear();
        self.time = 0.0;
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;

        if self.fireflies.len() < 15 && rng.gen::<f32>() < 0.12 {
            self.spawn(rng);
        }

        self.fireflies.retain_mut(|f| {
            f.life -= dt;
            f.x += f.vx * dt * f.speed;
            f.y += f.vy * dt * f.speed;
            f.phase += dt * 2.0;
            // Bounce off edges
            if f.x < 0.0 { f.x = 0.0; f.vx = -f.vx; }
            if f.x > self.width as f32 { f.x = self.width as f32; f.vx = -f.vx; }
            if f.y < 1.0 { f.y = 1.0; f.vy = -f.vy; }
            if f.y > self.height as f32 - 2.0 { f.y = self.height as f32 - 2.0; f.vy = -f.vy; }
            f.life > 0.0
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let breath = 0.8 + 0.2 * (self.time * 0.3).sin();

        // Dusk sky
        for x in 0..area.width {
            for y in 0..area.height - 3 {
                let h = hash((x * 7 + y * 13) as u32);
                let depth = 0.3 + 0.2 * (y as f32 / area.height as f32);
                let color = mul_color(C_DUSK, depth * breath);
                plot_dot(buf, area, (x as i32) * 2, (y as i32) * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, (y as i32) * 4, color);
            }
        }

        // Grass at bottom
        let ground = (area.height as f32 - 2.0).max(3.0);
        for x in 0..area.width {
            let h = hash(x as u32);
            if h % 4 < 2 {
                let color = mul_color(C_GRASS, 0.6 + 0.4 * (self.time * 0.2 + x as f32 * 0.1).sin());
                plot_dot(buf, area, (x as i32) * 2, ground as i32 * 4, color);
                plot_dot(buf, area, (x as i32) * 2 + 1, ground as i32 * 4, color);
            }
        }

        // Fireflies
        for f in &self.fireflies {
            let glow = 0.5 + 0.5 * f.phase.sin();
            let color = mul_color(C_FIREFLY, glow * breath);
            let dim = mul_color(C_FIREFLY_DIM, glow * 0.3 * breath);
            stamp_disc(buf, area, f.x * 2.0, f.y * 4.0, 0.6, color);
            stamp_disc(buf, area, f.x * 2.0 + 1.0, f.y * 4.0 + 1.0, 0.3, dim);
        }
    }
}
