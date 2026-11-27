//! Aurora Void (極光の虚空) — organic aurora curtains with glowing trails
//! undulating over a twinkling starfield, above a deep-space horizon.
//!
//! Visual layers (back to front):
//! 1. Space gradient — pure black at the zenith, blue-violet and green glow
//!    rising from the horizon.
//! 2. Starfield — 110 stars in three size classes, each twinkling on its own
//!    phase and speed (deterministic via `hash`, so no RNG state needed).
//! 3. Aurora curtains — seven curtains in varied colours, each swayed by three
//!    incommensurate sine waves, with a vertical brightness gradient and a
//!    short ghosted trail that lags the live edge.

use crate::ui::themes::{hash, lerp_color, mul_color, plot_dot, smoothstep, stamp_disc, Theme, TAU};
use rand_chacha::ChaCha8Rng;
use ratatui::{buffer::Buffer, layout::Rect};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// Absolute black of deep space (zenith).
const C_VOID: (u8, u8, u8) = (0x02, 0x02, 0x06);
/// Blue-violet glow rising from the horizon.
const C_HORIZON: (u8, u8, u8) = (0x0E, 0x12, 0x30);
/// Faint green wash bleeding into the horizon from the curtains.
const C_HORIZON_GREEN: (u8, u8, u8) = (0x0A, 0x2E, 0x24);
/// Cold tint blended into the tops of the curtains as they fade away.
const C_CURTAIN_TOP: (u8, u8, u8) = (0x14, 0x18, 0x48);

const C_AURORA_GREEN: (u8, u8, u8) = (0x00, 0xFF, 0x90);
const C_AURORA_TEAL: (u8, u8, u8) = (0x00, 0xE0, 0xD0);
const C_AURORA_BLUE: (u8, u8, u8) = (0x48, 0x90, 0xFF);
const C_AURORA_VIOLET: (u8, u8, u8) = (0x98, 0x48, 0xFF);
const C_AURORA_PINK: (u8, u8, u8) = (0xFF, 0x60, 0xC8);
const C_AURORA_MINT: (u8, u8, u8) = (0x70, 0xFF, 0xC0);
const C_AURORA_LILAC: (u8, u8, u8) = (0xC0, 0x70, 0xFF);

const C_STAR: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);
const C_STAR_COOL: (u8, u8, u8) = (0xC0, 0xD4, 0xFF);
const C_STAR_WARM: (u8, u8, u8) = (0xFF, 0xE4, 0xC0);

// ---------------------------------------------------------------------------
// Tuning
// ---------------------------------------------------------------------------

const STAR_COUNT: u32 = 110;
/// Number of ghost copies drawn behind the live curtain edge (the trail).
const TRAIL_STEPS: u32 = 3;
/// Time lag (seconds) between consecutive trail ghosts.
const TRAIL_DT: f32 = 0.11;
/// Vertical samples per curtain per pass.
const CURTAIN_SAMPLES: usize = 16;

#[derive(Copy, Clone)]
struct Curtain {
    /// Normalized horizontal anchor (0..1).
    base_x: f32,
    /// Sway amplitude in braille dots.
    amp: f32,
    /// Speed of the primary vertical phase wave.
    speed: f32,
    /// Speed of the slow overall left/right drift.
    drift: f32,
    /// Phase offset; decorrelates the curtains from each other.
    phase: f32,
    /// Base stroke radius in dots.
    thickness: f32,
    color: (u8, u8, u8),
}

fn curtain(
    i: usize,
    base_x: f32,
    amp: f32,
    speed: f32,
    drift: f32,
    thickness: f32,
    color: (u8, u8, u8),
) -> Curtain {
    // Mix a near-uniform TAU spacing with a slow irrational step so the
    // curtains never lock into a visible rhythm.
    let phase = i as f32 * TAU / 7.0 + i as f32 * 0.9;
    Curtain {
        base_x,
        amp,
        speed,
        drift,
        phase,
        thickness,
        color,
    }
}

/// Horizontal dot-space position of a curtain at normalized height `y01`
/// (0 = horizon, 1 = top) and time `t`.  Three incommensurate sine waves are
/// stacked so the sway reads as organic fabric, not a metronome.
fn wave_x(c: &Curtain, w: f32, y01: f32, t: f32) -> f32 {
    let base = c.base_x * w + (t * c.drift + c.phase).sin() * w * 0.07;
    let a = c.amp;
    base
        + (t * c.speed + y01 * 3.1 + c.phase).sin() * a
        + (t * c.speed * 1.63 + y01 * 5.7 - c.phase * 0.7).sin() * a * 0.45
        + (t * c.speed * 0.47 + y01 * 1.3 + c.phase * 1.9).sin() * a * 0.3
}

pub struct AuroraVoid {
    curtains: [Curtain; 7],
    time: f32,
    width: u16,
    height: u16,
}

impl AuroraVoid {
    pub fn new() -> Self {
        // Seven curtains spread across the width, each with its own colour,
        // amplitude, speed and drift so they weave through one another.
        let curtains = [
            curtain(0, 0.07, 10.0, 0.55, 0.21, 1.0, C_AURORA_GREEN),
            curtain(1, 0.21, 14.0, 0.42, 0.16, 1.2, C_AURORA_TEAL),
            curtain(2, 0.36, 12.0, 0.63, 0.27, 1.1, C_AURORA_BLUE),
            curtain(3, 0.50, 16.0, 0.48, 0.13, 1.3, C_AURORA_VIOLET),
            curtain(4, 0.64, 11.0, 0.58, 0.24, 1.0, C_AURORA_PINK),
            curtain(5, 0.79, 15.0, 0.40, 0.19, 1.2, C_AURORA_MINT),
            curtain(6, 0.93, 12.0, 0.52, 0.29, 1.1, C_AURORA_LILAC),
        ];
        Self {
            curtains,
            time: 0.0,
            width: 80,
            height: 24,
        }
    }

    /// Paint one vertical pass of a curtain at time `t`.
    ///
    /// Brightness falls off toward the top (squared, so the base glows and
    /// the tips dissolve), and the stroke thins as it climbs.  The colour
    /// itself fades from `c.color` at the horizon toward a cold blue up top.
    fn stamp_curtain(
        &self,
        buf: &mut Buffer,
        area: Rect,
        c: &Curtain,
        w: f32,
        h_dots: f32,
        t: f32,
        intensity: f32,
        top: (u8, u8, u8),
    ) {
        for i in 0..CURTAIN_SAMPLES {
            let f = i as f32 / (CURTAIN_SAMPLES - 1) as f32; // 0 = horizon
            let y = f * (h_dots - 4.0); // keep the top sample one cell in
            let x = wave_x(c, w, f, t);
            let vert = 1.0 - f;
            let lum = (0.10 + 0.90 * vert * vert) * intensity;
            let col = mul_color(lerp_color(top, c.color, vert), lum);
            let r = c.thickness * (0.6 + 0.8 * vert);
            stamp_disc(buf, area, x, y, r, col);
        }
    }

    /// Deterministic starfield: position, size class, colour, twinkle phase
    /// and speed all come from `hash`, so every frame is reproducible without
    /// touching the RNG.
    fn draw_stars(&self, buf: &mut Buffer, area: Rect) {
        let w = self.width as f32 * 2.0;
        let h = (self.height - 2) as f32 * 4.0;
        for i in 0..STAR_COUNT {
            let k = i + 1; // keep the hash inputs distinct
            let x = (hash(k.wrapping_mul(7349).wrapping_add(911)) % 1000) as f32 / 1000.0 * w;
            // Stars stay in the upper 85 % — the horizon band belongs to the glow.
            let y = (hash(k.wrapping_mul(1847).wrapping_add(211)) % 1000) as f32 / 1000.0 * h * 0.85;
            let size = (hash(k.wrapping_mul(8123).wrapping_add(373)) % 1000) as f32 / 1000.0;
            let phase = (hash(k.wrapping_mul(4051).wrapping_add(577)) % 1000) as f32 / 1000.0 * TAU;
            let speed = 0.6 + (hash(k.wrapping_mul(6073).wrapping_add(991)) % 1000) as f32 / 1000.0 * 2.6;
            let warmth = (hash(k.wrapping_mul(2971).wrapping_add(63)) % 100) as u32;

            // Mostly white, a minority cool or warm for depth.
            let base = if warmth < 15 {
                C_STAR_WARM
            } else if warmth < 30 {
                C_STAR_COOL
            } else {
                C_STAR
            };

            // Independent twinkle: per-star phase and speed mean no two
            // stars blink in sync.
            let tw = 0.25 + 0.75 * (0.5 + 0.5 * (self.time * speed + phase).sin());

            if size < 0.65 {
                // Dim pinprick: a single dot.
                plot_dot(buf, area, x as i32, y as i32, mul_color(base, tw * 0.8));
            } else if size < 0.92 {
                // Mid star: a small disc.
                stamp_disc(buf, area, x, y, 1.0, mul_color(base, tw));
            } else {
                // Bright star: disc plus a soft halo behind it.
                stamp_disc(buf, area, x, y, 2.4, mul_color(C_STAR_COOL, tw * 0.30));
                stamp_disc(buf, area, x, y, 1.3, mul_color(base, tw));
            }
        }
    }

    /// Space gradient: absolute black at the top, easing down through a
    /// blue-violet glow to a green-tinted horizon where the curtains stand.
    fn draw_sky(&self, buf: &mut Buffer, area: Rect) {
        let rows = self.height.saturating_sub(2);
        for cy in 0..rows {
            let t = (cy as f32 + 0.5) / rows as f32;
            let k = smoothstep((t - 0.30) / 0.70);
            let mut col = lerp_color(C_VOID, C_HORIZON, k);
            // Extra green wash in the lowest quarter, as if the curtains
            // illuminate the ground beneath them.
            let g = smoothstep((t - 0.70) / 0.30) * 0.5;
            col = lerp_color(col, C_HORIZON_GREEN, g);
            for cx in 0..self.width {
                plot_dot(buf, area, (cx as i32) * 2, (cy as i32) * 4, col);
                plot_dot(buf, area, (cx as i32) * 2 + 1, (cy as i32) * 4, col);
            }
        }
    }

    fn draw_curtains(&self, buf: &mut Buffer, area: Rect) {
        let w = self.width as f32 * 2.0;
        let h_dots = (self.height - 2) as f32 * 4.0;
        for c in &self.curtains {
            // Each curtain breathes on its own slow rhythm.
            let pulse = 0.55 + 0.45 * (self.time * 0.45 + c.phase * 2.0).sin();
            let top = lerp_color(c.color, C_CURTAIN_TOP, 0.75);

            // Glowing trail: ghost copies of the wave evaluated at earlier
            // times, each dimmer than the last, so the curtain leaves a
            // luminous wake as it sways.
            for lag in 1..=TRAIL_STEPS {
                let fade = 1.0 - (lag as f32 * 0.8) / (TRAIL_STEPS as f32);
                let t = self.time - lag as f32 * TRAIL_DT;
                self.stamp_curtain(buf, area, c, w, h_dots, t, pulse * fade, top);
            }
            // The live edge at full intensity.
            self.stamp_curtain(buf, area, c, w, h_dots, self.time, pulse, top);

            // Bright crest where the curtain meets the horizon.
            let crest_x = wave_x(c, w, 0.0, self.time);
            let crest = mul_color(lerp_color(c.color, C_STAR, 0.35), pulse);
            stamp_disc(buf, area, crest_x, h_dots - 4.0, 1.6, crest);
        }
    }
}

impl Theme for AuroraVoid {
    fn name(&self) -> &'static str {
        "aurora-void"
    }

    fn description(&self) -> &'static str {
        "極光の虚空 — aurora curtains with glowing trails over a starfield"
    }

    fn resize(&mut self, area: Rect) {
        self.width = area.width;
        self.height = area.height;
    }

    fn update(&mut self, dt: f32, _rng: &mut ChaCha8Rng) {
        // Everything is a pure function of `self.time`, so update only
        // advances the clock (and guards against big pauses).
        self.time += dt.clamp(0.0, 0.1);
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }
        self.draw_sky(buf, area);
        self.draw_stars(buf, area);
        self.draw_curtains(buf, area);
    }
}
