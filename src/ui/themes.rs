use rand::{Rng, SeedableRng, seq::SliceRandom};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
};
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;

/// Trait for all animated background themes
pub trait Theme: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng);
    fn draw(&mut self, buf: &mut Buffer, area: Rect);
    fn resize(&mut self, area: Rect);
}

/// Theme registry - all available themes
pub struct ThemeRegistry {
    themes: Vec<Box<dyn Theme>>,
    current: usize,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            themes: Vec::new(),
            current: 0,
        };
        registry.register_all();
        registry
    }

    fn register_all(&mut self) {
        self.themes.push(Box::new(SpiderLily::new()));
        self.themes.push(Box::new(SakuraNight::new()));
        self.themes.push(Box::new(AbyssalBloom::new()));
        self.themes.push(Box::new(FireflyDusk::new()));
        self.themes.push(Box::new(AuroraVoid::new()));
        self.themes.push(Box::new(KoiDreams::new()));
        self.themes.push(Box::new(BambooWhisper::new()));
        self.themes.push(Box::new(NebulaHeart::new()));
        self.themes.push(Box::new(RainGlass::new()));
        self.themes.push(Box::new(WillowTears::new()));
    }

    pub fn current(&self) -> &dyn Theme {
        self.themes[self.current].as_ref()
    }

    pub fn current_mut(&mut self) -> &mut dyn Theme {
        self.themes[self.current].as_mut()
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        for (i, theme) in self.themes.iter().enumerate() {
            if theme.name().eq_ignore_ascii_case(name) {
                self.current = i;
                return true;
            }
        }
        false
    }

    pub fn next_theme(&mut self) {
        self.current = (self.current + 1) % self.themes.len();
    }

    pub fn prev_theme(&mut self) {
        self.current = if self.current == 0 { self.themes.len() - 1 } else { self.current - 1 };
    }

    pub fn list_themes(&self) -> Vec<(&'static str, &'static str)> {
        self.themes.iter().map(|t| (t.name(), t.description())).collect()
    }

    pub fn current_name(&self) -> &'static str {
        self.themes[self.current].name()
    }

    pub fn resize_all(&mut self, area: Rect) {
        for theme in &mut self.themes {
            theme.resize(area);
        }
    }
}

// ============================================================================
// HIGH-QUALITY ASCII ART HELPERS
// ============================================================================

thread_local! {
    static DRAW_RNG: RefCell<ChaCha8Rng> = RefCell::new(ChaCha8Rng::from_rng(&mut rand::thread_rng()).unwrap());
}

fn draw_rng() -> impl FnMut() -> f32 {
    || DRAW_RNG.with(|r| r.borrow_mut().gen::<f32>())
}

fn new_rng() -> ChaCha8Rng {
    ChaCha8Rng::from_rng(&mut rand::thread_rng()).unwrap()
}

// Color interpolation helper
fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        (c1.0 as f32 + (c2.0 as f32 - c1.0 as f32) * t) as u8,
        (c1.1 as f32 + (c2.1 as f32 - c1.1 as f32) * t) as u8,
        (c1.2 as f32 + (c2.2 as f32 - c1.2 as f32) * t) as u8,
    )
}

// Draw a character with color at position
fn draw_char(buf: &mut Buffer, x: i32, y: i32, area: Rect, ch: char, color: (u8, u8, u8)) {
    if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
        buf[(x as u16, y as u16)].set_char(ch);
        buf[(x as u16, y as u16)].set_fg(Color::Rgb(color.0, color.1, color.2));
    }
}

// ============================================================================
// THEME 1: SPIDER LILY (彼岸花) - Tokyo Ghoul style
// ============================================================================

struct SpiderFlower {
    x: f32, y: f32,
    stage: f32,           // 0.0=bud, 1.0=full bloom, >1=wilting
    stage_speed: f32,
    rotation: f32,
    size: f32,
    color_phase: f32,
    petal_fall_timer: f32,
}

struct FallingPetal {
    x: f32, y: f32,
    vx: f32, vy: f32,
    rotation: f32, rot_speed: f32,
    frame: usize,
    color: (u8, u8, u8),
    life: f32,
}

// Spider lily petal frames (6 petals radiating)
const SPIDER_PETAL_FRAMES: &[&[&str]] = &[
    // Bud stage
    &["  ▄  ", " ▄█▄ ", "▄███▄", " ▀▀▀ "],
    // Opening
    &["  ▄  ", " ▄█▄ ", "▄███▄", "█ █ █", " ▀▀▀ "],
    // Half bloom
    &["  ▄  ", "╱█╲", "╱███╲", "█ █ █", "╲ █ ╱", " ╲ ╱ "],
    // Full bloom - 6 petals curved
    &["   ▄   ", "  ╱█╲  ", " ╱███╲ ", "╱█████╲", "█ ▄ ▄ █", "╲ ▀ ▀ ╱", " ╲   ╱ ", "  ╲ ╱  "],
    // Wilting - petals drooping
    &["   ▄   ", "  ╱█╲  ", " ╱███╲ ", "███████", "█ ▄ ▄ █", "╲ ▀ ▀ ╱", "  ╲ ╱  ", "   ╱╲   ", "  ╱  ╲  "],
];

const SPIDER_PETAL_CHARS: &[char] = &['❀', '✿', '❁', '♢', '◆', '◈', '◇', '◆'];

struct SpiderLily {
    flowers: Vec<SpiderFlower>,
    petals: Vec<FallingPetal>,
    time: f32,
    width: u16,
    height: u16,
}

impl SpiderLily {
    fn new() -> Self {
        Self { flowers: Vec::new(), petals: Vec::new(), time: 0.0, width: 0, height: 0 }
    }

    fn spawn_flowers(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.flowers.clear();
        let count = ((width * height) as f32 / 3500.0).clamp(3.0, 10.0) as usize;
        for _ in 0..count {
            self.flowers.push(SpiderFlower {
                x: rng.gen_range(4.0..(width - 4) as f32),
                y: rng.gen_range(6.0..(height - 6) as f32),
                stage: rng.gen_range(0.0..1.0),
                stage_speed: rng.gen_range(0.005..0.015),
                rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                size: rng.gen_range(0.7..1.3),
                color_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                petal_fall_timer: 0.0,
            });
        }
    }
}

impl Theme for SpiderLily {
    fn name(&self) -> &'static str { "spider-lily" }
    fn description(&self) -> &'static str { "彼岸花 - Red spider lilies blooming/wilting, petals drifting like blood (Tokyo Ghoul)" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn_flowers(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for flower in &mut self.flowers {
            flower.stage += flower.stage_speed * dt * 60.0;
            flower.rotation += 0.015 * dt * 60.0;
            flower.petal_fall_timer += dt;

            // Spawn falling petals during wilting phase
            if flower.stage > 1.3 && flower.stage < 1.35 && flower.petal_fall_timer > 0.5 {
                flower.petal_fall_timer = 0.0;
                for _ in 0..rng.gen_range(1..4) {
                    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                    let speed = rng.gen_range(0.2..0.8);
                    self.petals.push(FallingPetal {
                        x: flower.x, y: flower.y,
                        vx: angle.cos() * speed * 0.4, vy: angle.sin() * speed * 0.4 + 0.15,
                        rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                        rot_speed: rng.gen_range(-0.03..0.03),
                        frame: rng.gen_range(0..SPIDER_PETAL_CHARS.len()),
                        color: *[(0x8B,0,0), (0xB2,0x22,0x22), (0xDC,0x14,0x3C), (0xFF,0x24,0), (0x8B,0,0)].choose(rng).unwrap(),
                        life: 0.0,
                    });
                }
            }

            // Reset cycle
            if flower.stage > 2.5 {
                flower.stage = 0.0;
                flower.stage_speed = rng.gen_range(0.005..0.015);
                flower.x = rng.gen_range(4.0..(self.width - 4) as f32);
                flower.y = rng.gen_range(6.0..(self.height - 6) as f32);
                flower.petal_fall_timer = 0.0;
            }
        }

        // Update falling petals
        self.petals.retain_mut(|p| {
            p.x += p.vx * dt * 60.0;
            p.y += p.vy * dt * 60.0;
            p.rotation += p.rot_speed * dt * 60.0;
            p.vy += 0.003 * dt * 60.0; // gentle gravity
            p.life += dt;
            p.life < 20.0 && p.y < self.height as f32 + 3.0
        });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Draw flowers
        for flower in &self.flowers {
            self.draw_flower(buf, flower, area);
        }
        // Draw falling petals
        for petal in &self.petals {
            let x = petal.x as i32;
            let y = petal.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let alpha = 1.0 - (petal.life / 20.0).min(1.0);
                let (r, g, b) = petal.color;
                draw_char(buf, x, y, area, SPIDER_PETAL_CHARS[petal.frame % SPIDER_PETAL_CHARS.len()],
                    ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
            }
        }
    }
}

impl SpiderLily {
    fn draw_flower(&self, buf: &mut Buffer, flower: &SpiderFlower, area: Rect) {
        let cx = flower.x as i32;
        let cy = flower.y as i32;
        if cx < 0 || cx >= area.width as i32 || cy < 0 || cy >= area.height as i32 { return; }

        let bloom = flower.stage.clamp(0.0, 1.0);
        let wilt = (flower.stage - 1.0).clamp(0.0, 1.0);
        let size_idx = (flower.size * bloom * (1.0 - wilt * 0.4) * 3.0).floor() as usize;
        let size_idx = size_idx.min(SPIDER_PETAL_FRAMES.len() - 1);

        // Color shifting
        let hue_shift = (self.time * 0.08 + flower.color_phase).sin() * 0.12;
        let base_r = (0x8B as f32 + hue_shift * 60.0) as u8;
        let base_g = (0x00 as f32 + hue_shift * 40.0) as u8;

        let frame = SPIDER_PETAL_FRAMES[size_idx];
        let frame_h = frame.len() as i32;
        let frame_w = frame[0].len() as i32;
        let start_y = cy - frame_h / 2;
        let start_x = cx - frame_w / 2;

        for (fy, line) in frame.iter().enumerate() {
            for (fx, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    let px = start_x + fx as i32;
                    let py = start_y + fy as i32;
                    if px >= 0 && px < area.width as i32 && py >= 0 && py < area.height as i32 {
                        let alpha = bloom * (1.0 - wilt) * (0.7 + 0.3 * (fy as f32 / frame_h as f32));
                        draw_char(buf, px, py, area, ch,
                            ((base_r as f32 * alpha) as u8, (base_g as f32 * alpha) as u8, 0));
                    }
                }
            }
        }

        // Center stamen
        if bloom > 0.4 {
            draw_char(buf, cx, cy, area, '✶', (0xFF, 0xD7, 0x00));
        }

        // Long curved stamens (6 radiating)
        if bloom > 0.5 {
            for i in 0..6 {
                let angle = flower.rotation + i as f32 * std::f32::consts::TAU / 6.0 + 0.5;
                for step in 1..=(6 + (flower.size * 4.0) as i32) {
                    let px = cx + (angle.cos() * step as f32 * 1.2) as i32;
                    let py = cy + (angle.sin() * step as f32 * 0.5) as i32;
                    if px >= 0 && px < area.width as i32 && py >= 0 && py < area.height as i32 {
                        let alpha = 0.6 * (1.0 - step as f32 / 20.0);
                        draw_char(buf, px, py, area, '│', ((0xFF as f32 * alpha) as u8, (0xD7 as f32 * alpha) as u8, 0));
                    }
                }
            }
        }
    }
}

// ============================================================================
// THEME 2: SAKURA NIGHT (夜桜) - Cherry blossoms at night
// ============================================================================

struct SakuraTree {
    x: f32, y: f32,
    trunk_h: usize,
    branches: Vec<Branch>,
    sway_phase: f32, sway_speed: f32,
    blossom_phase: f32,
}

struct Branch {
    start_y: f32,
    angle: f32,
    length: f32,
    blossoms: Vec<Blossom>,
}

struct Blossom {
    offset: f32,      // position along branch
    size: f32,
    phase: f32,       // bloom phase
    color_idx: usize,
}

struct FallingPetalSakura {
    x: f32, y: f32,
    vx: f32, vy: f32,
    rotation: f32, rot_speed: f32,
    frame: usize,
    color: (u8, u8, u8),
    life: f32,
}

const SAKURA_BLOSSOM_FRAMES: &[&str] = &["·", "∘", "◦", "●", "✿", "❀", "✿", "❀"];
const SAKURA_COLORS: &[(u8, u8, u8)] = &[
    (0xFF, 0xB7, 0xC5), (0xFF, 0xA0, 0xB4), (0xFF, 0x85, 0xA3),
    (0xFF, 0x69, 0xB4), (0xE8, 0x5D, 0x9E), (0xFF, 0xE4, 0xE1),
];

const MOON_FRAMES: &[&[&str]] = &[
    &["  ▄▄  ", " ████ ", "██████", " ▀▀▀▀ "],
    &[" ▄▄▄▄ ", "██████", "██████", " ▀▀▀▀ "],
];

struct SakuraNight {
    trees: Vec<SakuraTree>,
    petals: Vec<FallingPetalSakura>,
    moon_x: f32, moon_y: f32,
    moon_phase: usize,
    time: f32,
    width: u16, height: u16,
}

impl SakuraNight {
    fn new() -> Self {
        Self { trees: Vec::new(), petals: Vec::new(), moon_x: 0.0, moon_y: 0.0, moon_phase: 0, time: 0.0, width: 0, height: 0 }
    }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.trees.clear(); self.petals.clear();
        let count = (width as f32 / 28.0).clamp(2.0, 7.0) as usize;
        let ground = height as f32 - 1.0;
        self.moon_x = rng.gen_range(width as f32 * 0.15..width as f32 * 0.85);
        self.moon_y = rng.gen_range(3.0..height as f32 * 0.35);
        for _ in 0..count {
            let h = rng.gen_range(8..14);
            let mut branches = Vec::new();
            for b in 0..rng.gen_range(4..7) {
                let by = ground - (h as f32 * 0.3) - b as f32 * 2.5;
                branches.push(Branch {
                    start_y: by,
                    angle: rng.gen_range(-0.5..0.5),
                    length: rng.gen_range(6.0..14.0),
                    blossoms: (0..rng.gen_range(8..18)).map(|_| Blossom {
                        offset: rng.gen_range(0.1..0.95),
                        size: rng.gen_range(0.5..1.2),
                        phase: rng.gen_range(0.0..std::f32::consts::TAU),
                        color_idx: rng.gen_range(0..SAKURA_COLORS.len()),
                    }).collect(),
                });
            }
            self.trees.push(SakuraTree {
                x: rng.gen_range(6.0..(width - 6) as f32),
                y: ground,
                trunk_h: h,
                branches,
                sway_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                sway_speed: rng.gen_range(0.0015..0.004),
                blossom_phase: rng.gen_range(0.0..std::f32::consts::TAU),
            });
        }
    }
}

impl Theme for SakuraNight {
    fn name(&self) -> &'static str { "sakura-night" }
    fn description(&self) -> &'static str { "夜桜 - Cherry blossoms at night, pink petals falling under moonlight" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        // Moon phase cycle
        if (self.time * 0.002).floor() as usize != self.moon_phase {
            self.moon_phase = (self.moon_phase + 1) % MOON_FRAMES.len();
        }

        for tree in &mut self.trees {
            tree.sway_phase += tree.sway_speed * dt * 60.0;
            tree.blossom_phase += 0.02 * dt * 60.0;

            // Spawn falling petals
            if rng.gen::<f32>() < 0.006 * dt * 60.0 {
                let bx = tree.x + (tree.sway_phase.sin() * tree.trunk_h as f32 * 0.4);
                for branch in &tree.branches {
                    for blossom in &branch.blossoms {
                        if rng.gen::<f32>() < 0.05 {
                            let px = bx + branch.angle.sin() * branch.length * blossom.offset * 0.5;
                            let py = branch.start_y - branch.angle.cos() * branch.length * blossom.offset;
                            self.petals.push(FallingPetalSakura {
                                x: px, y: py,
                                vx: rng.gen_range(-0.25..0.25) + (tree.sway_phase.sin() * 0.1),
                                vy: rng.gen_range(0.12..0.35),
                                rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                                rot_speed: rng.gen_range(-0.02..0.02),
                                frame: rng.gen_range(0..SAKURA_BLOSSOM_FRAMES.len()),
                                color: SAKURA_COLORS[blossom.color_idx],
                                life: 0.0,
                            });
                        }
                    }
                }
            }
        }

        self.petals.retain_mut(|p| {
            p.x += p.vx * dt * 60.0;
            p.y += p.vy * dt * 60.0;
            p.rotation += p.rot_speed * dt * 60.0;
            p.vx += rng.gen_range(-0.008..0.008) * dt * 60.0;
            p.life += dt;
            p.life < 25.0 && p.y < self.height as f32 + 3.0
        });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Draw moon
        let moon_frame = MOON_FRAMES[self.moon_phase];
        let mx = self.moon_x as i32;
        let my = self.moon_y as i32;
        for (fy, line) in moon_frame.iter().enumerate() {
            for (fx, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    draw_char(buf, mx - 3 + fx as i32, my - 2 + fy as i32, area, ch, (0xFF, 0xF0, 0xA0));
                }
            }
        }

        // Draw trees
        for tree in &self.trees {
            let sway = (tree.sway_phase.sin() * 1.8) as i32;
            let base_x = tree.x as i32 + sway;
            let base_y = tree.y as i32;

            // Trunk with segments
            for i in 0..tree.trunk_h {
                let sy = base_y - i as i32;
                let sx = base_x + (sway * (i as i32) / tree.trunk_h as i32);
                if sx >= 0 && sx < area.width as i32 && sy >= 0 && sy < area.height as i32 {
                    draw_char(buf, sx, sy, area, '│', (0x3D, 0x2B, 0x1F));
                    if i % 3 == 0 {
                        draw_char(buf, sx, sy, area, '┼', (0x4D, 0x3B, 0x2F));
                    }
                }
            }

            // Branches with blossoms
            for branch in &tree.branches {
                let sway_b = (branch.angle.sin() * 2.0) as i32;
                for step in 0..=(branch.length as i32) {
                    let bx = base_x + sway_b + (branch.angle.cos() * step as f32) as i32;
                    let by = (branch.start_y - branch.angle.sin() * step as f32) as i32;
                    if bx >= 0 && bx < area.width as i32 && by >= 0 && by < area.height as i32 {
                        draw_char(buf, bx, by, area, '·', (0x5D, 0x4B, 0x3F));
                    }
                }
                // Blossoms on branch
                for blossom in &branch.blossoms {
                    let bx = base_x + sway_b + (branch.angle.cos() * branch.length * blossom.offset * 0.5) as i32;
                    let by = (branch.start_y - branch.angle.sin() * branch.length * blossom.offset) as i32;
                    if bx >= 0 && bx < area.width as i32 && by >= 0 && by < area.height as i32 {
                        let bloom = (tree.blossom_phase.sin() * 0.3 + 0.7) * blossom.size;
                        let (r, g, b) = SAKURA_COLORS[blossom.color_idx];
                        let ch = SAKURA_BLOSSOM_FRAMES[(tree.blossom_phase * 2.0).floor() as usize % SAKURA_BLOSSOM_FRAMES.len()];
                        draw_char(buf, bx, by, area, ch.chars().next().unwrap(), ((r as f32 * bloom) as u8, (g as f32 * bloom) as u8, (b as f32 * bloom) as u8));
                    }
                }
            }
        }

        // Falling petals
        for petal in &self.petals {
            let x = petal.x as i32;
            let y = petal.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let alpha = 1.0 - (petal.life / 25.0).min(1.0);
                let (r, g, b) = petal.color;
                draw_char(buf, x, y, area, SAKURA_BLOSSOM_FRAMES[petal.frame % SAKURA_BLOSSOM_FRAMES.len()].chars().next().unwrap(),
                    ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
            }
        }
    }
}

// ============================================================================
// THEME 3: ABYSSAL BLOOM (深海発光) - Deep sea bioluminescence
// ============================================================================

#[derive(Clone, Copy)]
enum AbyssalKind { Jellyfish, CombJelly, Siphonophore, Anglerfish }

struct AbyssalCreature {
    x: f32, y: f32,
    vx: f32, vy: f32,
    kind: AbyssalKind,
    pulse_phase: f32, pulse_speed: f32,
    size: f32,
    color: (u8, u8, u8),
    trail: Vec<(f32, f32)>,
}

struct AbyssalParticle {
    x: f32, y: f32,
    vx: f32, vy: f32,
    color: (u8, u8, u8),
    life: f32,
    ch: char,
    size: f32,
}

const ABYSSAL_COLORS: &[(u8, u8, u8)] = &[
    (0x00, 0xFF, 0xFF), (0x00, 0xBF, 0xFF), (0x40, 0x00, 0xFF),
    (0x80, 0x00, 0xFF), (0xBF, 0x00, 0xFF), (0xFF, 0x00, 0xBF),
    (0x00, 0xFF, 0xBF), (0x00, 0xFF, 0x80),
];

struct AbyssalBloom {
    creatures: Vec<AbyssalCreature>,
    particles: Vec<AbyssalParticle>,
    time: f32,
    width: u16, height: u16,
}

impl AbyssalBloom {
    fn new() -> Self { Self { creatures: Vec::new(), particles: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.creatures.clear();
        let count = ((width * height) as f32 / 5000.0).clamp(2.0, 6.0) as usize;
        for _ in 0..count {
            let kind = match rng.gen_range(0..4) {
                0 => AbyssalKind::Jellyfish, 1 => AbyssalKind::CombJelly,
                2 => AbyssalKind::Siphonophore, _ => AbyssalKind::Anglerfish,
            };
            self.creatures.push(AbyssalCreature {
                x: rng.gen_range(8.0..(width - 8) as f32),
                y: rng.gen_range(8.0..(height - 8) as f32),
                vx: rng.gen_range(-0.15..0.15), vy: rng.gen_range(-0.08..0.08),
                kind, pulse_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                pulse_speed: rng.gen_range(0.015..0.05), size: rng.gen_range(1.2..2.2),
                color: *ABYSSAL_COLORS.choose(rng).unwrap(), trail: Vec::new(),
            });
        }
    }
}

impl Theme for AbyssalBloom {
    fn name(&self) -> &'static str { "abyssal-bloom" }
    fn description(&self) -> &'static str { "深海発光 - Bioluminescent deep sea creatures pulsing in darkness" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for c in &mut self.creatures {
            c.pulse_phase += c.pulse_speed * dt * 60.0;
            c.trail.insert(0, (c.x, c.y));
            if c.trail.len() > 12 { c.trail.pop(); }
            c.x += c.vx * dt * 60.0; c.y += c.vy * dt * 60.0;
            if c.x < 5.0 { c.x = 5.0; c.vx = c.vx.abs(); }
            if c.x > self.width as f32 - 5.0 { c.x = self.width as f32 - 5.0; c.vx = -c.vx.abs(); }
            if c.y < 5.0 { c.y = 5.0; c.vy = c.vy.abs(); }
            if c.y > self.height as f32 - 5.0 { c.y = self.height as f32 - 5.0; c.vy = -c.vy.abs(); }

            // Spawn particles
            if rng.gen::<f32>() < 0.015 * dt * 60.0 {
                self.particles.push(AbyssalParticle {
                    x: c.x, y: c.y,
                    vx: rng.gen_range(-0.4..0.4), vy: rng.gen_range(-0.4..0.4),
                    color: c.color, life: 0.0,
                    ch: *['·', '∘', '○', '●', '✦', '⋆', '✧', '◆'].choose(rng).unwrap(),
                    size: rng.gen_range(0.5..1.5),
                });
            }
        }
        self.particles.retain_mut(|p| {
            p.x += p.vx * dt * 60.0; p.y += p.vy * dt * 60.0;
            p.life += dt; p.life < 10.0
        });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        for c in &self.creatures { self.draw_creature(buf, c, area); }
        for p in &self.particles {
            let x = p.x as i32; let y = p.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let alpha = 1.0 - (p.life / 10.0).min(1.0);
                let (r, g, b) = p.color;
                draw_char(buf, x, y, area, p.ch, ((r as f32 * alpha * p.size) as u8, (g as f32 * alpha * p.size) as u8, (b as f32 * alpha * p.size) as u8));
            }
        }
    }
}

impl AbyssalBloom {
    fn draw_creature(&self, buf: &mut Buffer, c: &AbyssalCreature, area: Rect) {
        let cx = c.x as i32; let cy = c.y as i32;
        if cx < 0 || cx >= area.width as i32 || cy < 0 || cy >= area.height as i32 { return; }
        let pulse = (c.pulse_phase.sin() + 1.0) / 2.0;
        let size = (c.size * (0.4 + pulse * 0.6)) as i32;
        let (r, g, b) = c.color;

        // Draw trail
        for (i, &(tx, ty)) in c.trail.iter().enumerate() {
            let alpha = (1.0 - i as f32 / c.trail.len() as f32) * 0.3 * pulse;
            let tx_i = tx as i32; let ty_i = ty as i32;
            if tx_i >= 0 && tx_i < area.width as i32 && ty_i >= 0 && ty_i < area.height as i32 {
                draw_char(buf, tx_i, ty_i, area, '·', ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
            }
        }

        match c.kind {
            AbyssalKind::Jellyfish => {
                // Bell
                for dy in -size..=size {
                    let w = ((size * size - dy * dy) as f32).sqrt() as i32;
                    for dx in -w..=w {
                        let x = cx + dx; let y = cy + dy;
                        if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                            let alpha = (0.3 + pulse * 0.7) * (1.0 - dy.abs() as f32 / size as f32 * 0.5);
                            draw_char(buf, x, y, area, if dy < 0 { '╭' } else { '╰' }, ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                        }
                    }
                }
                // Tentacles
                for i in 0..8 {
                    let angle = self.time * 0.4 + i as f32 * 0.78;
                    for step in 1..=size * 4 {
                        let x = cx + (angle.cos() * step as f32 * 0.4) as i32;
                        let y = cy + size + step;
                        if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                            draw_char(buf, x, y, area, '│', (r, g, b));
                        }
                    }
                }
            }
            AbyssalKind::CombJelly => {
                // Oval body with comb rows
                for dy in -size..=size {
                    let w = (size - dy.abs() / 2) as i32;
                    for dx in -w..=w {
                        let x = cx + dx; let y = cy + dy;
                        if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                            let ch = if dx.abs() == w { if dy < 0 { '╭' } else { '╰' } } else { '◆' };
                            let alpha = pulse * (1.0 - dx.abs() as f32 / w as f32 * 0.3);
                            draw_char(buf, x, y, area, ch, ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                        }
                    }
                }
                // Iridescent comb rows
                for row in 0..5 {
                    let ry = cy - size + row * size * 2 / 5;
                    for dx in -size..=size {
                        let x = cx + dx;
                        if x >= 0 && x < area.width as i32 && ry >= 0 && ry < area.height as i32 {
                            let hue = (self.time * 3.0 + dx as f32 * 0.4 + row as f32).sin();
                            let cr = ((hue + 1.0) * 127.0) as u8;
                            let cg = ((hue * 0.5 + 1.0) * 127.0) as u8;
                            draw_char(buf, x, ry, area, '✦', (cr, cg, 255));
                        }
                    }
                }
            }
            AbyssalKind::Siphonophore => {
                // Chain of zooids
                for i in 0..10 {
                    let sy = cy + i * 3;
                    let sx = cx + (self.time * 0.5 + i as f32 * 0.4).sin() as i32 * 3;
                    if sx >= 0 && sx < area.width as i32 && sy >= 0 && sy < area.height as i32 {
                        let alpha = (1.0 - i as f32 / 10.0) * pulse;
                        draw_char(buf, sx, sy, area, '●', ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                    }
                }
            }
            AbyssalKind::Anglerfish => {
                draw_char(buf, cx, cy, area, '🐟', (r, g, b));
                let lure_x = cx + (self.time * 2.5).cos() as i32 * 4;
                let lure_y = cy - 3;
                if lure_x >= 0 && lure_x < area.width as i32 && lure_y >= 0 && lure_y < area.height as i32 {
                    draw_char(buf, lure_x, lure_y, area, '✦', (0xFF, 0xFF, 0x00));
                }
            }
        }
    }
}

// ============================================================================
// THEME 4: FIREFLY DUSK (蛍の黄昏) - Fireflies at dusk
// ============================================================================

struct Firefly {
    x: f32, y: f32,
    vx: f32, vy: f32,
    blink_phase: f32, blink_speed: f32,
    blink_pattern: usize,
    color: (u8, u8, u8),
    trail: Vec<(f32, f32)>,
}

struct GrassBlade {
    x: f32, base_y: f32,
    height: usize,
    sway_phase: f32, sway_speed: f32,
    color: (u8, u8, u8),
}

const FIREFLY_COLORS: &[(u8, u8, u8)] = &[
    (0xFF, 0xFF, 0x00), (0xFF, 0xE4, 0x00), (0xAD, 0xFF, 0x2F), (0xFF, 0xD7, 0x00), (0xFF, 0xF0, 0x50),
];

const GRASS_COLORS: &[(u8, u8, u8)] = &[
    (0x1A, 0x5C, 0x2E), (0x22, 0x6B, 0x3A), (0x15, 0x4D, 0x25), (0x2E, 0x8B, 0x57),
];

struct FireflyDusk {
    fireflies: Vec<Firefly>,
    grass: Vec<GrassBlade>,
    time: f32,
    width: u16, height: u16,
}

impl FireflyDusk {
    fn new() -> Self { Self { fireflies: Vec::new(), grass: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.fireflies.clear(); self.grass.clear();
        for _ in 0..((width * height) as f32 / 6000.0).clamp(4.0, 15.0) as usize {
            self.fireflies.push(Firefly {
                x: rng.gen_range(0.0..width as f32),
                y: rng.gen_range(0.0..height as f32 * 0.75),
                vx: rng.gen_range(-0.2..0.2), vy: rng.gen_range(-0.15..0.15),
                blink_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                blink_speed: rng.gen_range(0.04..0.12),
                blink_pattern: rng.gen_range(0..3),
                color: *FIREFLY_COLORS.choose(rng).unwrap(),
                trail: Vec::new(),
            });
        }
        let ground = height as f32 - 1.0;
        for x in (0..width).step_by(2) {
            self.grass.push(GrassBlade {
                x: x as f32 + rng.gen_range(-0.4..0.4), base_y: ground,
                height: rng.gen_range(3..7), sway_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                sway_speed: rng.gen_range(0.0008..0.002),
                color: *GRASS_COLORS.choose(rng).unwrap(),
            });
        }
    }
}

impl Theme for FireflyDusk {
    fn name(&self) -> &'static str { "firefly-dusk" }
    fn description(&self) -> &'static str { "蛍の黄昏 - Fireflies dancing at dusk, grass swaying gently" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for f in &mut self.fireflies {
            f.blink_phase += f.blink_speed * dt * 60.0;
            f.trail.insert(0, (f.x, f.y));
            if f.trail.len() > 8 { f.trail.pop(); }
            f.x += f.vx * dt * 60.0; f.y += f.vy * dt * 60.0;
            f.vx += rng.gen_range(-0.015..0.015) * dt * 60.0;
            f.vy += rng.gen_range(-0.015..0.015) * dt * 60.0;
            f.vx = f.vx.clamp(-0.4, 0.4); f.vy = f.vy.clamp(-0.25, 0.25);
            if f.x < 0.0 { f.x = self.width as f32; } if f.x > self.width as f32 { f.x = 0.0; }
            if f.y < 0.0 { f.y = self.height as f32 * 0.75; } if f.y > self.height as f32 * 0.75 { f.y = 0.0; }
        }
        for g in &mut self.grass { g.sway_phase += g.sway_speed * dt * 60.0; }
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Grass
        for g in &self.grass {
            let x = g.x as i32; let by = g.base_y as i32;
            let h = g.height as i32;
            for i in 0..h {
                let sway = (g.sway_phase.sin() * (1.0 + i as f32 * 0.3)) as i32;
                let sx = x + sway; let sy = by - i;
                if sx >= 0 && sx < area.width as i32 && sy >= 0 && sy < area.height as i32 {
                    let ch = if i == h - 1 { '╿' } else { '│' };
                    let alpha = 0.4 + 0.6 * (1.0 - i as f32 / h as f32);
                    let (r, g_, b) = g.color;
                    draw_char(buf, sx, sy, area, ch, ((r as f32 * alpha) as u8, (g_ as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                }
            }
        }
        // Fireflies with trails
        for f in &self.fireflies {
            // Trail
            for (i, &(tx, ty)) in f.trail.iter().enumerate() {
                let tx_i = tx as i32; let ty_i = ty as i32;
                if tx_i >= 0 && tx_i < area.width as i32 && ty_i >= 0 && ty_i < area.height as i32 {
                    let alpha = (1.0 - i as f32 / f.trail.len() as f32) * 0.4;
                    let (r, g, b) = f.color;
                    draw_char(buf, tx_i, ty_i, area, '·', ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                }
            }
            // Main body
            let x = f.x as i32; let y = f.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let blink = match f.blink_pattern {
                    0 => (f.blink_phase.sin() + 1.0) / 2.0,                    // Smooth pulse
                    1 => if (f.blink_phase * 2.0).sin() > 0.0 { 1.0 } else { 0.1 }, // Sharp flash
                    _ => (f.blink_phase.sin() * 0.5 + 0.5).powf(2.0),          // Slow fade
                };
                if blink > 0.15 {
                    let (r, g, b) = f.color;
                    let ch = if blink > 0.8 { '✦' } else if blink > 0.5 { '⋆' } else { '·' };
                    draw_char(buf, x, y, area, ch, ((r as f32 * blink) as u8, (g as f32 * blink) as u8, (b as f32 * blink) as u8));
                    // Glow halo
                    if blink > 0.6 {
                        for (dx, dy) in [(-1,0),(1,0),(0,-1),(0,1),(-1,-1),(1,-1),(-1,1),(1,1)] {
                            let hx = x + dx; let hy = y + dy;
                            if hx >= 0 && hx < area.width as i32 && hy >= 0 && hy < area.height as i32 {
                                let (r, g, b) = f.color;
                                draw_char(buf, hx, hy, area, '·', ((r as f32 * blink * 0.3) as u8, (g as f32 * blink * 0.3) as u8, (b as f32 * blink * 0.3) as u8));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// THEME 5: AURORA VOID (極光の虚空) - Cosmic aurora
// ============================================================================

struct AuroraCurtain {
    x: f32, phase: f32, speed: f32,
    color1: (u8, u8, u8), color2: (u8, u8, u8),
    width: f32, intensity: f32,
}

struct StarAurora {
    x: f32, y: f32,
    brightness: f32, twinkle_speed: f32, twinkle_phase: f32,
    color: (u8, u8, u8),
}

const AURORA_COLOR_PAIRS: &[((u8,u8,u8), (u8,u8,u8))] = &[
    ((0x00,0xFF,0x7F), (0x00,0xBF,0xFF)),
    ((0xBF,0x00,0xFF), (0xFF,0x00,0x7F)),
    ((0xFF,0xD7,0x00), (0xFF,0x8C,0x00)),
    ((0x00,0xFF,0xBF), (0x00,0xFF,0xFF)),
];

struct AuroraVoid {
    curtains: Vec<AuroraCurtain>,
    stars: Vec<StarAurora>,
    time: f32,
    width: u16, height: u16,
}

impl AuroraVoid {
    fn new() -> Self { Self { curtains: Vec::new(), stars: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.curtains.clear(); self.stars.clear();
        for i in 0..6 {
            let (c1, c2) = *AURORA_COLOR_PAIRS.choose(rng).unwrap();
            self.curtains.push(AuroraCurtain {
                x: i as f32 * width as f32 / 6.0 + rng.gen_range(-15.0..15.0),
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
                speed: rng.gen_range(0.0003..0.0015),
                color1: c1, color2: c2,
                width: rng.gen_range(25.0..50.0),
                intensity: rng.gen_range(0.6..1.0),
            });
        }
        for _ in 0..((width * height) as f32 / 400.0).clamp(30.0, 120.0) as usize {
            self.stars.push(StarAurora {
                x: rng.gen_range(0.0..width as f32), y: rng.gen_range(0.0..height as f32),
                brightness: rng.gen_range(0.15..1.0),
                twinkle_speed: rng.gen_range(0.015..0.06),
                twinkle_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                color: *[(0xFF,0xFF,0xFF), (0xFF,0xF0,0xE0), (0xE0,0xE0,0xFF), (0xFF,0xFF,0xE0)].choose(rng).unwrap(),
            });
        }
    }
}

impl Theme for AuroraVoid {
    fn name(&self) -> &'static str { "aurora-void" }
    fn description(&self) -> &'static str { "極光の虚空 - Cosmic aurora curtains drifting, stars twinkling" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for c in &mut self.curtains {
            c.phase += c.speed * dt * 60.0;
            c.x += 0.015 * dt * 60.0;
            if c.x > self.width as f32 + c.width { c.x = -c.width; }
        }
        for s in &mut self.stars { s.twinkle_phase += s.twinkle_speed * dt * 60.0; }
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Aurora curtains with color blending
        for c in &self.curtains {
            for y in 0..area.height {
                let rel_y = y as f32;
                let wave1 = (rel_y * 0.04 + c.phase).sin();
                let wave2 = (rel_y * 0.07 + c.phase * 1.3).sin() * 0.5;
                let combined = wave1 + wave2;
                let curtain_x = (c.x + combined * c.width * 0.4) as i32;
                let curtain_w = (c.width * (0.6 + combined.abs() * 0.4)) as i32;

                for dx in -curtain_w..=curtain_w {
                    let x = curtain_x + dx;
                    if x >= 0 && x < area.width as i32 {
                        let dist = dx.abs() as f32 / curtain_w as f32;
                        let alpha = (1.0 - dist) * (0.08 + 0.12 * combined.abs()) * c.intensity;
                        if alpha > 0.015 {
                            // Blend colors based on wave
                            let blend = (combined + 1.0) / 2.0;
                            let (r, g, b) = lerp_color(c.color1, c.color2, blend);
                            draw_char(buf, x, y as i32, area, if alpha > 0.05 { '░' } else { '▒' }, (r, g, b));
                        }
                    }
                }
            }
        }
        // Stars
        for s in &self.stars {
            let x = s.x as i32; let y = s.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let twinkle = (s.twinkle_phase.sin() + 1.0) / 2.0;
                let brightness = s.brightness * (0.4 + twinkle * 0.6);
                if brightness > 0.2 {
                    let ch = if brightness > 0.85 { '✦' } else if brightness > 0.65 { '⋆' } else if brightness > 0.4 { '·' } else { ' ' };
                    if ch != ' ' {
                        let (r, g, b) = s.color;
                        let v = (255.0 * brightness) as u8;
                        draw_char(buf, x, y, area, ch, (r.min(v), g.min(v), b.min(v)));
                    }
                }
            }
        }
    }
}

// ============================================================================
// THEME 6: KOI DREAMS (錦鯉の夢) - Koi fish in pond
// ============================================================================

#[derive(Clone, Copy)]
enum KoiPattern { Kohaku, Sanke, Showa, Ogon, Utsuri }

struct KoiFish {
    x: f32, y: f32,
    vx: f32, vy: f32,
    angle: f32,
    pattern: KoiPattern,
    size: f32,
    swim_phase: f32,
    body_color: (u8, u8, u8),
    spot_colors: &'static [(u8, u8, u8)],
}

struct WaterLily {
    x: f32, y: f32,
    bloom: f32, bloom_speed: f32,
    color: (u8, u8, u8),
    petals: usize,
}

struct Ripple {
    x: f32, y: f32,
    radius: f32, max_radius: f32,
    life: f32,
    alpha: f32,
}

const KOI_PATTERNS: &[(KoiPattern, (u8,u8,u8), &[(u8,u8,u8)])] = &[
    (KoiPattern::Kohaku, (0xFF,0xFF,0xFF), &[(0xFF,0x00,0x00)]),
    (KoiPattern::Sanke, (0xFF,0xFF,0xFF), &[(0xFF,0x00,0x00), (0x00,0x00,0x00)]),
    (KoiPattern::Showa, (0x00,0x00,0x00), &[(0xFF,0x00,0x00), (0xFF,0xFF,0xFF)]),
    (KoiPattern::Ogon, (0xFF,0xD7,0x00), &[]),
    (KoiPattern::Utsuri, (0x00,0x00,0x00), &[(0xFF,0xFF,0xFF)]),
];

struct KoiDreams {
    koi: Vec<KoiFish>,
    lilies: Vec<WaterLily>,
    ripples: Vec<Ripple>,
    time: f32,
    width: u16, height: u16,
}

impl KoiDreams {
    fn new() -> Self { Self { koi: Vec::new(), lilies: Vec::new(), ripples: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.koi.clear(); self.lilies.clear(); self.ripples.clear();
        for _ in 0..((width * height) as f32 / 10000.0).clamp(2.0, 5.0) as usize {
            let (pattern, body, spots) = *KOI_PATTERNS.choose(rng).unwrap();
            self.koi.push(KoiFish {
                x: rng.gen_range(15.0..(width - 15) as f32),
                y: rng.gen_range(8.0..(height - 8) as f32),
                vx: rng.gen_range(-0.25..0.25), vy: rng.gen_range(-0.08..0.08),
                angle: rng.gen_range(0.0..std::f32::consts::TAU),
                pattern, size: rng.gen_range(1.8..3.5), swim_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                body_color: body, spot_colors: spots,
            });
        }
        for _ in 0..(width as f32 / 22.0).clamp(2.0, 7.0) as usize {
            self.lilies.push(WaterLily {
                x: rng.gen_range(8.0..(width - 8) as f32),
                y: rng.gen_range(4.0..(height - 4) as f32),
                bloom: rng.gen_range(0.2..0.8), bloom_speed: rng.gen_range(0.0005..0.0015),
                color: *[(0xFF,0x69,0xB4), (0xFF,0xFF,0xFF), (0xFF,0xB6,0xC1), (0xE6,0xE6,0xFA), (0xFF,0xE4,0xE1)].choose(rng).unwrap(),
                petals: rng.gen_range(6..10),
            });
        }
    }
}

impl Theme for KoiDreams {
    fn name(&self) -> &'static str { "koi-dreams" }
    fn description(&self) -> &'static str { "錦鯉の夢 - Koi fish gliding, water lilies blooming, gentle ripples" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for k in &mut self.koi {
            k.swim_phase += 0.025 * dt * 60.0;
            k.angle += (k.swim_phase.sin() * 0.02 + k.vx * 0.5) * dt * 60.0;
            k.x += k.angle.cos() * (0.8 + k.swim_phase.sin() * 0.15) * dt * 60.0;
            k.y += k.angle.sin() * (0.8 + k.swim_phase.sin() * 0.15) * dt * 60.0 + k.vy * dt * 60.0;

            if k.x < 10.0 { k.x = 10.0; k.angle = 0.0; } if k.x > self.width as f32 - 10.0 { k.x = self.width as f32 - 10.0; k.angle = std::f32::consts::PI; }
            if k.y < 5.0 { k.y = 5.0; k.vy = k.vy.abs(); } if k.y > self.height as f32 - 5.0 { k.y = self.height as f32 - 5.0; k.vy = -k.vy.abs(); }

            if rng.gen::<f32>() < 0.0015 * dt * 60.0 {
                self.ripples.push(Ripple { x: k.x, y: k.y, radius: 0.0, max_radius: rng.gen_range(6.0..18.0), life: 0.0, alpha: rng.gen_range(0.3..0.6) });
            }
        }
        for l in &mut self.lilies {
            l.bloom += l.bloom_speed * dt * 60.0;
            if l.bloom > 1.0 { l.bloom = 0.2; }
        }
        self.ripples.retain_mut(|r| { r.radius += 0.4 * dt * 60.0; r.life += dt; r.alpha *= 0.995; r.radius < r.max_radius && r.life < 12.0 });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Water lilies
        for l in &self.lilies {
            let x = l.x as i32; let y = l.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let (r, g, b) = l.color;
                let alpha = l.bloom;
                let petal_ch = if l.bloom > 0.7 { '✿' } else { '❀' };
                for i in 0..l.petals {
                    let angle = i as f32 * std::f32::consts::TAU / l.petals as f32;
                    let px = x + (angle.cos() * (3.0 + l.bloom * 4.0)) as i32;
                    let py = y + (angle.sin() * (2.0 + l.bloom * 3.0) * 0.6) as i32;
                    if px >= 0 && px < area.width as i32 && py >= 0 && py < area.height as i32 {
                        draw_char(buf, px, py, area, petal_ch, ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
                    }
                }
                // Center
                draw_char(buf, x, y, area, '✶', (0xFF, 0xD7, 0x00));
            }
        }
        // Ripples
        for r in &self.ripples {
            let cx = r.x as i32; let cy = r.y as i32;
            let rad = r.radius as i32;
            let alpha = r.alpha * (1.0 - r.radius / r.max_radius);
            if alpha < 0.02 { continue; }
            for angle_step in 0..72 {
                let angle = angle_step as f32 * std::f32::consts::TAU / 72.0;
                let x = cx + (angle.cos() * rad as f32) as i32;
                let y = cy + (angle.sin() * rad as f32 * 0.5) as i32;
                if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 && DRAW_RNG.with(|rng| rng.borrow_mut().gen::<f32>()) < alpha * 0.4 {
                    draw_char(buf, x, y, area, '∼', (0x00, (0xBF as f32 * alpha) as u8, (0xFF as f32 * alpha) as u8));
                }
            }
        }
        // Koi fish
        for k in &self.koi {
            let x = k.x as i32; let y = k.y as i32;
            let sz = k.size as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                // Body segments
                for seg in 0..=sz {
                    let seg_x = x + (k.angle.cos() * (sz - seg) as f32) as i32;
                    let seg_y = y + (k.angle.sin() * (sz - seg) as f32 * 0.4) as i32;
                    let seg_w = (sz - seg) / 2 + 1;
                    for dx in -seg_w..=seg_w {
                        let px = seg_x + dx;
                        let py = seg_y;
                        if px >= 0 && px < area.width as i32 && py >= 0 && py < area.height as i32 {
                            let ch = if seg == 0 { '►' } else if seg == sz { '●' } else { '▬' };
                            let (r, g, b) = k.body_color;
                            draw_char(buf, px, py, area, ch, (r, g, b));
                        }
                    }
                    // Spots
                    if !k.spot_colors.is_empty() && seg % 2 == 0 && DRAW_RNG.with(|rng| rng.borrow_mut().gen::<f32>()) < 0.3 {
                        for spot_color in k.spot_colors.iter() {
                            let sx = seg_x + (k.angle.sin() * 2.0) as i32;
                            let sy = seg_y;
                            if sx >= 0 && sx < area.width as i32 && sy >= 0 && sy < area.height as i32 {
                                draw_char(buf, sx, sy, area, '◆', *spot_color);
                            }
                        }
                    }
                }
                // Tail
                let tx = x - (k.angle.cos() * (sz + 2) as f32) as i32;
                let ty = y - (k.angle.sin() * (sz + 1) as f32 * 0.4) as i32;
                if tx >= 0 && tx < area.width as i32 && ty >= 0 && ty < area.height as i32 {
                    draw_char(buf, tx, ty, area, '◁', k.body_color);
                }
                // Fins
                let fx1 = x + (k.angle.cos() * (sz / 2) as f32 - k.angle.sin() * 3.0) as i32;
                let fy1 = y + (k.angle.sin() * (sz / 2) as f32 + k.angle.cos() * 3.0 * 0.4) as i32;
                if fx1 >= 0 && fx1 < area.width as i32 && fy1 >= 0 && fy1 < area.height as i32 {
                    draw_char(buf, fx1, fy1, area, '△', k.body_color);
                }
            }
        }
    }
}

// ============================================================================
// THEME 7: BAMBOO WHISPER (竹の囁き) - Bamboo forest
// ============================================================================

struct BambooStalk {
    x: f32, y: f32,
    height: usize, segments: usize,
    sway_phase: f32, sway_speed: f32,
    thickness: f32,
    leaf_bunches: Vec<LeafBunch>,
}

struct LeafBunch {
    segment: usize,
    angle: f32,
    length: f32,
    leaves: usize,
}

struct BambooLeaf {
    x: f32, y: f32,
    vx: f32, vy: f32,
    rotation: f32, rot_speed: f32,
    life: f32,
    char: char,
    color: (u8, u8, u8),
}

const BAMBOO_COLORS: &[(u8, u8, u8)] = &[
    (0x2E, 0x8B, 0x57), (0x3C, 0xB3, 0x71), (0x6B, 0x8E, 0x23),
    (0x8F, 0xBC, 0x8F), (0x55, 0x6B, 0x2F), (0x4A, 0x7C, 0x59),
];

const LEAF_CHARS: &[char] = &['🍃', '🍂', '🌿', '🍀', '➤', '➥', '▸', '▹'];

struct BambooWhisper {
    bamboos: Vec<BambooStalk>,
    leaves: Vec<BambooLeaf>,
    time: f32,
    width: u16, height: u16,
}

impl BambooWhisper {
    fn new() -> Self { Self { bamboos: Vec::new(), leaves: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.bamboos.clear(); self.leaves.clear();
        let count = (width as f32 / 20.0).clamp(3.0, 10.0) as usize;
        let ground = height as f32 - 1.0;
        for _ in 0..count {
            let h = rng.gen_range(height as usize / 3..height as usize * 3 / 4);
            let segs = (h / 4).max(3);
            let mut bunches = Vec::new();
            for b in 0..rng.gen_range(3..6) {
                bunches.push(LeafBunch {
                    segment: rng.gen_range(segs.saturating_sub(4)..segs),
                    angle: rng.gen_range(-0.6..0.6),
                    length: rng.gen_range(4.0..10.0),
                    leaves: rng.gen_range(4..8),
                });
            }
            self.bamboos.push(BambooStalk {
                x: rng.gen_range(4.0..(width - 4) as f32), y: ground,
                height: h, segments: segs,
                sway_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                sway_speed: rng.gen_range(0.0008..0.0025),
                thickness: rng.gen_range(0.8..1.5),
                leaf_bunches: bunches,
            });
        }
    }
}

impl Theme for BambooWhisper {
    fn name(&self) -> &'static str { "bamboo-whisper" }
    fn description(&self) -> &'static str { "竹の囁き - Bamboo forest swaying, leaves drifting down" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for b in &mut self.bamboos {
            b.sway_phase += b.sway_speed * dt * 60.0;
            if rng.gen::<f32>() < 0.0003 * dt * 60.0 {
                let bunch = b.leaf_bunches.choose(rng).unwrap();
                let seg_y = b.y - (bunch.segment * b.height / b.segments) as f32;
                self.leaves.push(BambooLeaf {
                    x: b.x + (b.sway_phase.sin() * bunch.length * 0.5),
                    y: seg_y,
                    vx: rng.gen_range(-0.15..0.15), vy: rng.gen_range(0.08..0.25),
                    rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                    rot_speed: rng.gen_range(-0.01..0.01),
                    life: 0.0, char: *LEAF_CHARS.choose(rng).unwrap(),
                    color: *BAMBOO_COLORS.choose(rng).unwrap(),
                });
            }
        }
        self.leaves.retain_mut(|l| {
            l.x += l.vx * dt * 60.0; l.y += l.vy * dt * 60.0;
            l.rotation += l.rot_speed * dt * 60.0; l.life += dt;
            l.life < 18.0 && l.y < self.height as f32 + 2.0
        });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        for b in &self.bamboos {
            let sway = (b.sway_phase.sin() * 2.5) as i32;
            let bx = b.x as i32 + sway;
            let by = b.y as i32;
            let seg_h = (b.height / b.segments.max(1)) as i32;
            let segs = b.segments as i32;

            // Stalk segments
            for seg in 0..segs {
                let y = by - seg * seg_h;
                let x = bx + sway * seg / segs;
                for i in 0..seg_h {
                    let sy = y - i;
                    if sy >= 0 && sy < area.height as i32 && x >= 0 && x < area.width as i32 {
                        draw_char(buf, x, sy, area, '│', BAMBOO_COLORS[0]);
                    }
                }
                // Node ring
                if y >= 0 && y < area.height as i32 && x >= 0 && x < area.width as i32 {
                    draw_char(buf, x, y, area, '┼', BAMBOO_COLORS[1]);
                }
            }

            // Leaf bunches at top
            for bunch in &b.leaf_bunches {
                let seg_y = by - bunch.segment as i32 * seg_h;
                let seg_x = bx + sway * (bunch.segment as i32) / segs;
                let bunch_leaves = bunch.leaves as i32;
                    for side in [-1, 1] {
                        for l in 0..bunch_leaves {
                            let lx = seg_x + side * (l + (bunch.length as i32 * l / bunch_leaves) / 2);
                            let ly = seg_y - l - (bunch.length as i32 * l / bunch_leaves) / 3;
                        if lx >= 0 && lx < area.width as i32 && ly >= 0 && ly < area.height as i32 {
                            draw_char(buf, lx, ly, area, if side < 0 { '╴' } else { '╶' }, BAMBOO_COLORS[2]);
                        }
                    }
                }
            }
        }
        // Falling leaves
        for l in &self.leaves {
            let x = l.x as i32; let y = l.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let alpha = 1.0 - (l.life / 18.0).min(1.0);
                let sway_x = (l.rotation.sin() * 0.8) as i32;
                draw_char(buf, x + sway_x, y, area, l.char, ((l.color.0 as f32 * alpha) as u8, (l.color.1 as f32 * alpha) as u8, (l.color.2 as f32 * alpha) as u8));
            }
        }
    }
}

// ============================================================================
// THEME 8: NEBULA HEART (星雲の心臓) - Cosmic nebula
// ============================================================================

struct NebulaParticle {
    x: f32, y: f32, vx: f32, vy: f32,
    color: (u8, u8, u8), life: f32, size: f32,
    orbit_angle: f32, orbit_speed: f32,
}

struct NebulaCore {
    x: f32, y: f32, vx: f32, vy: f32,
    radius: f32, color: (u8, u8, u8),
    pulse_phase: f32, pulse_speed: f32,
    rotation: f32,
}

const NEBULA_COLORS: &[(u8, u8, u8)] = &[
    (0xFF, 0x00, 0xFF), (0x00, 0xFF, 0xFF), (0xFF, 0xFF, 0x00),
    (0xFF, 0x00, 0x7F), (0xBF, 0x00, 0xFF), (0x00, 0xFF, 0xBF),
];

struct NebulaHeart {
    particles: Vec<NebulaParticle>,
    cores: Vec<NebulaCore>,
    time: f32,
    width: u16, height: u16,
}

impl NebulaHeart {
    fn new() -> Self { Self { particles: Vec::new(), cores: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.particles.clear(); self.cores.clear();
        for _ in 0..4 {
            self.cores.push(NebulaCore {
                x: rng.gen_range(width as f32 * 0.15..width as f32 * 0.85),
                y: rng.gen_range(height as f32 * 0.15..height as f32 * 0.85),
                vx: rng.gen_range(-0.04..0.04), vy: rng.gen_range(-0.04..0.04),
                radius: rng.gen_range(18.0..35.0),
                color: *NEBULA_COLORS.choose(rng).unwrap(),
                pulse_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                pulse_speed: rng.gen_range(0.008..0.025),
                rotation: rng.gen_range(0.0..std::f32::consts::TAU),
            });
        }
        for _ in 0..((width * height) as f32 / 180.0).clamp(120.0, 600.0) as usize {
            let core = self.cores.choose(rng).unwrap();
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist = rng.gen_range(0.0..core.radius * 0.8);
            self.particles.push(NebulaParticle {
                x: core.x + angle.cos() * dist, y: core.y + angle.sin() * dist,
                vx: 0.0, vy: 0.0, color: core.color,
                life: rng.gen_range(0.0..25.0), size: rng.gen_range(0.4..1.8),
                orbit_angle: angle, orbit_speed: rng.gen_range(0.002..0.008),
            });
        }
    }
}

impl Theme for NebulaHeart {
    fn name(&self) -> &'static str { "nebula-heart" }
    fn description(&self) -> &'static str { "星雲の心臓 - Nebula cores pulsing, particles swirling in cosmic dance" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for c in &mut self.cores {
            c.pulse_phase += c.pulse_speed * dt * 60.0;
            c.rotation += 0.002 * dt * 60.0;
            c.x += c.vx * dt * 60.0; c.y += c.vy * dt * 60.0;
            if c.x < c.radius { c.x = c.radius; c.vx = c.vx.abs(); }
            if c.x > self.width as f32 - c.radius { c.x = self.width as f32 - c.radius; c.vx = -c.vx.abs(); }
            if c.y < c.radius { c.y = c.radius; c.vy = c.vy.abs(); }
            if c.y > self.height as f32 - c.radius { c.y = self.height as f32 - c.radius; c.vy = -c.vy.abs(); }
        }
        for p in &mut self.particles {
            let mut nearest = &self.cores[0]; let mut md = f32::MAX;
            for c in &self.cores { let dx = p.x - c.x; let dy = p.y - c.y; let d = dx*dx+dy*dy; if d < md { md = d; nearest = c; } }
            let dx = nearest.x - p.x; let dy = nearest.y - p.y;
            let dist = (dx*dx + dy*dy).sqrt().max(1.0);
            // Orbital motion
            p.orbit_angle += p.orbit_speed * dt * 60.0;
            let target_x = nearest.x + p.orbit_angle.cos() * (nearest.radius * 0.7);
            let target_y = nearest.y + p.orbit_angle.sin() * (nearest.radius * 0.7) * 0.6;
            p.vx += (target_x - p.x) * 0.02 * dt * 60.0;
            p.vy += (target_y - p.y) * 0.02 * dt * 60.0;
            p.vx *= 0.98; p.vy *= 0.98;
            p.x += p.vx * dt * 60.0; p.y += p.vy * dt * 60.0;
            p.life += dt;
            if p.life > 35.0 || (p.x - nearest.x).abs() > nearest.radius * 1.8 || (p.y - nearest.y).abs() > nearest.radius * 1.8 {
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist = rng.gen_range(0.0..nearest.radius * 0.7);
                p.x = nearest.x + angle.cos() * dist; p.y = nearest.y + angle.sin() * dist;
                p.vx = 0.0; p.vy = 0.0; p.life = 0.0; p.orbit_angle = angle;
            }
        }
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Particles
        for p in &self.particles {
            let x = p.x as i32; let y = p.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let (r, g, b) = p.color;
                let alpha = (1.0 - (p.life / 35.0).min(1.0)) * p.size;
                let ch = if p.size > 1.4 { '✦' } else if p.size > 1.0 { '⋆' } else if p.size > 0.7 { '·' } else { '░' };
                draw_char(buf, x, y, area, ch, ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8));
            }
        }
        // Cores with pulsating glow
        for c in &self.cores {
            let cx = c.x as i32; let cy = c.y as i32;
            let rad = c.radius as i32;
            let pulse = (c.pulse_phase.sin() + 1.0) / 2.0;
            let (r, g, b) = c.color;
            // Outer glow
            for dy in -rad..=rad {
                let w = ((rad * rad - dy * dy) as f32).sqrt() as i32;
                for dx in -w..=w {
                    let x = cx + dx; let y = cy + dy;
                    if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                        let dist = (dx*dx + dy*dy) as f32 / (rad*rad) as f32;
                        let alpha = (1.0 - dist.sqrt()) * pulse * 0.25;
                        if alpha > 0.015 { draw_char(buf, x, y, area, '░', ((r as f32 * alpha) as u8, (g as f32 * alpha) as u8, (b as f32 * alpha) as u8)); }
                    }
                }
            }
            // Core center with rotation
            for i in 0..6 {
                let angle = c.rotation + i as f32 * std::f32::consts::TAU / 6.0;
                for step in 1..=rad / 3 {
                    let x = cx + (angle.cos() * step as f32) as i32;
                    let y = cy + (angle.sin() * step as f32 * 0.5) as i32;
                    if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                        draw_char(buf, x, y, area, '✶', (r, g, b));
                    }
                }
            }
        }
    }
}

// ============================================================================
// THEME 9: RAIN GLASS (雨のガラス) - Rain on window
// ============================================================================

struct RainDrop {
    x: f32, y: f32, vy: f32,
    size: f32, trail: f32,
    wobble: f32,
}

struct RainStream {
    x: f32, y: f32,
    length: f32, speed: f32, alpha: f32,
    wobble_phase: f32,
}

struct RainGlass {
    drops: Vec<RainDrop>,
    streams: Vec<RainStream>,
    condensation: Vec<(f32, f32, f32)>, // x, y, size
    time: f32,
    width: u16, height: u16,
}

impl RainGlass {
    fn new() -> Self { Self { drops: Vec::new(), streams: Vec::new(), condensation: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.drops.clear(); self.streams.clear(); self.condensation.clear();
        for _ in 0..((width * height) as f32 / 700.0).clamp(25.0, 120.0) as usize {
            self.drops.push(RainDrop {
                x: rng.gen_range(0.0..width as f32), y: rng.gen_range(-30.0..height as f32),
                vy: rng.gen_range(2.5..7.0), size: rng.gen_range(0.4..1.8),
                trail: rng.gen_range(4.0..14.0), wobble: rng.gen_range(0.0..std::f32::consts::TAU),
            });
        }
        // Condensation droplets
        for _ in 0..(width * height / 2000).clamp(15, 80) {
            self.condensation.push((
                rng.gen_range(0.0..width as f32),
                rng.gen_range(0.0..height as f32),
                rng.gen_range(0.3..1.0),
            ));
        }
    }
}

impl Theme for RainGlass {
    fn name(&self) -> &'static str { "rain-glass" }
    fn description(&self) -> &'static str { "雨のガラス - Rain drops sliding down glass, streams merging, condensation" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for d in &mut self.drops {
            d.y += d.vy * dt * 60.0; d.wobble += 0.05 * dt * 60.0;
            if rng.gen::<f32>() < 0.008 * dt * 60.0 && d.y > 8.0 {
                self.streams.push(RainStream { x: d.x + d.wobble.sin() * 0.5, y: d.y,
                    length: rng.gen_range(6.0..22.0), speed: rng.gen_range(1.5..4.0),
                    alpha: rng.gen_range(0.25..0.7), wobble_phase: rng.gen_range(0.0..std::f32::consts::TAU) });
            }
            if d.y > self.height as f32 + 8.0 {
                d.x = rng.gen_range(0.0..self.width as f32); d.y = rng.gen_range(-30.0..0.0);
                d.vy = rng.gen_range(2.5..7.0); d.size = rng.gen_range(0.4..1.8);
                d.trail = rng.gen_range(4.0..14.0); d.wobble = rng.gen_range(0.0..std::f32::consts::TAU);
            }
        }
        self.streams.retain_mut(|s| { s.y += s.speed * dt * 60.0; s.wobble_phase += 0.03 * dt * 60.0; s.alpha -= 0.004 * dt * 60.0; s.alpha > 0.04 && s.y < self.height as f32 + 12.0 });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        // Condensation
        for &(x, y, sz) in &self.condensation {
            let xi = x as i32; let yi = y as i32;
            if xi >= 0 && xi < area.width as i32 && yi >= 0 && yi < area.height as i32 {
                let ch = if sz > 0.7 { '○' } else if sz > 0.4 { '·' } else { '░' };
                draw_char(buf, xi, yi, area, ch, (0xB0, 0xC4, 0xDE));
            }
        }
        // Streams
        for s in &self.streams {
            let x = s.x as i32; let len = s.length as i32;
            for i in 0..len {
                let wobble = (s.wobble_phase + i as f32 * 0.1).sin() * 0.5;
                let xi = x + wobble as i32;
                let yi = (s.y - i as f32) as i32;
                if xi >= 0 && xi < area.width as i32 && yi >= 0 && yi < area.height as i32 {
                    let alpha = s.alpha * (1.0 - i as f32 / len as f32);
                    if alpha > 0.04 {
                        draw_char(buf, xi, yi, area, '│', ((0x87 as f32 * alpha) as u8, (0xCE as f32 * alpha) as u8, (0xEB as f32 * alpha) as u8));
                    }
                }
            }
        }
        // Drops with trails
        for d in &self.drops {
            let x = d.x as i32; let y = d.y as i32;
            let tr = d.trail as i32;
            for i in 0..=tr {
                let ty = y - i;
                let wobble = (d.wobble + i as f32 * 0.08).sin() * 0.3;
                let xi = x + wobble as i32;
                if xi >= 0 && xi < area.width as i32 && ty >= 0 && ty < area.height as i32 {
                    let alpha = 1.0 - i as f32 / tr as f32;
                    let ch = if i == 0 { '●' } else if i == 1 { '○' } else if i < 3 { '·' } else { '░' };
                    draw_char(buf, xi, ty, area, ch, ((0x87 as f32 * alpha * 0.8) as u8, (0xCE as f32 * alpha * 0.8) as u8, (0xEB as f32 * alpha * 0.8) as u8));
                }
            }
            // Impact splash at bottom
            if (y as f32) > self.height as f32 - 3.0 {
                for _ in 0..2 {
                    let sx = x + DRAW_RNG.with(|rng| rng.borrow_mut().gen_range(-2..3));
                    let sy = self.height as i32 - 1 - DRAW_RNG.with(|rng| rng.borrow_mut().gen_range(0..2));
                    if sx >= 0 && sx < area.width as i32 {
                        draw_char(buf, sx, sy, area, '⋆', (0x87, 0xCE, 0xEB));
                    }
                }
            }
        }
    }
}

// ============================================================================
// THEME 10: WILLOW TEARS (垂柳の涙) - Weeping willow
// ============================================================================

struct WillowTree {
    x: f32, y: f32,
    trunk_h: usize,
    branches: Vec<WillowBranch>,
    sway_phase: f32, sway_speed: f32,
}

struct WillowBranch {
    attach_y: f32,
    base_angle: f32,
    length: f32,
    segments: usize,
    leaves: Vec<WillowLeaf>,
}

struct WillowLeaf {
    x: f32, y: f32,
    vx: f32, vy: f32,
    sway: f32, sway_speed: f32,
    life: f32,
    char: char,
    color: (u8, u8, u8),
}

const WILLOW_COLORS: &[(u8, u8, u8)] = &[
    (0x6B, 0x8E, 0x23), (0x55, 0x6B, 0x2F), (0x8F, 0xBC, 0x8F),
    (0x9A, 0xCD, 0x32), (0x4A, 0x7C, 0x59), (0x7C, 0x9E, 0x4A),
];

const WILLOW_LEAF_CHARS: &[char] = &['🍃', '🍂', '🌿', '🍀', '➤', '▸', '▹', '➥'];

struct WillowTears {
    willows: Vec<WillowTree>,
    leaves: Vec<WillowLeaf>,
    time: f32,
    width: u16, height: u16,
}

impl WillowTears {
    fn new() -> Self { Self { willows: Vec::new(), leaves: Vec::new(), time: 0.0, width: 0, height: 0 } }
    fn spawn(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        self.willows.clear(); self.leaves.clear();
        let count = (width as f32 / 35.0).clamp(1.0, 3.0) as usize;
        let ground = height as f32 - 1.0;
        for _ in 0..count {
            let h = rng.gen_range(height as usize / 3..height as usize * 2 / 3);
            let mut branches = Vec::new();
            for b in 0..rng.gen_range(8..14) {
                let mut leaves = Vec::new();
                for _ in 0..rng.gen_range(6..12) {
                    leaves.push(WillowLeaf {
                        x: 0.0, y: 0.0, vx: 0.0, vy: 0.0,
                        sway: 0.0, sway_speed: 0.0, life: 0.0,
                        char: ' ', color: (0,0,0),
                    });
                }
                branches.push(WillowBranch {
                    attach_y: ground - h as f32 / 3.0 - b as f32 * 2.5,
                    base_angle: rng.gen_range(-0.4..0.4),
                    length: rng.gen_range(12.0..22.0),
                    segments: rng.gen_range(15..25),
                    leaves,
                });
            }
            self.willows.push(WillowTree {
                x: rng.gen_range(15.0..(width - 15) as f32), y: ground,
                trunk_h: h / 3, branches,
                sway_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                sway_speed: rng.gen_range(0.0004..0.0012),
            });
        }
    }
}

impl Theme for WillowTears {
    fn name(&self) -> &'static str { "willow-tears" }
    fn description(&self) -> &'static str { "垂柳の涙 - Weeping willow branches swaying, leaves falling like tears" }
    fn resize(&mut self, area: Rect) { self.width = area.width; self.height = area.height; let mut rng = new_rng(); self.spawn(area.width, area.height, &mut rng); }
    fn update(&mut self, dt: f32, rng: &mut ChaCha8Rng) {
        self.time += dt;
        for w in &mut self.willows {
            w.sway_phase += w.sway_speed * dt * 60.0;
            // Drop leaves from branch tips
            if rng.gen::<f32>() < 0.0015 * dt * 60.0 {
                let branch = w.branches.choose(rng).unwrap();
                self.leaves.push(WillowLeaf {
                    x: w.x + (w.sway_phase.sin() * branch.length * 0.6),
                    y: branch.attach_y - branch.length * 0.9,
                    vx: rng.gen_range(-0.25..0.25), vy: rng.gen_range(0.15..0.45),
                    sway: rng.gen_range(0.0..std::f32::consts::TAU),
                    sway_speed: rng.gen_range(0.02..0.05),
                    life: 0.0, char: *WILLOW_LEAF_CHARS.choose(rng).unwrap(),
                    color: *WILLOW_COLORS.choose(rng).unwrap(),
                });
            }
        }
        self.leaves.retain_mut(|l| {
            l.x += l.vx * dt * 60.0; l.y += l.vy * dt * 60.0;
            l.sway += l.sway_speed * dt * 60.0; l.life += dt;
            l.life < 22.0 && l.y < self.height as f32 + 3.0
        });
    }
    fn draw(&mut self, buf: &mut Buffer, area: Rect) {
        for w in &self.willows {
            let sway = (w.sway_phase.sin() * 1.8) as i32;
            let bx = w.x as i32 + sway;
            let by = w.y as i32;

            // Trunk
            for i in 0..w.trunk_h {
                let y = by - i as i32; let x = bx + sway * (i as i32) / w.trunk_h as i32;
                if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                    draw_char(buf, x, y, area, '│', WILLOW_COLORS[4]);
                }
            }

            // Weeping branches
            for branch in &w.branches {
                let branch_y = branch.attach_y as i32;
                for seg in 0..branch.segments {
                    let t = seg as f32 / branch.segments as f32;
                    let curve = (t * std::f32::consts::PI).sin();
                    let seg_x = bx + sway + (branch.base_angle.cos() * branch.length * t + curve * 3.0) as i32;
                    let seg_y = branch_y - (branch.length * t * 0.7) as i32;
                    let sway_add = (w.sway_phase.sin() * (branch.length * t * 0.3)) as i32;

                    if seg_x >= 0 && seg_x < area.width as i32 && seg_y >= 0 && seg_y < area.height as i32 {
                        let ch = if t < 0.3 { '·' } else if t < 0.7 { '╴' } else { '╶' };
                        draw_char(buf, seg_x + sway_add, seg_y, area, ch, WILLOW_COLORS[seg % WILLOW_COLORS.len()]);
                    }
                }
                // Leaves along branch
                for leaf in &branch.leaves {
                    let t = DRAW_RNG.with(|rng| rng.borrow_mut().gen::<f32>());
                    let lx = bx + sway + (branch.base_angle.cos() * branch.length * t + (t * std::f32::consts::PI).sin() * 3.0) as i32;
                    let ly = branch_y - (branch.length * t * 0.7) as i32;
                    if lx >= 0 && lx < area.width as i32 && ly >= 0 && ly < area.height as i32 && DRAW_RNG.with(|rng| rng.borrow_mut().gen::<f32>()) < 0.4 {
                        draw_char(buf, lx, ly, area, '🍃', WILLOW_COLORS[0]);
                    }
                }
            }
        }
        // Falling leaves (tears)
        for l in &self.leaves {
            let x = l.x as i32; let y = l.y as i32;
            if x >= 0 && x < area.width as i32 && y >= 0 && y < area.height as i32 {
                let alpha = 1.0 - (l.life / 22.0).min(1.0);
                let sway_x = (l.sway.sin() * 0.6) as i32;
                draw_char(buf, x + sway_x, y, area, l.char, ((l.color.0 as f32 * alpha) as u8, (l.color.1 as f32 * alpha) as u8, (l.color.2 as f32 * alpha) as u8));
            }
        }
    }
}