//! Spider Lily (彼岸花) — a slow, minimal night garden drawn in braille.
//!
//! Everything here is background art only: stems planted in the soil, long
//! recurved petals, gold stamens and a few petals drifting down. The editor
//! itself is untouched.
//!
//! Layers, back to front:
//!   1. cached soil (moss dots + grass blades, positions computed once)
//!   2. low mist hugging the ground and a handful of pollen motes
//!   3. stems, with a specular edge on near flowers
//!   4. flower heads: aura, stamens, petals, dew, a rare éclat
//!   5. falling / landed petals
//!
//! Rendering notes (why this stays cheap at 60 fps):
//!   * the static ground layout lives in [`Ground`] and is rebuilt only when
//!     the terminal width changes — no per-frame hashing of every column;
//!   * every stroke picks its sample count from its on-screen length in
//!     braille dots, so short stems are not stamped 80 times over;
//!   * detail layers (dew, veins, specular edges, glints) are gated on
//!     `depth`, which keeps the far flowers down to a few strokes per petal.

use super::{
    draw_dot_line, hash, lerp_color, mul_color, plot_dot, smoothstep, stamp_disc,
    stamp_disc_force, tint, C_ANTHER, C_DEEP_BG, C_FALLING, C_GLOW, C_MOSS, C_MOSS_LIGHT,
    C_PETAL_DEEP, C_PETAL_HOT, C_PETAL_MID, C_PETAL_TIP, C_STAMEN, C_STEM_DARK, C_STEM_LIGHT,
    TAU,
};
use crate::ui::themes::Theme;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use ratatui::{buffer::Buffer, layout::Rect};

const PETAL_COUNT: usize = 6;
const STAMEN_COUNT: usize = 5;
const DEW_COUNT: usize = 3;

/// Cap so a burst of gusts can never flood the frame with sprites.
const MAX_FALLING: usize = 140;

// ============================================================================
// Colour
// ============================================================================

/// Per-flower palette.
///
/// `Flower::hue` (-1 cool, +1 warm) shifts *every* step of the petal gradient,
/// so the garden reads as many different flowers instead of one red sprite
/// repeated six times. All entries are derived from the shared `C_*` colours.
#[derive(Copy, Clone)]
struct Palette {
    deep: (u8, u8, u8),
    mid: (u8, u8, u8),
    tip: (u8, u8, u8),
    hot: (u8, u8, u8),
    glow: (u8, u8, u8),
    vein: (u8, u8, u8),
    anther: (u8, u8, u8),
    stem_lo: (u8, u8, u8),
    stem_hi: (u8, u8, u8),
}

impl Palette {
    fn new(hue: f32) -> Self {
        // 0 = cool crimson/violet, 1 = warm scarlet/coral.
        let warm = ((hue.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);

        let deep = lerp_color(
            tint(C_PETAL_DEEP, 0.78, 1.14),
            tint(C_PETAL_DEEP, 1.12, 0.82),
            warm,
        );
        let mid = lerp_color(
            tint(C_PETAL_MID, 0.82, 1.16),
            tint(C_PETAL_MID, 1.10, 0.84),
            warm,
        );
        // Warm flowers fade through coral into hot peach at the tips; cool ones
        // keep a sharp, slightly bluish scarlet.
        let tip = lerp_color(
            tint(C_PETAL_TIP, 0.90, 1.10),
            C_PETAL_HOT,
            0.08 + warm * 0.60,
        );
        let hot = lerp_color(C_PETAL_HOT, C_ANTHER, 0.10 + warm * 0.38);
        let glow = lerp_color(
            tint(C_GLOW, 0.85, 1.30),
            tint(C_GLOW, 1.35, 0.72),
            warm,
        );
        let vein = lerp_color(mid, hot, 0.42);
        let anther = lerp_color(C_ANTHER, C_PETAL_TIP, 0.34 * (1.0 - warm));
        let stem_lo = lerp_color(C_STEM_DARK, tint(C_STEM_DARK, 1.28, 0.82), warm * 0.55);
        let stem_hi = lerp_color(C_STEM_LIGHT, tint(C_STEM_LIGHT, 1.22, 0.86), warm * 0.45);

        Self {
            deep,
            mid,
            tip,
            hot,
            glow,
            vein,
            anther,
            stem_lo,
            stem_hi,
        }
    }
}

/// Atmospheric perspective: distant flowers sink toward the night background
/// and lose saturation instead of merely getting darker.
#[inline]
fn atmos(c: (u8, u8, u8), depth: f32) -> (u8, u8, u8) {
    if depth >= 0.98 {
        c
    } else {
        lerp_color(c, C_DEEP_BG, (1.0 - depth) * 0.34)
    }
}

// ============================================================================
// Sprites
// ============================================================================

#[derive(Copy, Clone)]
struct Petal {
    angle: f32,
    length: f32,  // cells
    width: f32,   // dots (radius)
    curl: f32,    // radians the petal bends along its length
    recurve: f32, // how much the tip lifts back up
    twist: f32,   // direction the petal sags when the flower wilts
    phase: f32,
    hue_bias: f32, // tiny per-petal red-channel shift
    vein: bool,    // draw the central highlight?
}

#[derive(Copy, Clone)]
struct Stamen {
    angle: f32,
    length: f32,
    curl: f32,
    recurve: f32,
    phase: f32,
    speed: f32,
}

/// A drop of dew living on a petal (or, with `anchor == 0`, on the stem).
///
/// It swells, glints, drains and respawns on its own clock, so nothing outside
/// the flower has to track it — which is why it can live inside [`Flower`].
#[derive(Copy, Clone)]
struct Dew {
    anchor: u8, // 0 = stem, 1..=PETAL_COUNT = petal index + 1
    t: f32,     // position along the anchor in 0..1
    size: f32,
    phase: f32,
    timer: f32,
    cycle: f32,
}

struct Flower {
    base_x: f32,
    stem_height: f32,
    lean: f32,
    curve_amp: f32,
    curve_freq: f32,
    curve_phase: f32,
    sway_phase: f32,
    sway_speed: f32,
    sway_speed2: f32, // second harmonic multiplier => non-repeating sway
    sway_amp: f32,
    head_rotation: f32,
    growth: f32,
    grow_time: f32,
    age: f32,
    bloom_time: f32,
    hold_time: f32,
    wilt_time: f32,
    bloom: f32,
    wilt: f32, // 0 = fresh, 1 = fully sagging
    dead: bool,
    petals: [Petal; PETAL_COUNT],
    stamens: [Stamen; STAMEN_COUNT],
    dew: [Dew; DEW_COUNT],
    hue: f32,
    depth: f32,
    seed: u32,
    fall_accum: f32,
}

#[derive(Copy, Clone)]
struct FallingPetal {
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
    depth: f32,
    hue: f32,
    landed: bool,
    rest: f32, // seconds spent on the soil
}

/// Everything a stroke needs from the simulation, copied once per frame.
#[derive(Copy, Clone)]
struct Ctx {
    time: f32,
    ground_y: f32,
    wind: f32, // signed, -1..1
    gust: f32, // positive half of `wind`, drives flutter
    breath: f32,
}

/// A flower head flattened into the values the drawing code needs.
struct HeadView {
    hx: f32,
    hy: f32,
    rot: f32,
    bloom: f32,
    droop: f32,
    depth: f32,
    breath: f32,
    dim: f32, // depth * breath, multiplied into every colour
    pal: Palette,
}

// ============================================================================
// Cached soil
// ============================================================================

struct MossDot {
    dx: i32,
    dy: i32, // rows above the bottom dot of the ground
    bright: bool,
}

struct Tuft {
    x: f32,
    blade: f32,
    phase: f32,
    lean: f32,
    bright: bool,
}

#[derive(Default)]
struct Ground {
    width: u16,
    moss: Vec<MossDot>,
    tufts: Vec<Tuft>,
}

impl Ground {
    /// Lay out the soil once. Only the sway is evaluated per frame.
    fn build(width: u16) -> Self {
        let mut moss = Vec::with_capacity(width as usize);
        let mut tufts = Vec::with_capacity(width as usize / 8 + 4);

        for x in 0..width as i32 {
            let h = hash(x as u32 ^ 0x51ED_2701);

            if h % 9 < 3 {
                moss.push(MossDot {
                    dx: x * 2 + (h & 1) as i32,
                    dy: 0,
                    bright: h % 5 == 0,
                });
            }
            // A sparse second row gives the soil some depth.
            if h % 23 == 0 {
                moss.push(MossDot {
                    dx: x * 2 + ((h >> 5) & 1) as i32,
                    dy: 1,
                    bright: true,
                });
            }

            if h % 17 == 0 || h % 71 == 0 {
                tufts.push(Tuft {
                    x: x as f32 * 2.0 + ((h >> 3) & 1) as f32,
                    blade: 2.0 + (h % 5) as f32,
                    phase: x as f32 * 0.6,
                    lean: if h % 17 == 0 { 0.9 } else { 0.45 },
                    bright: h % 3 == 0,
                });
            }
        }

        Self {
            width,
            moss,
            tufts,
        }
    }
}

pub struct SpiderLily {
    flowers: Vec<Flower>,
    falling: Vec<FallingPetal>,
    ground: Ground,
    time: f32,
    width: u16,
    height: u16,
    target: usize,
    next_roll: f32,
    wind: f32,
    wind_target: f32,
    gust_timer: f32,
}

impl SpiderLily {
    pub fn new() -> Self {
        Self {
            flowers: Vec::new(),
            falling: Vec::new(),
            ground: Ground::build(80),
            time: 0.0,
            width: 80,
            height: 24,
            target: 0,
            next_roll: 0.0,
            wind: 0.0,
            wind_target: 0.0,
            gust_timer: 2.0,
        }
    }

    fn ground_y(&self) -> f32 {
        (self.height as f32 - 3.0).max(4.0)
    }

    fn pick_x(&self, rng: &mut ChaCha8Rng) -> f32 {
        let margin = (self.width as f32 * 0.15).clamp(10.0, 20.0);
        let lo = margin;
        let hi = (self.width as f32 - margin).max(lo + 2.0);

        let mut best = rng.gen_range(lo..hi);
        let mut best_distance = -1.0_f32;

        for _ in 0..10 {
            let x = rng.gen_range(lo..hi);
            let distance = self
                .flowers
                .iter()
                .map(|f| (f.base_x - x).abs())
                .fold(f32::INFINITY, f32::min);

            if distance > best_distance {
                best_distance = distance;
                best = x;
            }
            if distance > 18.0 {
                break;
            }
        }
        best
    }

    fn spawn_flower(&self, x: f32, rng: &mut ChaCha8Rng) -> Flower {
        let ground_y = self.ground_y();
        let min_height = (ground_y * 0.38).max(5.0);
        let max_height = (ground_y * 0.60).max(min_height + 2.0);
        let stem_height = rng.gen_range(min_height..max_height);
        let head_y = (ground_y - stem_height).max(3.0);

        // Short stems are "closer" to the viewer: bigger, brighter, more detail.
        let depth = (0.62 + 0.38 * (1.0 - stem_height / ground_y)).clamp(0.60, 1.0);

        let max_len = (head_y * 0.95)
            .min(self.width as f32 * 0.10)
            .clamp(4.0, 13.0);

        let width_scale = (self.width as f32 / 100.0).clamp(0.72, 1.10);
        let curl_sign = if rng.gen::<bool>() { 1.0 } else { -1.0 };
        let hero: usize = rng.gen_range(0..PETAL_COUNT);
        let vein_petal: usize = rng.gen_range(0..PETAL_COUNT);

        let blank_petal = Petal {
            angle: 0.0,
            length: 0.0,
            width: 0.0,
            curl: 0.0,
            recurve: 0.0,
            twist: 0.0,
            phase: 0.0,
            hue_bias: 1.0,
            vein: false,
        };
        let mut petals = [blank_petal; PETAL_COUNT];

        for (i, petal) in petals.iter_mut().enumerate() {
            let base = i as f32 * TAU / PETAL_COUNT as f32 + rng.gen_range(-0.12..0.12);
            let len = if i == hero {
                max_len * rng.gen_range(1.00..1.08)
            } else {
                max_len * rng.gen_range(0.70..1.00)
            };

            *petal = Petal {
                angle: base,
                length: len,
                width: rng.gen_range(0.90..1.30) * width_scale,
                curl: curl_sign * rng.gen_range(0.80..1.40),
                recurve: rng.gen_range(0.60..1.20),
                twist: curl_sign * rng.gen_range(0.30..0.70),
                phase: rng.gen_range(0.0..TAU),
                hue_bias: rng.gen_range(0.92..1.08),
                vein: i == vein_petal,
            };
        }

        let stamen_max = (head_y * 1.30).max(max_len * 1.1);
        let blank_stamen = Stamen {
            angle: 0.0,
            length: 0.0,
            curl: 0.0,
            recurve: 0.0,
            phase: 0.0,
            speed: 0.0,
        };
        let mut stamens = [blank_stamen; STAMEN_COUNT];

        for (i, stamen) in stamens.iter_mut().enumerate() {
            *stamen = Stamen {
                angle: -std::f32::consts::FRAC_PI_2
                    + (i as f32 - 2.0) * 0.62
                    + rng.gen_range(-0.15..0.15),
                length: (max_len * rng.gen_range(0.70..1.00)).min(stamen_max),
                curl: rng.gen_range(-0.22..0.22),
                recurve: rng.gen_range(0.55..1.10),
                phase: rng.gen_range(0.0..TAU),
                speed: rng.gen_range(0.12..0.40),
            };
        }

        // Dew anchors: mostly on petal tips, sometimes on the stalk.
        let blank_dew = Dew {
            anchor: 0,
            t: 0.0,
            size: 0.0,
            phase: 0.0,
            timer: 0.0,
            cycle: 1.0,
        };
        let mut dew = [blank_dew; DEW_COUNT];
        for drop in dew.iter_mut() {
            let on_petal = rng.gen::<bool>();
            *drop = Dew {
                anchor: if on_petal {
                    1 + rng.gen_range(0..PETAL_COUNT as u8)
                } else {
                    0
                },
                t: if on_petal {
                    rng.gen_range(0.55..0.92)
                } else {
                    rng.gen_range(0.30..0.80)
                },
                size: rng.gen_range(0.55..1.0),
                phase: rng.gen_range(0.0..TAU),
                timer: rng.gen_range(0.0..22.0),
                cycle: rng.gen_range(14.0..30.0),
            };
        }

        let grow_time: f32 = rng.gen_range(10.0..18.0);
        let bloom_time: f32 = rng.gen_range(35.0..70.0);
        let hold_time: f32 = rng.gen_range(100.0..240.0);
        let wilt_time: f32 = rng.gen_range(40.0..75.0);

        let age: f32 = rng.gen_range(grow_time..(grow_time + bloom_time + hold_time * 0.70));
        let growth = smoothstep((age / grow_time).clamp(0.0, 1.0));

        Flower {
            base_x: x,
            stem_height,
            lean: rng.gen_range(-3.5..3.5),
            curve_amp: rng.gen_range(0.80..2.20),
            curve_freq: rng.gen_range(1.2..2.2),
            curve_phase: rng.gen_range(0.0..TAU),
            sway_phase: rng.gen_range(0.0..TAU),
            sway_speed: rng.gen_range(0.055..0.135),
            sway_speed2: rng.gen_range(0.32..0.74),
            sway_amp: rng.gen_range(1.2..3.2),
            head_rotation: rng.gen_range(-0.45..0.45),
            growth,
            grow_time,
            age,
            bloom_time,
            hold_time,
            wilt_time,
            bloom: 0.0,
            wilt: 0.0,
            dead: false,
            petals,
            stamens,
            dew,
            hue: rng.gen_range(-1.0..1.0),
            depth,
            seed: rng.gen(),
            fall_accum: rng.gen_range(0.0..1.0),
        }
    }

    fn ensure_flowers(&mut self, rng: &mut ChaCha8Rng) {
        if self.target == 0 || self.time >= self.next_roll {
            let base = ((self.width as f32 / 42.0).round() as usize).clamp(2, 5);
            let roll = rng.gen::<f32>();
            let bonus = if roll < 0.02 {
                3
            } else if roll < 0.10 {
                2
            } else if roll < 0.30 {
                1
            } else {
                0
            };
            self.target = (base + bonus).min(6);
            self.next_roll = self.time + rng.gen_range(55.0..120.0);
        }

        let mut guard = 0;
        while self.flowers.len() < self.target && guard < 8 {
            guard += 1;
            let x = self.pick_x(rng);
            let flower = self.spawn_flower(x, rng);
            self.flowers.push(flower);
        }
    }

    /// Breeze model: long calm stretches, occasional gusts that bend the whole
    /// garden. Smoothed with an exponential follow so stems never snap.
    fn update_wind(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.gust_timer -= dt;
        if self.gust_timer <= 0.0 {
            let roll = rng.gen::<f32>();
            self.wind_target = if roll < 0.18 {
                // A real gust: one direction, sometimes barely there.
                rng.gen_range(0.45..1.0)
            } else if roll < 0.50 {
                rng.gen_range(-0.26..0.26)
            } else {
                rng.gen_range(-0.08..0.12)
            };
            self.gust_timer = rng.gen_range(5.0..18.0);
        }

        let k = 1.0 - (-dt * 0.55).exp();
        self.wind += (self.wind_target - self.wind) * k;
        self.wind = self.wind.clamp(-1.2, 1.2);
    }
}

// ============================================================================
// Geometry
// ============================================================================

/// Position along a stem, `s` from 0 (soil) to 1 (flower head), in cells.
fn stem_point(flower: &Flower, ctx: &Ctx, s: f32) -> (f32, f32) {
    let height = flower.stem_height * flower.growth;

    // Two harmonics with a non-integer speed ratio: the sway never visibly
    // repeats, which is what makes it read as wind and not as a metronome.
    let speed = flower.sway_speed * (1.0 + 0.18 * (ctx.time * 0.05 + flower.sway_phase).sin());
    let sway1 = (ctx.time * speed + flower.sway_phase).sin();
    let sway2 = (ctx.time * speed * flower.sway_speed2 + flower.sway_phase * 1.7 + 1.9).sin();

    let gust = ctx.wind * flower.sway_amp * 1.45;
    let bend = (flower.lean + sway1 * flower.sway_amp + sway2 * flower.sway_amp * 0.32 + gust)
        * s.powf(1.7);

    // A slight S-curve in the stalk so no two stems look machine-straight.
    let wiggle = (s * std::f32::consts::PI * flower.curve_freq + flower.curve_phase).sin()
        * flower.curve_amp
        * s
        * (1.0 - s)
        * 4.0;

    (flower.base_x + bend + wiggle, ctx.ground_y - height * s)
}

/// Point on a petal spine at `t` in 0..=1, in cells.
fn petal_point(petal: &Petal, view: &HeadView, t: f32, len: f32, ctx: &Ctx) -> (f32, f32) {
    let base_angle = petal.angle + view.rot;
    let bend = petal.curl * t * t * 1.20;
    // Flutter grows with the gust: a still night barely moves the petals.
    let wave = (t * 6.5 + petal.phase + ctx.time * 0.35).sin()
        * 0.045
        * t
        * (1.0 + 1.6 * ctx.gust);
    // Wilting petals fold inward and sag.
    let sag = view.droop * t * t * 0.85 * petal.twist;
    let angle = base_angle + bend + wave + sag;

    let inner = (len * 0.22).min(2.2);
    let radius = inner + (len - inner) * t;
    let x = view.hx + angle.cos() * radius;
    let mut y = view.hy + angle.sin() * radius * 0.52;

    y -= petal.recurve * t.powi(3) * len * 0.30 * (0.35 + 0.65 * view.bloom);
    y += view.droop * t * t * len * 0.20;
    (x, y)
}

fn new_falling(x: f32, y: f32, depth: f32, hue: f32, rng: &mut ChaCha8Rng) -> FallingPetal {
    let direction = if rng.gen::<bool>() { 1.0 } else { -1.0 };
    FallingPetal {
        x,
        y,
        vx: direction * rng.gen_range(0.02..0.16),
        vy: rng.gen_range(0.16..0.42),
        angle: rng.gen_range(0.0..TAU),
        spin: rng.gen_range(-0.30..0.30),
        phase: rng.gen_range(0.0..TAU),
        life: 0.0,
        max_life: rng.gen_range(18.0..42.0),
        size: rng.gen_range(0.6..1.4),
        depth,
        hue,
        landed: false,
        rest: 0.0,
    }
}

// ============================================================================
// Theme implementation
// ============================================================================

impl Theme for SpiderLily {
    fn name(&self) -> &'static str {
        "spider-lily"
    }

    fn description(&self) -> &'static str {
        "彼岸花 — a slow night garden of red spider lilies"
    }

    fn resize(&mut self, area: Rect) {
        let changed = area.width != self.width || area.height != self.height;
        self.width = area.width;
        self.height = area.height;

        if self.ground.width != area.width || self.ground.moss.is_empty() {
            self.ground = Ground::build(area.width);
        }

        if changed || self.flowers.is_empty() {
            self.flowers.clear();
            self.falling.clear();
            self.time = 0.0;
            self.target = 0;
            self.next_roll = 0.0;
        }
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        // A long tab-switch must not teleport the simulation forward.
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.update_wind(dt, rng);
        self.ensure_flowers(rng);

        let ground_y = self.ground_y();
        let time = self.time;
        let wind = self.wind;
        let gust = wind.max(0.0);
        let ctx = Ctx {
            time,
            ground_y,
            wind,
            gust,
            breath: 1.0,
        };

        let Self {
            flowers, falling, ..
        } = self;

        for flower in flowers.iter_mut() {
            flower.age += dt;
            flower.growth = smoothstep((flower.age / flower.grow_time).clamp(0.0, 1.0));

            let t = flower.age - flower.grow_time * 0.5;
            let bloom = if t <= 0.0 {
                0.0
            } else if t < flower.bloom_time {
                let u = t / flower.bloom_time;
                1.0 - (1.0 - u).powi(3)
            } else if t < flower.bloom_time + flower.hold_time {
                1.0
            } else {
                let u = ((t - flower.bloom_time - flower.hold_time) / flower.wilt_time)
                    .clamp(0.0, 1.0);
                1.0 - smoothstep(u)
            };
            flower.bloom = bloom;

            // Wilt rises before the bloom starts to fade, so the petals begin
            // to sag while there is still length left to show it.
            let wilting = t > flower.bloom_time + flower.hold_time;
            let wilt_target = if wilting {
                1.0
            } else if t > flower.bloom_time * 0.85 {
                0.30
            } else {
                0.0
            };
            flower.wilt += (wilt_target - flower.wilt) * (dt * 0.55).min(1.0);

            // Dew clocks.
            for drop in flower.dew.iter_mut() {
                drop.timer += dt;
                if drop.timer >= drop.cycle {
                    drop.timer -= drop.cycle;
                    // Re-roll the bead so successive drops are not identical.
                    drop.phase += 1.7;
                }
            }

            // Wind shakes ripe flowers loose, so a gust visibly rains petals.
            let rate = if wilting {
                1.35
            } else if bloom > 0.97 {
                0.05
            } else {
                0.20
            };
            flower.fall_accum += rate * dt * (1.0 + gust * 1.8);

            if flower.fall_accum >= 1.0
                && flower.age > flower.grow_time * 0.7
                && falling.len() < MAX_FALLING
            {
                flower.fall_accum = 0.0;
                let (hx, hy) = stem_point(flower, &ctx, 1.0);
                let angle = rng.gen_range(0.0..TAU);
                let radius = rng.gen_range(0.4..2.6);
                falling.push(new_falling(
                    hx + angle.cos() * radius,
                    hy + angle.sin() * radius * 0.5,
                    flower.depth,
                    flower.hue,
                    rng,
                ));
            }

            if t > flower.bloom_time + flower.hold_time + flower.wilt_time {
                flower.dead = true;
                let (hx, hy) = stem_point(flower, &ctx, 1.0);
                let burst = rng.gen_range(3..7);
                for _ in 0..burst {
                    if falling.len() >= MAX_FALLING {
                        break;
                    }
                    let angle = rng.gen_range(0.0..TAU);
                    let radius = rng.gen_range(0.0..2.2);
                    falling.push(new_falling(
                        hx + angle.cos() * radius,
                        hy + angle.sin() * radius * 0.5,
                        flower.depth,
                        flower.hue,
                        rng,
                    ));
                }
            }
        }

        flowers.retain(|flower| !flower.dead);

        falling.retain_mut(|petal| {
            petal.life += dt;

            if petal.landed {
                // Settled petals creep a little with the breeze, flatten out
                // and fade instead of vanishing mid-air.
                petal.rest += dt;
                petal.x += wind * 0.05 * dt;
                petal.angle += (0.0 - petal.angle) * (dt * 0.9).min(1.0);
                return petal.rest < 5.0;
            }

            let flutter = (time * 1.25 + petal.phase).sin();
            petal.x += (petal.vx + flutter * 0.30 + wind * 0.35) * dt;
            petal.y += petal.vy * dt;
            // Terminal velocity: petals speed up, then float at a steady rate.
            petal.vy = (petal.vy + 0.010 * dt).min(0.85);
            petal.angle += petal.spin * (1.0 + wind.abs() * 0.8) * dt;

            if petal.y >= ground_y {
                petal.y = ground_y;
                petal.landed = true;
                petal.rest = 0.0;
            }

            petal.life < petal.max_life
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        // The soil layout is static; rebuilding it here would be the single
        // most expensive thing in the frame.
        if self.ground.width != area.width {
            self.ground = Ground::build(area.width);
        }

        let ground_y = (area.height as f32 - 3.0).max(4.0);
        let breath = 0.93 + 0.07 * (self.time * 0.22).sin();
        let ctx = Ctx {
            time: self.time,
            ground_y,
            wind: self.wind,
            gust: self.wind.max(0.0),
            breath,
        };

        self.draw_ground(buf, area, &ctx);
        self.draw_motes(buf, area, &ctx);

        for flower in &self.flowers {
            self.draw_stem(buf, area, flower, &ctx);
            self.draw_head(buf, area, flower, &ctx);
        }

        self.draw_falling(buf, area, &ctx);
    }
}

impl SpiderLily {
    // ------------------------------------------------------------------
    // Soil
    // ------------------------------------------------------------------

    fn draw_ground(&self, buf: &mut Buffer, area: Rect, ctx: &Ctx) {
        let ground_row = ctx.ground_y.round() as i32;
        let bottom = ground_row * 4 + 3;
        let width = area.width as i32;

        // Moss: positions are cached, only the alpha breathes.
        let breathe = 0.78 + 0.22 * ctx.breath;
        for dot in &self.ground.moss {
            let base = if dot.bright { C_MOSS_LIGHT } else { C_MOSS };
            plot_dot(buf, area, dot.dx, bottom - dot.dy, mul_color(base, breathe));
        }

        // Grass blades bend with the same wind field as the stems.
        for tuft in &self.ground.tufts {
            let sway = (ctx.time * 0.33 + tuft.phase).sin() * tuft.lean
                + ctx.wind * tuft.blade * 0.55;
            let tip = if tuft.bright { C_MOSS_LIGHT } else { C_MOSS };
            draw_dot_line(
                buf,
                area,
                tuft.x,
                bottom as f32,
                tuft.x + sway,
                bottom as f32 - tuft.blade,
                0.5,
                0.32,
                mul_color(C_MOSS, breathe),
                mul_color(tip, breathe),
            );
        }

        // Low mist: a broken line one dot above the soil that drifts sideways.
        if width > 30 {
            for x in (1..width).step_by(3) {
                let h = hash(x as u32 * 7 + 11);
                if h % 3 != 0 {
                    continue;
                }
                let drift = (ctx.time * 0.16 + x as f32 * 0.21).sin();
                if drift < -0.2 {
                    continue;
                }
                let color = mul_color(C_MOSS_LIGHT, 0.16 + 0.12 * drift.max(0.0));
                plot_dot(
                    buf,
                    area,
                    x * 2 + (h & 1) as i32,
                    bottom - 1 - (h % 3) as i32,
                    color,
                );
            }
        }

        // A little mound of moss where every stem enters the soil ties the
        // flower to the ground instead of letting it float.
        for flower in &self.flowers {
            let cx = flower.base_x * 2.0;
            for i in 0..3 {
                let h = hash(flower.seed.wrapping_add(i * 7919));
                let dx = cx + (i as f32 - 1.0) * 1.3 + ((h & 3) as f32 - 1.5) * 0.4;
                let dy = (h % 3) as i32;
                stamp_disc(
                    buf,
                    area,
                    dx,
                    (bottom - dy) as f32,
                    0.5,
                    mul_color(C_MOSS_LIGHT, 0.55 + 0.25 * breathe),
                );
            }
        }
    }

    /// Pollen motes: a handful of very dim dots drifting through the night.
    /// Fully time-derived, so they cost no state at all.
    fn draw_motes(&self, buf: &mut Buffer, area: Rect, ctx: &Ctx) {
        let count = (area.width as usize / 28).clamp(3, 10) as u32;
        for i in 0..count {
            let seed = hash(i.wrapping_mul(0x9E37_79B9) ^ 0x1F_2A);
            let base_x = (seed % 997) as f32 / 997.0 * area.width as f32;
            let phase = ((seed >> 10) & 1023) as f32 / 1023.0 * TAU;
            let speed = 0.05 + ((seed >> 20) & 15) as f32 * 0.008;

            let drift = ((ctx.time * speed + phase).sin() + 1.0) * 0.5;
            let x = base_x + (drift - 0.5) * 9.0 + ctx.wind * 2.4;
            let y = 2.0
                + (area.height as f32 - 6.0)
                    * (((ctx.time * speed * 0.7 + phase * 1.7).sin() + 1.0) * 0.5);

            let twinkle = 0.25 + 0.75 * ((ctx.time * 1.7 + phase).sin().max(0.0)).powi(3);
            let color = mul_color(C_PETAL_HOT, 0.10 * twinkle * ctx.breath);
            stamp_disc(buf, area, x * 2.0, y * 4.0, 0.5, color);
        }
    }

    // ------------------------------------------------------------------
    // Stems
    // ------------------------------------------------------------------

    fn draw_stem(&self, buf: &mut Buffer, area: Rect, flower: &Flower, ctx: &Ctx) {
        if flower.growth <= 0.02 {
            return;
        }

        let pal = Palette::new(flower.hue);
        let fade = (0.30 + 0.70 * flower.growth) * flower.depth * ctx.breath;

        // One sample per ~2 braille dots instead of a fixed 80: a short stem
        // used to stamp the same three dots over and over.
        let len_dots = (flower.stem_height * flower.growth * 4.0).max(2.0);
        let steps = ((len_dots * 0.5).ceil() as usize).clamp(3, 56);

        let (mut px, mut py) = stem_point(flower, ctx, 0.0);
        let mut pr = 0.92;
        let mut pc = atmos(mul_color(pal.stem_lo, fade * 0.85), flower.depth);

        for i in 1..=steps {
            let s = i as f32 / steps as f32;
            let (x, y) = stem_point(flower, ctx, s);
            let radius = 0.92 + (0.34 - 0.92) * s;
            let color = atmos(
                mul_color(lerp_color(pal.stem_lo, pal.stem_hi, s * 0.90), fade),
                flower.depth,
            );

            draw_dot_line(
                buf,
                area,
                px * 2.0,
                py * 4.0,
                x * 2.0,
                y * 4.0,
                pr,
                radius,
                pc,
                color,
            );

            px = x;
            py = y;
            pr = radius;
            pc = color;
        }

        // Specular edge: a hairline of lighter green on the light side of the
        // stalk. Only near flowers, where one dot of contrast is legible.
        if flower.depth > 0.84 && flower.growth > 0.35 {
            let hl = mul_color(pal.stem_hi, fade * 0.30);
            let mut prev: Option<(f32, f32)> = None;
            let hs = (steps / 2).max(3);
            for i in 0..=hs {
                let s = 0.15 + 0.82 * i as f32 / hs as f32;
                let (x, y) = stem_point(flower, ctx, s);
                let x = x - 0.55; // light comes from the left
                if let Some((ax, ay)) = prev {
                    draw_dot_line(
                        buf,
                        area,
                        ax * 2.0,
                        ay * 4.0,
                        x * 2.0,
                        y * 4.0,
                        0.28,
                        0.28,
                        hl,
                        hl,
                    );
                }
                prev = Some((x, y));
            }
        }

        // Small bract at the foot of the stalk.
        let (bx, by) = stem_point(flower, ctx, 0.0);
        let bract = atmos(mul_color(pal.stem_lo, fade * 0.9), flower.depth);
        stamp_disc(buf, area, bx * 2.0 - 1.3, by * 4.0, 0.5, bract);
        stamp_disc(buf, area, bx * 2.0 + 1.4, by * 4.0 - 0.6, 0.5, bract);
    }

    // ------------------------------------------------------------------
    // Flower head
    // ------------------------------------------------------------------

    fn draw_head(&self, buf: &mut Buffer, area: Rect, flower: &Flower, ctx: &Ctx) {
        let (hx, hy) = stem_point(flower, ctx, 1.0);
        let (tx, ty) = stem_point(flower, ctx, 0.96);

        let tilt = ((hx - tx).atan2((ty - hy).max(0.0001)) * 0.65).clamp(-0.55, 0.55);
        let rot = tilt
            + flower.head_rotation
            + (ctx.time * 0.045 + flower.sway_phase * 1.3).sin() * 0.035
            + ctx.wind * 0.06;

        if flower.bloom < 0.015 {
            self.draw_bud(buf, area, hx, hy, flower, ctx);
            return;
        }

        let view = HeadView {
            hx,
            hy,
            rot,
            bloom: flower.bloom,
            droop: flower.wilt,
            depth: flower.depth,
            breath: ctx.breath,
            dim: flower.depth * ctx.breath,
            pal: Palette::new(flower.hue),
        };

        self.draw_aura(buf, area, &view, flower.sway_phase, ctx);

        // Stamens first so the petals can shade over their bases.
        self.draw_stamens(buf, area, flower, &view, ctx);

        for petal in flower.petals.iter() {
            self.draw_petal(buf, area, petal, &view, ctx);
        }

        // Heart: forced dark so the petals always read as radiating out of a
        // hole rather than a blob.
        let core = mul_color(atmos(view.pal.deep, view.depth), view.dim * view.bloom * 0.85);
        stamp_disc_force(
            buf,
            area,
            hx * 2.0,
            hy * 4.0,
            0.85 + view.bloom * 0.25,
            core,
        );

        // Warm crown just above the heart, then the pollen bead in the middle.
        stamp_disc(
            buf,
            area,
            hx * 2.0,
            hy * 4.0 - 0.9,
            0.5,
            mul_color(view.pal.hot, view.dim * view.bloom * 0.32),
        );
        stamp_disc(
            buf,
            area,
            hx * 2.0,
            hy * 4.0,
            0.5,
            mul_color(view.pal.anther, view.dim * view.bloom * 0.55),
        );

        self.draw_dew(buf, area, flower, &view, ctx);
        self.draw_flower_glint(buf, area, flower, &view, ctx);
    }

    fn draw_bud(
        &self,
        buf: &mut Buffer,
        area: Rect,
        hx: f32,
        hy: f32,
        flower: &Flower,
        ctx: &Ctx,
    ) {
        let pal = Palette::new(flower.hue);
        let depth = flower.depth * ctx.breath;
        let dark = atmos(mul_color(pal.deep, depth * 0.85), flower.depth);
        let warm = atmos(mul_color(pal.mid, depth * 0.60), flower.depth);

        // The bud leans very slightly, at its own pace.
        let sway = (ctx.time * 0.8 + flower.sway_phase).sin() * 0.25;

        for i in 0..5 {
            let u = i as f32 / 4.0;
            let yy = hy * 4.0 + (u - 0.5) * 2.6;
            let radius = 0.45 + 0.35 * (std::f32::consts::PI * u).sin();
            let color = if u > 0.6 {
                lerp_color(dark, warm, (u - 0.6) / 0.4)
            } else {
                dark
            };
            stamp_disc(buf, area, hx * 2.0 + sway * u, yy, radius, color);
        }

        // Wet tip: the first hint of red before the petals open.
        stamp_disc(
            buf,
            area,
            hx * 2.0 + sway,
            hy * 4.0 - 1.6,
            0.5,
            atmos(mul_color(pal.tip, depth * 0.55), flower.depth),
        );
        stamp_disc(
            buf,
            area,
            hx * 2.0,
            hy * 4.0 + 2.4,
            0.5,
            atmos(mul_color(pal.stem_hi, depth), flower.depth),
        );
    }

    /// Soft elliptical halo. Kept to a few discs: it is atmosphere, not light.
    fn draw_aura(
        &self,
        buf: &mut Buffer,
        area: Rect,
        view: &HeadView,
        phase: f32,
        ctx: &Ctx,
    ) {
        if view.bloom < 0.10 {
            return;
        }

        let pulse = 0.82 + 0.18 * (ctx.time * 0.30 + phase).sin();
        let intensity = view.bloom * view.depth * view.breath * pulse * 0.34;
        if intensity < 0.02 {
            return;
        }

        // A very faint disc under the whole head glues the petals together.
        stamp_disc(
            buf,
            area,
            view.hx * 2.0,
            view.hy * 4.0,
            2.2 + view.bloom * 0.8,
            mul_color(view.pal.glow, intensity * 0.55),
        );

        let discs = if view.depth > 0.85 { 7 } else { 5 };
        for i in 0..discs {
            // Golden angle => even scatter without storing any positions.
            let angle = i as f32 * 2.399_963 + phase + ctx.wind * 0.12;
            let radius = 3.2 + ((i * 7) % 9) as f32 * 0.62;
            let falloff = (1.0 - radius / 11.0).max(0.0);
            if falloff <= 0.0 {
                continue;
            }

            let color = mul_color(view.pal.glow, intensity * falloff);
            let x = view.hx + angle.cos() * radius;
            let y = view.hy + angle.sin() * radius * 0.5;
            stamp_disc(buf, area, x * 2.0, y * 4.0, 0.55, color);
        }
    }

    fn draw_petal(
        &self,
        buf: &mut Buffer,
        area: Rect,
        petal: &Petal,
        view: &HeadView,
        ctx: &Ctx,
    ) {
        let len = petal.length * (0.10 + 0.90 * view.bloom);
        if len < 0.8 {
            return;
        }

        // Sampling is fine (one spine sample every fifth of a cell) because the
        // duplicate-dot skip below throws most of those samples away anyway.
        // Far petals keep fewer samples; the loss is invisible at that depth.
        let lod = 0.65 + 0.35 * view.depth;
        let steps = ((len * 5.0 * lod).clamp(8.0, 64.0)) as usize;

        // Petals pointing up catch the (imaginary) moonlight, lower ones sit in
        // shade. Computed once per petal, not per sample.
        let up = (-(petal.angle + view.rot).sin()).max(0.0);
        let light = (0.80 + 0.38 * up) * view.dim * (0.72 + 0.28 * view.bloom);
        let vein_color = mul_color(atmos(view.pal.vein, view.depth), light * 0.30);

        let mut last_dot: Option<(i32, i32)> = None;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let (x, y) = petal_point(petal, view, t, len, ctx);

            let dx = (x * 2.0).round() as i32;
            let dy = (y * 4.0).round() as i32;
            // Consecutive samples often land on the same braille dot; stamping
            // it twice is pure waste.
            if last_dot == Some((dx, dy)) {
                continue;
            }
            last_dot = Some((dx, dy));

            let profile = (std::f32::consts::PI * t.min(0.999)).sin().powf(0.80);
            let width = (petal.width * (0.26 + 0.74 * profile)).max(0.55);

            // Three-stop gradient: dark base -> body -> hot tip. The extra stop
            // at 0.74 keeps the tip from jumping straight to peach.
            let base = if t < 0.34 {
                lerp_color(view.pal.deep, view.pal.mid, t / 0.34)
            } else if t < 0.74 {
                lerp_color(view.pal.mid, view.pal.tip, (t - 0.34) / 0.40)
            } else {
                lerp_color(view.pal.tip, view.pal.hot, (t - 0.74) / 0.26 * 0.75)
            };
            let color = atmos(mul_color(tint(base, petal.hue_bias, 1.0), light), view.depth);

            stamp_disc(buf, area, dx as f32, dy as f32, width, color);

            // Central highlight: one dot every other sample, lifted a hair
            // above the spine so it reads as a vein, not a thicker petal.
            if petal.vein && view.depth > 0.82 && view.bloom > 0.45 && i % 2 == 0 {
                stamp_disc(buf, area, dx as f32, dy as f32 - 0.8, 0.5, vein_color);
            }
        }

        // Bright tip: the hottest point of a spider lily.
        if view.bloom > 0.45 {
            let heat = ((view.bloom - 0.45) / 0.55).clamp(0.0, 1.0);
            let (x, y) = petal_point(petal, view, 1.0, len, ctx);
            stamp_disc(
                buf,
                area,
                x * 2.0,
                y * 4.0,
                0.62 + 0.25 * heat,
                mul_color(atmos(view.pal.hot, view.depth), view.dim * heat * 0.95),
            );
        }
    }

    fn draw_stamens(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        view: &HeadView,
        ctx: &Ctx,
    ) {
        if view.bloom < 0.05 {
            return;
        }

        let dim = mul_color(atmos(C_STAMEN, view.depth), view.dim * 0.42);
        let bright = mul_color(atmos(C_STAMEN, view.depth), view.dim * 0.88);
        let anther = atmos(view.pal.anther, view.depth);
        let lod = 0.6 + 0.4 * view.depth;

        for stamen in &flower.stamens {
            let len = stamen.length * (0.25 + 0.75 * view.bloom);
            if len < 0.9 {
                continue;
            }

            // Each filament has its own speed, plus the shared gust, so the
            // bunch never moves in lockstep.
            let sway = (ctx.time * stamen.speed + stamen.phase).sin() * 0.05
                + ctx.gust * 0.10 * (0.4 + stamen.speed);
            let steps = ((len * 3.0 * lod).clamp(4.0, 34.0)) as usize;

            let mut previous: Option<(f32, f32, (u8, u8, u8))> = None;

            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let angle = stamen.angle
                    + view.rot
                    + stamen.curl * (t * 1.15).sin()
                    + sway * t
                    + view.droop * stamen.recurve * t * t * 0.5;
                let radius = len * t;
                let x = view.hx + angle.cos() * radius;
                let y = view.hy + angle.sin() * radius * 0.52
                    - stamen.recurve * t.powi(3) * len * 0.16
                    + view.droop * t * t * len * 0.10;

                let color = lerp_color(dim, bright, t);

                if let Some((px, py, pc)) = previous {
                    draw_dot_line(
                        buf,
                        area,
                        px * 2.0,
                        py * 4.0,
                        x * 2.0,
                        y * 4.0,
                        0.30,
                        0.30,
                        pc,
                        color,
                    );
                }
                previous = Some((x, y, color));
            }

            let angle = stamen.angle
                + view.rot
                + stamen.curl * 1.15f32.sin()
                + sway
                + view.droop * stamen.recurve * 0.5;
            let x = view.hx + angle.cos() * len;
            let y = view.hy + angle.sin() * len * 0.52 - stamen.recurve * len * 0.16
                + view.droop * len * 0.10;
            let pulse = 0.5 + 0.5 * (ctx.time * 0.45 + stamen.phase * 2.0).sin();

            // Soft halo, then the pollen bead itself, slightly elongated along
            // the filament so it hangs rather than floats.
            stamp_disc(
                buf,
                area,
                x * 2.0,
                y * 4.0,
                1.3,
                mul_color(anther, view.dim * 0.10),
            );
            stamp_disc(
                buf,
                area,
                x * 2.0,
                y * 4.0,
                0.40 + pulse * 0.22,
                mul_color(anther, view.dim * (0.55 + 0.35 * pulse)),
            );
            stamp_disc(
                buf,
                area,
                (x + angle.cos() * 0.5) * 2.0,
                (y + angle.sin() * 0.5) * 4.0,
                0.32,
                mul_color(anther, view.dim * 0.45),
            );

            // Pollen sparks: a four-dot glint at the peak of the pulse.
            if pulse > 0.88 {
                let strength = (pulse - 0.88) / 0.12;
                self.stamp_glint(buf, area, x, y, strength * view.dim * 0.85, view.pal.hot);
            }
        }
    }

    // ------------------------------------------------------------------
    // Details
    // ------------------------------------------------------------------

    /// Dew beads: they swell, throw a specular dot, then drain away.
    fn draw_dew(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        view: &HeadView,
        ctx: &Ctx,
    ) {
        // Dew on a distant flower is just noise: skip it below the threshold.
        if view.depth < 0.70 || view.bloom < 0.40 {
            return;
        }

        let len_scale = 0.10 + 0.90 * view.bloom;

        for drop in &flower.dew {
            let u = (drop.timer / drop.cycle).fract();
            let fill = smoothstep((u / 0.22).min(1.0))
                * (1.0 - smoothstep(((u - 0.72) / 0.28).max(0.0)));
            if fill <= 0.02 {
                continue;
            }

            let (x, y) = if drop.anchor == 0 {
                stem_point(flower, ctx, drop.t)
            } else {
                let petal = &flower.petals[(drop.anchor as usize - 1) % PETAL_COUNT];
                petal_point(petal, view, drop.t, petal.length * len_scale, ctx)
            };

            let radius = 0.32 + drop.size * 0.40 * fill;
            let body = mul_color(view.pal.tip, view.dim * fill * 0.55);
            let halo = mul_color(view.pal.glow, view.dim * fill * 0.45);

            stamp_disc(buf, area, x * 2.0, y * 4.0, radius + 0.45, halo);
            stamp_disc(buf, area, x * 2.0, y * 4.0, radius, body);

            // Single specular dot up-left: enough to read as a wet bead.
            let glint = (ctx.time * 0.7 + drop.phase).sin().max(0.0);
            stamp_disc(
                buf,
                area,
                x * 2.0 - 0.5,
                y * 4.0 - 0.9,
                0.5,
                mul_color(C_PETAL_HOT, view.dim * fill * (0.45 + 0.45 * glint)),
            );
        }
    }

    /// Rare specular éclat on a petal. Fully time-derived: no state, no timer.
    fn draw_flower_glint(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        view: &HeadView,
        ctx: &Ctx,
    ) {
        if view.bloom < 0.6 || flower.depth < 0.72 {
            return;
        }

        let wave = (ctx.time * 0.27 + flower.sway_phase * 2.3).sin();
        if wave <= 0.996 {
            return;
        }

        let strength = ((wave - 0.996) / 0.004).clamp(0.0, 1.0);
        let idx = (flower.seed as usize) % PETAL_COUNT;
        let petal = &flower.petals[idx];
        let len = petal.length * (0.10 + 0.90 * view.bloom);
        let (x, y) = petal_point(petal, view, 0.82, len, ctx);

        self.stamp_glint(buf, area, x, y, strength * view.dim * 0.75, view.pal.hot);
    }

    /// A four-dot cross plus a dim core: a cheap "sparkle".
    fn stamp_glint(
        &self,
        buf: &mut Buffer,
        area: Rect,
        x: f32,
        y: f32,
        strength: f32,
        color: (u8, u8, u8),
    ) {
        if strength <= 0.015 {
            return;
        }

        let dx = x * 2.0;
        let dy = y * 4.0;

        stamp_disc(buf, area, dx, dy, 1.0, mul_color(color, strength * 0.20));
        stamp_disc(buf, area, dx, dy, 0.5, mul_color(color, strength * 0.80));

        for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -0.8), (0.0, 0.8)] {
            stamp_disc(
                buf,
                area,
                dx + ox,
                dy + oy,
                0.5,
                mul_color(color, strength * 0.35),
            );
        }
    }

    // ------------------------------------------------------------------
    // Falling petals
    // ------------------------------------------------------------------

    fn draw_falling(&self, buf: &mut Buffer, area: Rect, ctx: &Ctx) {
        for petal in &self.falling {
            let life = (petal.life / petal.max_life).clamp(0.0, 1.0);
            let fade = (1.0 - life).powf(0.7);
            let settle = if petal.landed {
                (1.0 - (petal.rest / 5.0).clamp(0.0, 1.0)).powf(0.8)
            } else {
                1.0
            };

            // Hue still shows in the drifting petals, so they match the flower
            // they fell from.
            let base = lerp_color(C_FALLING, C_PETAL_HOT, (petal.hue * 0.5 + 0.5) * 0.35);
            let color = mul_color(base, petal.depth * ctx.breath * fade * settle * 0.9);

            let dx = petal.angle.cos();
            let dy = petal.angle.sin();

            if petal.landed {
                // A settled petal is a short flat streak on the soil.
                let len = 1.6 + petal.size * 1.5;
                for k in -1..=1 {
                    let off = k as f32 * len * 0.5;
                    stamp_disc(
                        buf,
                        area,
                        petal.x * 2.0 + off,
                        petal.y * 4.0 + dy * off * 0.18,
                        0.5,
                        color,
                    );
                }
                continue;
            }

            // Foreshorten with the spin: a petal turning edge-on narrows and
            // then flashes back to full width.
            let flip = petal.angle.cos().abs();
            let len = (1.2 + petal.size * 1.5) * (0.35 + 0.65 * flip);
            let half = len * 0.5;
            let tip = mul_color(color, 1.35);

            draw_dot_line(
                buf,
                area,
                (petal.x - dx * half) * 2.0,
                (petal.y - dy * half) * 4.0,
                (petal.x + dx * half) * 2.0,
                (petal.y + dy * half) * 4.0,
                0.30,
                0.55,
                color,
                tip,
            );

            // Occasional flash on the brighter face of the petal.
            if flip > 0.92 {
                stamp_disc(
                    buf,
                    area,
                    petal.x * 2.0,
                    petal.y * 4.0,
                    0.5,
                    mul_color(C_PETAL_HOT, petal.depth * fade * 0.35),
                );
            }
        }
    }
}