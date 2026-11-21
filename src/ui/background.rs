use crate::ui::themes::ThemeRegistry;
use ratatui::layout::Rect;
use std::time::Instant;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn new_rng() -> ChaCha8Rng {
    ChaCha8Rng::from_rng(&mut rand::thread_rng()).unwrap()
}

pub struct Background {
    registry: ThemeRegistry,
    last_update: Instant,
    rng: ChaCha8Rng,
}

impl Background {
    pub fn new(size: Rect) -> Self {
        let mut bg = Self {
            registry: ThemeRegistry::new(),
            last_update: Instant::now(),
            rng: new_rng(),
        };
        bg.registry.resize_all(size);
        bg
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        let area = Rect::new(0, 0, width, height);
        self.registry.resize_all(area);
    }

    pub fn update(&mut self) {
        let dt = self.last_update.elapsed().as_secs_f32();
        self.last_update = Instant::now();
        
        self.registry.current_mut().update(dt, &mut self.rng);
    }

    pub fn draw(&mut self, buf: &mut ratatui::buffer::Buffer, area: Rect) {
        self.registry.current_mut().draw(buf, area);
    }

    // Theme management
    pub fn current_theme_name(&self) -> &'static str {
        self.registry.current_name()
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        self.registry.set_theme(name)
    }

    pub fn next_theme(&mut self) {
        self.registry.next_theme();
    }

    pub fn prev_theme(&mut self) {
        self.registry.prev_theme();
    }

    pub fn list_themes(&self) -> Vec<(&'static str, &'static str)> {
        self.registry.list_themes()
    }
}