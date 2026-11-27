//! Firefly Dusk (蛍の黄昏) — fireflies with glowing light-trails drifting over
//! a field of individually swaying grass, beneath a three-stop twilight sky
//! (ember orange at the horizon, deep blue at the zenith, a violet band and
//! the first twinkling stars in between). A few "queen" fireflies burn
//! larger, brighter and slower than the rest.

use super::{
    draw_dot_line, hash, lerp_color, mul_color, plot_dot, smoothstep, stamp_disc, tint, Theme, TAU,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use ratatui::{buffer::Buffer, layout::Rect};

// ── Twilight palette ───────────────────────────────────────────────────────
const C_SKY_TOP: (u8, u8, u8) = (0x06, 0x09, 0x1E); // deep blue zenith
const C_SKY_MID: (u8, u8, u8) = (0x27, 0x1A, 0x3E); // violet band
const C_HORIZON: (u8, u8, u8) = (0x93, 0x3E, 0x12); // ember glow at ground line
const C_STAR: (u8, u8, u8) = (0xC9, 0xD4, 0xFF); // first stars, cool white

const C_GRASS_DARK: (u8, u8, u8) = (0x10, 0x1E, 0x10); // shaded blade base / soil
const C_GRASS: (u8, u8, u8) = (0x2D, 0x4A, 0x1E); // blade mid
const C_GRASS_LIT: (u8, u8, u8) = (0x40, 0x68, 0x2C); // sun-catching blade tip

const C_FIREFLY: (u8, u8, u8) = (0xFF, 0xF0, 0x80); // worker body
const C_FIREFLY_DIM: (u8, u8, u8) = (0x8A, 0x70, 0x20); // worker halo
const C_TRAIL: (u8, u8, u8) = (0x55, 0x48, 0x18); // cold ash of the trail
const C_QUEEN_CORE: (u8, u8, u8) = (0xFF, 0xFF, 0xE8); // hot white-gold core

// Trail history: a ring buffer of positions sampled a few times per second,
// long enough to leave a visible comet tail without ghosting across turns.
const TRAIL_LEN: usize = 10;
const TRAIL_SAMPLE: f32 = 0.22; // seconds between trail snapshots

/// One drifting light. `queen` fireflies are larger, brighter and glide
/// slower; `seed` desynchronises every per-insect oscillator.
#[derive(Clone, Copy)]
struct Firefly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32, // main pulse oscillator
    speed: f32, // drift-rate multiplier
    life: f32,
    age: f32,  // drives the fade-in envelope
    seed: f32, // stable per-insect randomness in 0..1
    queen: bool,
    trail: [(f32, f32); TRAIL_LEN],
    head: usize,  // most recent trail sample
    count: usize, // how many samples are filled
    since: f32,   // time accumulated since the last sample
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

    /// `allow_queen` caps the swarm at a small number of bright queens so
    /// they always stay special.
    fn spawn(&mut self, rng: &mut ChaCha8Rng, allow_queen: bool) {
        let queen = allow_queen && rng.gen::<f32>() < 0.2;
        let x = rng.gen_range(1.0..(self.width.max(3) - 1) as f32);
        let y = self.height as f32 * 0.3 + rng.gen_range(0.0..self.height as f32 * 0.65);
        let mut f = Firefly {
            x,
            y,
            vx: rng.gen_range(-0.5..0.5),
            vy: rng.gen_range(-0.35..0.35),
            phase: rng.gen_range(0.0..TAU),
            speed: if queen {
                rng.gen_range(0.45..0.8) // queens glide, workers dart
            } else {
                rng.gen_range(0.7..1.4)
            },
            life: rng.gen_range(8.0..22.0),
            age: 0.0,
            seed: rng.gen::<f32>(),
            queen,
            trail: [(x, y); TRAIL_LEN],
            head: 0,
            count: 1,
            since: 0.0,
        };
        f.trail[0] = (x, y);
        self.fireflies.push(f);
    }
}

/// Three-stop vertical gradient: zenith → violet band → horizon embers.
/// `t` runs 0 (top) to 1 (ground line).
fn sky_color(t: f32) -> (u8, u8, u8) {
    if t < 0.6 {
        lerp_color(C_SKY_TOP, C_SKY_MID, smoothstep(t / 0.6))
    } else {
        lerp_color(C_SKY_MID, C_HORIZON, smoothstep((t - 0.6) / 0.4))
    }
}

/// Non-sinusoidal shimmer: a slow main pulse multiplied by a detuned
/// harmonic and a fast micro-flicker. The product then gets a power curve,
/// which yields irregular bright flashes separated by long dim glows —
/// much closer to real firefly signalling than a plain sine.
fn sparkle(f: &Firefly, time: f32) -> f32 {
    let main = 0.5 + 0.5 * f.phase.sin();
    let harm = 0.5 + 0.5 * (f.phase * 1.73 + f.seed * TAU).sin();
    let flick = 0.5 + 0.5 * (time * (7.0 + f.seed * 9.0) + f.seed * 61.0).sin();
    let g = main.powf(1.8) * (0.3 + 0.7 * harm) * (0.82 + 0.18 * flick);
    if f.queen {
        (g * 1.3).min(1.0)
    } else {
        g.clamp(0.06, 1.0)
    }
}

impl Theme for FireflyDusk {
    fn name(&self) -> &'static str {
        "firefly-dusk"
    }

    fn description(&self) -> &'static str {
        "蛍の黄昏 — fireflies trailing light over swaying grass at dusk"
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

        let queens = self.fireflies.iter().filter(|f| f.queen).count();
        if self.fireflies.len() < 18 && rng.gen::<f32>() < 0.15 {
            self.spawn(rng, queens < 2);
        }

        let w = self.width as f32;
        let h = self.height as f32;
        let time = self.time;

        self.fireflies.retain_mut(|f| {
            f.age += dt;
            f.life -= dt;
            // Detuned pulse oscillator drives the shimmer cycle.
            f.phase += dt * (1.5 + f.seed * 2.0);

            // Gentle random walk on velocity → organic, non-looping flight
            // paths that never settle into a predictable ellipse.
            f.vx += (rng.gen::<f32>() - 0.5) * dt * 2.2;
            f.vy += (rng.gen::<f32>() - 0.5) * dt * 1.4;
            let vmax = if f.queen { 0.35 } else { 0.6 };
            f.vx = f.vx.clamp(-vmax, vmax);
            f.vy = f.vy.clamp(-vmax * 0.7, vmax * 0.7);
            f.x += f.vx * f.speed * dt * 1.6;
            // Tiny vertical bob on top of the drift, like real fireflies.
            f.y += f.vy * f.speed * dt * 1.6 + (time * 1.7 + f.seed * 40.0).sin() * dt * 0.08;

            // Sample the trail a few times per second so the tail spans a
            // meaningful distance even at slow drift speeds.
            f.since += dt;
            if f.since >= TRAIL_SAMPLE {
                f.since = 0.0;
                f.head = (f.head + 1) % TRAIL_LEN;
                f.trail[f.head] = (f.x, f.y);
                f.count = (f.count + 1).min(TRAIL_LEN);
            }

            // Soft walls: pin and steer back instead of mirroring instantly,
            // so trail segments stay continuous near the borders.
            if f.x < 1.0 {
                f.x = 1.0;
                f.vx = f.vx.abs();
            }
            if f.x > w - 1.0 {
                f.x = w - 1.0;
                f.vx = -f.vx.abs();
            }
            if f.y < h * 0.22 {
                f.y = h * 0.22;
                f.vy = f.vy.abs();
            }
            if f.y > h - 1.0 {
                f.y = h - 1.0;
                f.vy = -f.vy.abs();
            }

            f.life > 0.0
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        let sky_rows = area.height.saturating_sub(2); // bottom two rows belong to the grass
        let horizon = sky_rows.max(1) as f32;
        // The whole scene breathes slowly, like the last light fading.
        let breathe = 0.85 + 0.15 * (self.time * 0.25).sin();

        // ── Sky: twilight gradient, dithered stipple, emerging stars ──────
        for cy in 0..sky_rows {
            let t = cy as f32 / horizon;
            let base = sky_color(t);
            for cx in 0..area.width {
                // ~40 % of the dot slots carry a background pixel; skipping
                // the rest keeps the field airy so the lights pop against it.
                let cell = hash((cx as u32) ^ (cy as u32).wrapping_mul(0x5BD1_E995));
                if cell % 10 < 4 {
                    let dy = if cell % 20 < 10 { 1 } else { 3 };
                    let jitter = 0.7 + 0.3 * ((cell >> 8) as f32 / u8::MAX as f32);
                    plot_dot(
                        buf,
                        area,
                        (cx as i32) * 2,
                        (cy as i32) * 4 + dy,
                        mul_color(base, jitter * breathe),
                    );
                }

                // Stars only in the upper half, where the blue is darkest.
                if t < 0.5 {
                    let sh =
                        hash((cx as u32).wrapping_mul(0x27D4_EB4D) ^ ((cy as u32) << 3) ^ 0x005E_ED);
                    if sh % 97 < 2 {
                        let speed = 0.6 + (sh % 6) as f32 * 0.25;
                        let tw = 0.5 + 0.5 * (self.time * speed + (sh % 64) as f32 * 0.098).sin();
                        // Squaring the twinkle makes brief peaks stand out.
                        let bright = 0.2 + 0.7 * tw * tw;
                        plot_dot(
                            buf,
                            area,
                            (cx as i32) * 2 + (sh & 1) as i32,
                            (cy as i32) * 4 + 1 + ((sh >> 3) & 1) as i32,
                            mul_color(C_STAR, bright),
                        );
                    }
                }
            }
        }

        // ── Soil band: dim stipple anchoring the grass line ───────────────
        let ground_y = (area.height as f32 - 1.0) * 4.0; // dot row of the soil top
        for cx in 0..area.width {
            for k in 0..6u32 {
                let sh = hash((cx as u32) * 7 + k + 0x9100);
                if sh % 3 == 0 {
                    plot_dot(
                        buf,
                        area,
                        (cx as i32) * 2 + (sh & 1) as i32,
                        ground_y as i32 - (sh % 4) as i32,
                        mul_color(C_GRASS_DARK, 0.55 * breathe),
                    );
                }
            }
        }

        // ── Grass: individual blades swaying in a travelling gust ─────────
        for cx in 0..area.width {
            let h0 = hash((cx as u32) ^ 0xBEEF_0000);
            if h0 % 10 >= 8 {
                continue; // bare gaps keep the field natural
            }
            let x0 = (cx as f32) * 2.0 + (h0 % 2) as f32;
            let root = ground_y - (h0 % 3) as f32 * 0.9; // slight jitter per blade
            let tall = h0 % 10 >= 6; // occasional tall blade catching the light
            let height = if tall {
                12.0 + (h0 % 9) as f32
            } else {
                5.0 + (h0 % 8) as f32
            };

            // Travelling wave: the sway phase shifts along x, so gusts
            // visibly roll across the meadow instead of every blade nodding
            // in lockstep.
            let gust = (self.time * 1.15 + cx as f32 * 0.42 + (h0 % 62) as f32 * 0.1).sin();
            let lean = gust * (1.1 + (h0 % 5) as f32 * 0.3) + (h0 % 7) as f32 * 0.12 - 0.36;
            let mid_x = x0 + lean * 0.3;
            let mid_y = root - height * 0.55;
            let tip_x = x0 + lean;
            let tip_y = root - height;

            let shade = 0.5 + 0.5 * (0.3 * gust + (h0 % 16) as f32 / 20.0).clamp(0.0, 1.0);
            let c_root = mul_color(C_GRASS_DARK, (shade + 0.25) * breathe);
            let c_mid = mul_color(C_GRASS, shade * breathe);
            // Tips are warmed by the last light of the horizon (red up,
            // blue/green down) — this is what reads as "dusk" on the grass.
            let c_tip = mul_color(tint(C_GRASS_LIT, 1.25, 0.8), shade * 1.05 * breathe);

            // Two tapered segments give the blade a subtle bend.
            draw_dot_line(buf, area, x0, root, mid_x, mid_y, 0.75, 0.5, c_root, c_mid);
            draw_dot_line(buf, area, mid_x, mid_y, tip_x, tip_y, 0.5, 0.15, c_mid, c_tip);
        }

        // ── Fireflies: halo, comet trail, body, hot core ──────────────────
        // Drawn last; plot_dot keeps the brightest stroke per dot, so lights
        // always win over sky and grass pixels underneath them.
        for f in &self.fireflies {
            // Fade in over ~1.2 s, fade out over the final ~1.8 s of life,
            // so lights never blink into existence abruptly.
            let env = smoothstep((f.age / 1.2).min(1.0)) * smoothstep((f.life / 1.8).min(1.0));
            let glow = sparkle(f, self.time) * env;

            let (body, halo, core_r, body_r, halo_r) = if f.queen {
                (C_QUEEN_CORE, C_FIREFLY, 0.55, 1.15, 2.6)
            } else {
                (C_FIREFLY, C_FIREFLY_DIM, 0.35, 0.65, 1.7)
            };

            // Faint ambient bloom around the light.
            stamp_disc(buf, area, f.x * 2.0, f.y * 4.0, halo_r, mul_color(halo, glow * 0.16));

            // Trail: walk the ring buffer oldest → newest, brightening and
            // widening each segment toward the body (t goes 0 → 1).
            let n = f.count;
            if n >= 2 {
                for d in (1..n).rev() {
                    let a = f.trail[(f.head + TRAIL_LEN - d) % TRAIL_LEN];
                    let b = f.trail[(f.head + TRAIL_LEN - d + 1) % TRAIL_LEN];
                    let t = (n - d) as f32 / (n - 1).max(1) as f32;
                    let tc = lerp_color(C_TRAIL, body, 0.25 + 0.55 * t);
                    draw_dot_line(
                        buf,
                        area,
                        a.0 * 2.0,
                        a.1 * 4.0,
                        b.0 * 2.0,
                        b.1 * 4.0,
                        0.15,
                        0.45,
                        mul_color(tc, glow * 0.12 * t),
                        mul_color(tc, glow * 0.55 * (0.3 + 0.7 * t)),
                    );
                }
                // Bridge from the newest sample to the live position so the
                // tail stays connected even between samples.
                let b = f.trail[f.head];
                draw_dot_line(
                    buf,
                    area,
                    b.0 * 2.0,
                    b.1 * 4.0,
                    f.x * 2.0,
                    f.y * 4.0,
                    0.45,
                    0.6,
                    mul_color(body, glow * 0.5),
                    mul_color(body, glow * 0.85),
                );
            }

            // Body and white-hot core.
            stamp_disc(
                buf,
                area,
                f.x * 2.0,
                f.y * 4.0,
                body_r,
                mul_color(body, 0.3 + 0.7 * glow),
            );
            stamp_disc(
                buf,
                area,
                f.x * 2.0,
                f.y * 4.0,
                core_r,
                mul_color(C_QUEEN_CORE, glow),
            );
        }
    }
}
