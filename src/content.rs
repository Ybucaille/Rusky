use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Cartes livrées avec l'app (compilées dans le binaire).
const BUILTIN: &str = include_str!("../data/cards.toml");

#[derive(Debug, Deserialize)]
pub struct Deck {
    pub card: Vec<CardDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardDef {
    pub id: String,
    /// alphabet | vocab | grammar | phrase
    pub kind: String,
    pub front: String,
    /// Prononciation de `front`, à la française (syllabe accentuée en majuscules).
    #[serde(default)]
    pub pron: Option<String>,
    /// Consigne de la question (sinon une consigne par défaut selon le type).
    #[serde(default)]
    pub question: Option<String>,
    pub back: String,
    /// Mauvaises réponses proposées (sinon tirées des autres cartes du même type).
    #[serde(default)]
    pub wrong: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// Ordre d'apparition des nouvelles cartes (plus petit = plus tôt).
    #[serde(default = "default_level")]
    pub level: i64,
}

fn default_level() -> i64 {
    5
}

pub fn builtin() -> Result<Deck> {
    toml::from_str(BUILTIN).context("cartes intégrées invalides")
}

pub fn from_file(path: &Path) -> Result<Deck> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text).with_context(|| format!("fichier invalide : {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn builtin_deck_is_valid() {
        let deck = builtin().unwrap();
        assert!(deck.card.len() > 80);
        let mut ids = HashSet::new();
        for c in &deck.card {
            assert!(ids.insert(c.id.clone()), "id en double : {}", c.id);
            assert!(
                ["alphabet", "vocab", "grammar", "phrase"].contains(&c.kind.as_str()),
                "type inconnu : {}",
                c.kind
            );
            assert!(!c.front.trim().is_empty() && !c.back.trim().is_empty());
            assert!(!c.wrong.contains(&c.back), "la bonne réponse est aussi une mauvaise : {}", c.id);
        }
    }
}
