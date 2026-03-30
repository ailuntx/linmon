use ratatui::style::Color;
use serde::{Deserialize, Serialize};

const COLORS_OPTIONS: [Color; 7] = [
    Color::Green,
    Color::Yellow,
    Color::Red,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Reset,
];

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ViewType {
    Sparkline,
    Gauge,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_view_type")]
    pub view_type: ViewType,

    #[serde(default = "default_color")]
    pub color: Color,

    #[serde(default = "default_interval")]
    pub interval: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            view_type: default_view_type(),
            color: default_color(),
            interval: default_interval(),
        }
    }
}

fn default_view_type() -> ViewType {
    ViewType::Sparkline
}

fn default_color() -> Color {
    COLORS_OPTIONS[0]
}

fn default_interval() -> u32 {
    1000
}

impl Config {
    fn get_config_path() -> Option<std::path::PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let path = std::path::PathBuf::from(xdg)
                .join("linmon")
                .join("config.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return Some(path);
        }

        if let Ok(home) = std::env::var("HOME") {
            let path = std::path::PathBuf::from(home)
                .join(".config")
                .join("linmon")
                .join("config.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return Some(path);
        }

        None
    }

    pub fn load() -> Self {
        let Some(path) = Self::get_config_path() else {
            return Self::default();
        };

        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return Self::default(),
        };

        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::get_config_path() else {
            return;
        };

        let file = match std::fs::File::create(path) {
            Ok(file) => file,
            Err(_) => return,
        };

        let writer = std::io::BufWriter::new(file);
        let _ = serde_json::to_writer_pretty(writer, self);
    }

    pub fn next_color(&mut self) {
        self.color = match COLORS_OPTIONS.iter().position(|&c| c == self.color) {
            Some(idx) => COLORS_OPTIONS[(idx + 1) % COLORS_OPTIONS.len()],
            None => COLORS_OPTIONS[0],
        };
        self.save();
    }

    pub fn next_view_type(&mut self) {
        self.view_type = match self.view_type {
            ViewType::Sparkline => ViewType::Gauge,
            ViewType::Gauge => ViewType::Sparkline,
        };
        self.save();
    }

    pub fn dec_interval(&mut self) {
        let step = 250;
        self.interval = (self.interval.saturating_sub(step).div_ceil(step) * step).max(250);
        self.save();
    }

    pub fn inc_interval(&mut self) {
        let step = 250;
        self.interval = (self.interval.saturating_add(step) / step * step).min(10_000);
        self.save();
    }
}
