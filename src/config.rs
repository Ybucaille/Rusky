use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Réglages lus dans ~/.config/rusky/config.toml (créé au premier lancement).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Minutes entre deux popups.
    pub interval_minutes: u64,
    /// Heure de début des popups (0-23).
    pub active_start: u32,
    /// Heure de fin des popups (0-23, exclue). Peut passer minuit (ex. 18 -> 2).
    pub active_end: u32,
    /// Nombre maximum de nouvelles cartes par jour.
    pub max_new_per_day: u32,
    /// Types de cartes actifs : alphabet, vocab, grammar, phrase.
    pub kinds: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_minutes: 25,
            active_start: 9,
            active_end: 23,
            max_new_per_day: 8,
            kinds: vec![
                "alphabet".into(),
                "vocab".into(),
                "grammar".into(),
                "phrase".into(),
            ],
        }
    }
}

impl Config {
    pub fn in_active_hours(&self, hour: u32) -> bool {
        let (s, e) = (self.active_start, self.active_end);
        if s == e {
            true
        } else if s < e {
            hour >= s && hour < e
        } else {
            hour >= s || hour < e
        }
    }
}

pub fn dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "rusky").context("impossible de trouver le dossier personnel")
}

pub fn config_path() -> Result<PathBuf> {
    Ok(dirs()?.config_dir().join("config.toml"))
}

pub fn db_path() -> Result<PathBuf> {
    let dir = dirs()?.data_dir().to_path_buf();
    fs::create_dir_all(&dir)?;
    Ok(dir.join("rusky.db"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let default = Config::default();
        let text = format!(
            "# Configuration de rusky — modifiée à chaud, pas besoin de relancer\n{}",
            toml::to_string_pretty(&default)?
        );
        fs::write(&path, text)?;
        return Ok(default);
    }
    let text = fs::read_to_string(&path)?;
    toml::from_str(&text).with_context(|| format!("config invalide : {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_hours_wrap() {
        let mut c = Config::default();
        assert!(c.in_active_hours(9) && !c.in_active_hours(23) && !c.in_active_hours(3));
        c.active_start = 18;
        c.active_end = 2;
        assert!(c.in_active_hours(23) && c.in_active_hours(1) && !c.in_active_hours(10));
    }
}
