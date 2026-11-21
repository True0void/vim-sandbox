//! Spider Lily (彼岸花) — a slow, minimal night garden drawn in braille.
//!
//! Everything here is background art only: stems planted in the soil, long
//! recurved petals, gold stamens and a few petals drifting down. The editor
//! itself is untouched.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

/// Trait for an animated background theme.
pub trait Theme: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng);
    fn draw(&mut self, buf: &mut Buffer, area: Rect);
    fn resize(&mut self, area: Rect);
}

/// Single-theme registry: only the spider lily exists.
pub struct ThemeRegistry {
    theme: Box<dyn Theme>,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self {
            theme: Box::new(SpiderLily::new()),
        }
    }

    pub fn current(&self) -> &dyn Theme {
        self.theme.as_ref()
    }

    pub fn current_mut(&mut self) -> &mut dyn Theme {
        self.theme.as_mut()
    }

    pub fn current_name(&self) -> &'static str {
        self.theme.name()
    }

    pub fn resize_all(&mut self, area: Rect) {
        self.theme.resize(area);
    }
}

// ============================================================================
// Braille canvas
// ============================================================================
// One terminal cell is 2 braille dots wide and 4 dots tall.  Drawing directly
// in dot space keeps circles physically round and lets many strokes share the
// same cell.

const BRAILLE_BASE: u32 = 0x2800;

const BRAILLE_BITS: [[u8; 4]; 2] = [
    [0x01, 0x08, 0x10, 0x40], // left column: dots 1, 3, 5, 7
    [0x02, 0x04, 0x20, 0x80], // right column: dots 2, 4, 6, 8
];

#[inline]
fn plot_dot(buf: &mut Buffer, area: Rect, dx: i32, dy: i32, color: (u8, u8, u8)) {
    plot_dot_impl(buf, area, dx, dy, color, false);
}

/// Like [`plot_dot`], but always replaces the colour of the cell, even with a
/// darker one. Used for the flower heart, which must stay dark.
#[inline]
fn plot_dot_force(buf: &mut Buffer, area: Rect, dx: i32, dy: i32, color: (u8, u8, u8)) {
    plot_dot_impl(buf, area, dx, dy, color, true);
}

fn plot_dot_impl(
    buf: &mut Buffer,
    area: Rect,
    dx: i32,
    dy: i32,
    color: (u8, u8, u8),
    force: bool,
) {
    if dx < 0 || dy < 0 {
        return;
    }
    let cx = dx >> 1;
    let cy = dy >> 2;
    if cx >= area.width as i32 || cy >= area.height as i32 {
        return;
    }

    let bit = BRAILLE_BITS[(dx & 1) as usize][(dy & 3) as usize];
    let cell = &mut buf[(cx as u16, cy as u16)];

    // Keep the brightest stroke when two strokes share a cell: a dark petal
    // base must never punch a hole through a glowing tip.
    if !force {
        if let Color::Rgb(r, g, b) = cell.fg {
            let old = r as u16 + g as u16 + b as u16;
            let new = color.0 as u16 + color.1 as u16 + color.2 as u16;
            if old > new {
                return;
            }
        }
    }

    let mut pattern = cell
        .symbol()
        .chars()
        .next()
        .and_then(|ch| {
            let value = ch as u32;
            if (BRAILLE_BASE..=BRAILLE_BASE + 0xFF).contains(&value) {
                Some((value - BRAILLE_BASE) as u8)
            } else {
                None
            }
        })
        .unwrap_or(0);

    pattern |= bit;
    cell.set_char(char::from_u32(BRAILLE_BASE + pattern as u32).unwrap_or(' '));
    cell.set_fg(Color::Rgb(color.0, color.1, color.2));
}

/// Paint a round dot of `radius` dots at dot-space position (dx, dy).
#[inline]
fn stamp_disc(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
) {
    stamp_disc_impl(buf, area, dx, dy, radius, color, false);
}

/// A disc that is allowed to darken the dots it covers.
#[inline]
fn stamp_disc_force(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
) {
    stamp_disc_impl(buf, area, dx, dy, radius, color, true);
}

fn stamp_disc_impl(
    buf: &mut Buffer,
    area: Rect,
    dx: f32,
    dy: f32,
    radius: f32,
    color: (u8, u8, u8),
    force: bool,
) {
    let r = radius.max(0.5);
    let ri = r.ceil() as i32;
    let cx = dx.round() as i32;
    let cy = dy.round() as i32;
    let r2 = r * r;

    for oy in -ri..=ri {
        for ox in -ri..=ri {
            if (ox * ox + oy * oy) as f32 <= r2 {
                if force {
                    plot_dot_force(buf, area, cx + ox, cy + oy, color);
                } else {
                    plot_dot(buf, area, cx + ox, cy + oy, color);
                }
            }
        }
    }
}

/// Draw a tapered line in dot space.
#[allow(clippy::too_many_arguments)]
fn draw_dot_line(
    buf: &mut Buffer,
    area: Rect,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r0: f32,
    r1: f32,
    c0: (u8, u8, u8),
    c1: (u8, u8, u8),
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let distance = (dx * dx + dy * dy).sqrt();
    let steps = distance.ceil().max(1.0) as usize;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let radius = r0 + (r1 - r0) * t;
        let color = lerp_color(c0, c1, t);
        stamp_disc(buf, area, x0 + dx * t, y0 + dy * t, radius, color);
    }
}

// ============================================================================
// Small math / color helpers
// ============================================================================

const TAU: f32 = std::f32::consts::TAU;

#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

#[inline]
fn mul_color(c: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    (
        (c.0 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * factor).clamp(0.0, 255.0) as u8,
    )
}

#[inline]
fn tint(c: (u8, u8, u8), red: f32, green_blue: f32) -> (u8, u8, u8) {
    (
        (c.0 as f32 * red).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * green_blue).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * green_blue).clamp(0.0, 255.0) as u8,
    )
}

#[inline]
fn hash(mut x: u32) -> u32 {
    x = x.wrapping_mul(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x
}

// ============================================================================
// Palette — restrained: deep blood red, a little gold, almost-black green
// ============================================================================

const C_PETAL_DEEP: (u8, u8, u8) = (0x5E, 0x04, 0x0A);
const C_PETAL_MID: (u8, u8, u8) = (0xC0, 0x10, 0x1C);
const C_PETAL_TIP: (u8, u8, u8) = (0xFF, 0x36, 0x3A);
const C_PETAL_HOT: (u8, u8, u8) = (0xFF, 0x8A, 0x70);
const C_STAMEN: (u8, u8, u8) = (0x8A, 0x5C, 0x24);
const C_ANTHER: (u8, u8, u8) = (0xFF, 0xDC, 0x84);
const C_STEM_DARK: (u8, u8, u8) = (0x12, 0x2A, 0x16);
const C_STEM_LIGHT: (u8, u8, u8) = (0x30, 0x58, 0x32);
const C_MOSS: (u8, u8, u8) = (0x0E, 0x1E, 0x10);
const C_MOSS_LIGHT: (u8, u8, u8) = (0x1C, 0x30, 0x1C);
const C_GLOW: (u8, u8, u8) = (0x46, 0x04, 0x0A);
const C_FALLING: (u8, u8, u8) = (0x94, 0x0C, 0x12);

// ============================================================================
// Spider lily
// ============================================================================

#[derive(Copy, Clone)]
struct Petal {
    angle: f32,
    length: f32,  // cells
    width: f32,   // dots (radius)
    curl: f32,    // radians the petal bends along its length
    recurve: f32, // how much the tip lifts back up
    phase: f32,
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

struct Flower {
    base_x: f32,
    stem_height: f32,
    lean: f32,
    curve_amp: f32,
    curve_freq: f32,
    curve_phase: f32,
    sway_phase: f32,
    sway_speed: f32,
    sway_amp: f32,
    head_rotation: f32,
    growth: f32,
    grow_time: f32,
    age: f32,
    bloom_time: f32,
    hold_time: f32,
    wilt_time: f32,
    bloom: f32,
    dead: bool,
    petals: [Petal; 6],
    stamens: [Stamen; 5],
    hue: f32,
    depth: f32,
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
}

#[derive(Copy, Clone)]
struct HeadView {
    hx: f32,
    hy: f32,
    rot: f32,
    bloom: f32,
    depth: f32,
    red: f32,
    gb: f32,
    breath: f32,
}

struct SpiderLily {
    flowers: Vec<Flower>,
    falling: Vec<FallingPetal>,
    time: f32,
    width: u16,
    height: u16,
    target: usize,
    next_roll: f32,
}

impl SpiderLily {
    fn new() -> Self {
        Self {
            flowers: Vec::new(),
            falling: Vec::new(),
            time: 0.0,
            width: 80,
            height: 24,
            target: 0,
            next_roll: 0.0,
        }
    }

    fn ground_y(&self) -> f32 {
        (self.height as f32 - 3.0).max(4.0)
    }

    fn pick_x(&self, rng: &mut ChaCha8Rng) -> f32 {
        // Keep enough room for the widest head: petals, lean and sway all
        // reach past the stem, so a narrow margin would clip them at the edges.
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

        // Shorter plants feel closer and are drawn a touch brighter.
        let depth = (0.62 + 0.38 * (1.0 - stem_height / ground_y)).clamp(0.60, 1.0);

        // Keep the flower in proportion with the window, and leave the top of
        // the screen free so petals are never chopped off.
        let max_len = (head_y * 0.95)
            .min(self.width as f32 * 0.10)
            .clamp(4.0, 13.0);

        // On narrow terminals a petal should stay a fine line, not a bar.
        let width_scale = (self.width as f32 / 100.0).clamp(0.72, 1.10);
        let curl_sign = if rng.gen::<bool>() { 1.0 } else { -1.0 };
        let hero: usize = rng.gen_range(0..6);

        let mut petals = [Petal {
            angle: 0.0,
            length: 0.0,
            width: 0.0,
            curl: 0.0,
            recurve: 0.0,
            phase: 0.0,
        }; 6];

        for (i, petal) in petals.iter_mut().enumerate() {
            let base = i as f32 * TAU / 6.0 + rng.gen_range(-0.12..0.12);
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
                phase: rng.gen_range(0.0..TAU),
            };
        }

        let stamen_max = (head_y * 1.30).max(max_len * 1.1);
        let mut stamens = [Stamen {
            angle: 0.0,
            length: 0.0,
            curl: 0.0,
            recurve: 0.0,
            phase: 0.0,
            speed: 0.0,
        }; 5];

        for (i, stamen) in stamens.iter_mut().enumerate() {
            *stamen = Stamen {
                // A crown of filaments fanning upward, like the real flower.
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

        let grow_time: f32 = rng.gen_range(10.0..18.0);
        let bloom_time: f32 = rng.gen_range(35.0..70.0);
        let hold_time: f32 = rng.gen_range(100.0..240.0);
        let wilt_time: f32 = rng.gen_range(40.0..75.0);

        // Start mid-life so the garden is already alive on the first frame.
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
            sway_amp: rng.gen_range(1.2..3.2),
            head_rotation: rng.gen_range(-0.45..0.45),
            growth,
            grow_time,
            age,
            bloom_time,
            hold_time,
            wilt_time,
            bloom: 0.0,
            dead: false,
            petals,
            stamens,
            hue: rng.gen_range(-1.0..1.0),
            depth,
            fall_accum: rng.gen_range(0.0..1.0),
        }
    }

    fn ensure_flowers(&mut self, rng: &mut ChaCha8Rng) {
        // Re-roll the desired density only now and then, so the garden keeps a
        // stable mood instead of flickering between plant counts.
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
}

// ============================================================================
// Geometry
// ============================================================================

/// Position along a stem, `s` from 0 (soil) to 1 (flower head).
fn stem_point(flower: &Flower, time: f32, ground_y: f32, s: f32) -> (f32, f32) {
    let height = flower.stem_height * flower.growth;

    let sway1 = (time * flower.sway_speed + flower.sway_phase).sin();
    let sway2 = (time * flower.sway_speed * 0.47 + flower.sway_phase * 1.7 + 1.9).sin();
    let bend = (flower.lean + sway1 * flower.sway_amp + sway2 * flower.sway_amp * 0.35)
        * s.powf(1.7);

    // A gentle S-bend in the middle of the stem, zero at both ends.
    let wiggle = (s * std::f32::consts::PI * flower.curve_freq + flower.curve_phase).sin()
        * flower.curve_amp
        * s
        * (1.0 - s)
        * 4.0;

    let x = flower.base_x + bend + wiggle;
    let y = ground_y - height * s;
    (x, y)
}

/// Point on a petal spine at `t` in 0..=1.
fn petal_point(petal: &Petal, view: &HeadView, t: f32, len: f32, time: f32) -> (f32, f32) {
    let base_angle = petal.angle + view.rot;
    // The bend accelerates toward the tip, so each petal ends in a real hook.
    let bend = petal.curl * t * t * 1.20;
    let wave = (t * 6.5 + petal.phase + time * 0.35).sin() * 0.045 * t;
    let angle = base_angle + bend + wave;

    // Leave a small open heart instead of knotting every petal at one dot.
    let inner = (len * 0.22).min(2.2);
    let radius = inner + (len - inner) * t;
    let x = view.hx + angle.cos() * radius;
    let mut y = view.hy + angle.sin() * radius * 0.52;

    // Recurve: tips lift back toward the sky like real spider-lily petals.
    y -= petal.recurve * t.powi(3) * len * 0.30 * (0.35 + 0.65 * view.bloom);
    (x, y)
}

fn new_falling(x: f32, y: f32, depth: f32, rng: &mut ChaCha8Rng) -> FallingPetal {
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

        if changed || self.flowers.is_empty() {
            self.flowers.clear();
            self.falling.clear();
            self.time = 0.0;
            self.target = 0;
            self.next_roll = 0.0;
        }
    }

    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        // A long pause must not teleport the garden.
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.ensure_flowers(rng);

        let ground_y = self.ground_y();
        let time = self.time;

        // Split the borrows so flowers can spawn falling petals directly.
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

            // Falling petals: a slow trickle in full bloom, a gentle rain while
            // wilting, then a small farewell burst.
            let wilting = t > flower.bloom_time + flower.hold_time;
            let rate = if wilting {
                1.35
            } else if bloom > 0.97 {
                0.05
            } else {
                0.20
            };
            flower.fall_accum += rate * dt;

            if flower.fall_accum >= 1.0 && flower.age > flower.grow_time * 0.7 {
                flower.fall_accum = 0.0;
                let (hx, hy) = stem_point(flower, time, ground_y, 1.0);
                let angle = rng.gen_range(0.0..TAU);
                let radius = rng.gen_range(0.4..2.6);
                falling.push(new_falling(
                    hx + angle.cos() * radius,
                    hy + angle.sin() * radius * 0.5,
                    flower.depth,
                    rng,
                ));
            }

            if t > flower.bloom_time + flower.hold_time + flower.wilt_time {
                flower.dead = true;
                let (hx, hy) = stem_point(flower, time, ground_y, 1.0);
                for _ in 0..rng.gen_range(3..7) {
                    let angle = rng.gen_range(0.0..TAU);
                    let radius = rng.gen_range(0.0..2.2);
                    falling.push(new_falling(
                        hx + angle.cos() * radius,
                        hy + angle.sin() * radius * 0.5,
                        flower.depth,
                        rng,
                    ));
                }
            }
        }

        flowers.retain(|flower| !flower.dead);

        falling.retain_mut(|petal| {
            petal.life += dt;
            let flutter = (time * 1.25 + petal.phase).sin();
            petal.x += (petal.vx + flutter * 0.30) * dt;
            petal.y += petal.vy * dt;
            petal.vy = (petal.vy + 0.010 * dt).min(1.0);
            petal.angle += petal.spin * dt;
            petal.life < petal.max_life && petal.y < ground_y + 2.0
        });
    }

    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        if area.width < 24 || area.height < 9 {
            return;
        }

        // Sit just above the status bar so the stems are actually visible.
        let ground_y = (area.height as f32 - 3.0).max(4.0);
        let breath = 0.93 + 0.07 * (self.time * 0.22).sin();

        self.draw_ground(buf, area, ground_y);

        for flower in &self.flowers {
            self.draw_stem(buf, area, flower, ground_y, breath);
            self.draw_head(buf, area, flower, ground_y, breath);
        }

        self.draw_falling(buf, area, breath);
    }
}

impl SpiderLily {
    fn draw_ground(&self, buf: &mut Buffer, area: Rect, ground_y: f32) {
        let ground_row = ground_y.round() as i32;
        let bottom_dot = ground_row * 4 + 3;
        let width = area.width as i32;

        for x in 0..width {
            let h = hash(x as u32);

            // A broken, living moss line rather than a solid rule.
            if h % 9 < 3 {
                let color = if h % 5 == 0 { C_MOSS_LIGHT } else { C_MOSS };
                let dx = x * 2 + (h & 1) as i32;
                plot_dot(buf, area, dx, bottom_dot, color);
            }

            // Sparse blades of grass, swaying almost imperceptibly.
            if h % 17 == 0 {
                let base_x = x as f32 * 2.0 + ((h >> 3) & 1) as f32;
                let sway = (self.time * 0.35 + x as f32 * 0.6).sin() * 0.9;
                let blade = 2.0 + (h % 5) as f32;
                let tip_x = base_x + sway;
                let tip_y = bottom_dot as f32 - blade;
                draw_dot_line(
                    buf,
                    area,
                    base_x,
                    bottom_dot as f32,
                    tip_x,
                    tip_y,
                    0.5,
                    0.35,
                    C_MOSS,
                    C_MOSS_LIGHT,
                );
            }
        }

        // Little root clusters where the stems meet the soil.
        for flower in &self.flowers {
            let base_x = flower.base_x * 2.0;
            for i in -1..=1 {
                let h = hash((flower.base_x as u32).wrapping_mul(31).wrapping_add(i as u32));
                if h % 3 != 0 {
                    continue;
                }
                let dx = base_x + i as f32 * 1.1;
                let dy = bottom_dot as f32 - (h % 3) as f32;
                stamp_disc(buf, area, dx, dy, 0.5, C_MOSS_LIGHT);
            }
        }
    }

    fn draw_stem(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        ground_y: f32,
        breath: f32,
    ) {
        if flower.growth <= 0.02 {
            return;
        }

        let steps = 80;
        let fade = (0.30 + 0.70 * flower.growth) * flower.depth * breath;

        let (mut px, mut py) = stem_point(flower, self.time, ground_y, 0.0);
        let mut pr = 0.95;
        let mut pc = mul_color(C_STEM_DARK, fade);

        for i in 1..=steps {
            let s = i as f32 / steps as f32;
            let (x, y) = stem_point(flower, self.time, ground_y, s);
            // A single-dot line, with just a small flare where it meets the soil.
            let radius = 0.95 + (0.35 - 0.95) * s;
            let color = mul_color(
                lerp_color(C_STEM_DARK, C_STEM_LIGHT, s * 0.85),
                fade,
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
    }

    fn draw_head(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        ground_y: f32,
        breath: f32,
    ) {
        let (hx, hy) = stem_point(flower, self.time, ground_y, 1.0);
        let (tx, ty) = stem_point(flower, self.time, ground_y, 0.96);

        // The head leans with the stem, then drifts a hair on its own.
        let tilt = ((hx - tx).atan2((ty - hy).max(0.0001)) * 0.65).clamp(-0.55, 0.55);
        let rot = tilt
            + flower.head_rotation
            + (self.time * 0.045 + flower.sway_phase * 1.3).sin() * 0.035;

        if flower.bloom < 0.015 {
            self.draw_bud(buf, area, hx, hy, flower, breath);
            return;
        }

        let view = HeadView {
            hx,
            hy,
            rot,
            bloom: flower.bloom,
            depth: flower.depth,
            red: 1.0 + flower.hue * 0.10,
            gb: (1.0 - flower.hue * 0.25).clamp(0.72, 1.20),
            breath,
        };

        self.draw_aura(buf, area, &view, flower.sway_phase);

        // Stamens sit behind the petals; only their gold anthers peek through.
        self.draw_stamens(buf, area, flower, &view);

        for petal in flower.petals.iter() {
            self.draw_petal(buf, area, petal, &view, 1.0);
        }

        // Dark heart with a glint of gold.
        let core = mul_color(C_PETAL_DEEP, view.depth * view.bloom * view.breath * 0.85);
        stamp_disc_force(buf, area, hx * 2.0, hy * 4.0, 0.85 + view.bloom * 0.25, core);
        stamp_disc(
            buf,
            area,
            hx * 2.0,
            hy * 4.0,
            0.5,
            mul_color(C_ANTHER, view.depth * view.bloom * 0.55),
        );
    }

    fn draw_bud(
        &self,
        buf: &mut Buffer,
        area: Rect,
        hx: f32,
        hy: f32,
        flower: &Flower,
        breath: f32,
    ) {
        let depth = flower.depth * breath;
        let dark = mul_color(C_PETAL_DEEP, depth * 0.85);
        let warm = mul_color(C_PETAL_MID, depth * 0.55);

        for i in 0..4 {
            let yy = hy * 4.0 + (i as f32 - 1.5) * 1.15;
            let radius = if i == 0 || i == 3 { 0.5 } else { 0.7 };
            let color = if i == 3 { warm } else { dark };
            stamp_disc(buf, area, hx * 2.0, yy, radius, color);
        }

        // A tiny sepal keeps the bud attached to its stem.
        stamp_disc(
            buf,
            area,
            hx * 2.0,
            hy * 4.0 + 2.4,
            0.5,
            mul_color(C_STEM_LIGHT, depth),
        );
    }

    fn draw_aura(&self, buf: &mut Buffer, area: Rect, view: &HeadView, sway_phase: f32) {
        let pulse = 0.85 + 0.15 * (self.time * 0.30 + sway_phase).sin();
        let intensity = view.bloom * view.depth * view.breath * pulse * 0.32;

        // Golden-angle spiral: even, organic, never flickering.
        for i in 0..6 {
            let angle = i as f32 * 2.399_963 + sway_phase;
            let radius = 3.5 + ((i * 7) % 9) as f32 * 0.55;
            let falloff = (1.0 - radius / 10.0).max(0.0);
            if falloff <= 0.0 {
                continue;
            }

            let color = mul_color(C_GLOW, intensity * falloff);
            let x = view.hx + angle.cos() * radius;
            let y = view.hy + angle.sin() * radius * 0.5;
            stamp_disc(buf, area, x * 2.0, y * 4.0, 0.5, color);
        }
    }

    fn draw_petal(
        &self,
        buf: &mut Buffer,
        area: Rect,
        petal: &Petal,
        view: &HeadView,
        layer_dim: f32,
    ) {
        let len = petal.length * (0.10 + 0.90 * view.bloom);
        if len < 0.8 {
            return;
        }

        let steps = (len * 5.0).clamp(8.0, 90.0) as usize;
        let mut previous: Option<(f32, f32, f32, (u8, u8, u8))> = None;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let (x, y) = petal_point(petal, view, t, len, self.time);

            // Thin at the base, full in the middle, pointed at the tip.
            let profile = (std::f32::consts::PI * t).sin().powf(0.80);
            let width = (petal.width * (0.26 + 0.74 * profile)).max(0.55);

            let base_color = if t < 0.38 {
                lerp_color(C_PETAL_DEEP, C_PETAL_MID, t / 0.38)
            } else {
                lerp_color(C_PETAL_MID, C_PETAL_TIP, (t - 0.38) / 0.62)
            };
            let color = tint(
                mul_color(base_color, view.depth * layer_dim * view.breath),
                view.red,
                view.gb,
            );

            if let Some((px, py, pw, pc)) = previous {
                draw_dot_line(
                    buf,
                    area,
                    px * 2.0,
                    py * 4.0,
                    x * 2.0,
                    y * 4.0,
                    pw,
                    width,
                    pc,
                    color,
                );
            } else {
                stamp_disc(buf, area, x * 2.0, y * 4.0, width, color);
            }

            previous = Some((x, y, width, color));
        }

        // A single warm spark at the very tip.
        if view.bloom > 0.55 {
            let (x, y) = petal_point(petal, view, 1.0, len, self.time);
            let heat = ((view.bloom - 0.55) / 0.45).clamp(0.0, 1.0);
            let color = mul_color(
                C_PETAL_HOT,
                view.depth * layer_dim * view.breath * heat,
            );
            stamp_disc(buf, area, x * 2.0, y * 4.0, 0.55, color);
        }
    }

    fn draw_stamens(
        &self,
        buf: &mut Buffer,
        area: Rect,
        flower: &Flower,
        view: &HeadView,
    ) {
        for stamen in &flower.stamens {
            let len = stamen.length * (0.25 + 0.75 * view.bloom);
            if len < 0.8 {
                continue;
            }

            let sway = (self.time * stamen.speed + stamen.phase).sin() * 0.05;
            let steps = (len * 4.0).clamp(6.0, 70.0) as usize;
            let mut previous: Option<(f32, f32)> = None;

            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let angle = stamen.angle + view.rot + stamen.curl * (t * 1.15).sin()
                    + sway * t;
                let radius = len * t;
                let x = view.hx + angle.cos() * radius;
                let y = view.hy + angle.sin() * radius * 0.52
                    - stamen.recurve * t.powi(3) * len * 0.16;

                if let Some((px, py)) = previous {
                    let dim = mul_color(C_STAMEN, view.depth * view.breath * 0.45);
                    let bright = mul_color(C_STAMEN, view.depth * view.breath * 0.85);
                    let color = lerp_color(dim, bright, t);
                    draw_dot_line(
                        buf,
                        area,
                        px * 2.0,
                        py * 4.0,
                        x * 2.0,
                        y * 4.0,
                        0.30,
                        0.30,
                        color,
                        color,
                    );
                }
                previous = Some((x, y));
            }

            // Anther: a slow golden pulse with the faintest halo.
            let angle =
                stamen.angle + view.rot + stamen.curl * 1.15f32.sin() + sway;
            let x = view.hx + angle.cos() * len;
            let y = view.hy + angle.sin() * len * 0.52 - stamen.recurve * len * 0.16;
            let pulse = 0.5 + 0.5 * (self.time * 0.45 + stamen.phase * 2.0).sin();

            stamp_disc(
                buf,
                area,
                x * 2.0,
                y * 4.0,
                1.4,
                mul_color(C_ANTHER, view.depth * view.breath * 0.08),
            );
            stamp_disc(
                buf,
                area,
                x * 2.0,
                y * 4.0,
                0.45 + pulse * 0.30,
                mul_color(
                    C_ANTHER,
                    view.depth * view.breath * (0.55 + 0.35 * pulse),
                ),
            );
        }
    }

    fn draw_falling(&self, buf: &mut Buffer, area: Rect, breath: f32) {
        for petal in &self.falling {
            let life = (petal.life / petal.max_life).clamp(0.0, 1.0);
            let alpha = (1.0 - life).powf(0.7);
            let color = mul_color(C_FALLING, petal.depth * breath * alpha * 0.9);

            let len = 1.2 + petal.size * 1.5;
            let dx = petal.angle.cos();
            let dy = petal.angle.sin();

            for k in 0..3 {
                let offset = (k as f32 / 2.0 - 0.5) * len;
                stamp_disc(
                    buf,
                    area,
                    petal.x * 2.0 + dx * offset,
                    petal.y * 4.0 + dy * offset,
                    0.5,
                    color,
                );
            }
        }
    }
}
