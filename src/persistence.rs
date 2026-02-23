use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::message::LineNumbers;
use crate::undo_tree::UndoTree;

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub vim_enabled: bool,
    pub line_numbers: LineNumbers,
    pub word_wrap: bool,
    pub scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { vim_enabled: false, line_numbers: LineNumbers::None, word_wrap: true, scale: 1.0 }
    }
}

fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("f4"))
}

fn settings_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

fn undo_path(file_path: &Path) -> Option<PathBuf> {
    config_dir().map(|d| d.join("undo").join(format!("{:016x}.json", path_hash(file_path))))
}

fn path_hash(path: &Path) -> u64 {
    let s = path.to_string_lossy();
    let mut h: u64 = 14695981039346656037;
    for &b in s.as_bytes() {
        h = h.wrapping_mul(1099511628211);
        h ^= b as u64;
    }
    h
}

pub fn load_settings() -> Settings {
    let path = match settings_path() { Some(p) => p, None => return Settings::default() };
    let bytes = match std::fs::read(&path) { Ok(b) => b, Err(_) => return Settings::default() };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    let path = match settings_path() { Some(p) => p, None => return };
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_vec_pretty(settings) { let _ = std::fs::write(&path, json); }
}

pub fn load_undo_tree(file_path: &Path) -> Option<UndoTree> {
    let path = undo_path(file_path)?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_undo_tree(file_path: &Path, tree: &UndoTree) {
    let path = match undo_path(file_path) { Some(p) => p, None => return };
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_vec(tree) { let _ = std::fs::write(&path, json); }
}
