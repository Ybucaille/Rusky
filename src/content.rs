use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Cartes livrées avec l'app (compilées dans le binaire), un fichier par niveau.
const BUILTIN: &[&str] = &[
    include_str!("../data/niveau-1.toml"),
    include_str!("../data/niveau-1-alphabet.toml"),
    include_str!("../data/niveau-2.toml"),
    include_str!("../data/niveau-3.toml"),
];

#[derive(Debug, Deserialize)]
pub struct Deck {
    /// Niveau par défaut des cartes du fichier.
    #[serde(default)]
    pub tier: Option<i64>,
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
    /// Niveau (1 = débutant) ; sinon celui du fichier, sinon 1.
    #[serde(default)]
    pub tier: Option<i64>,
    /// Ordre d'apparition des nouvelles cartes dans le niveau (plus petit = plus tôt).
    #[serde(default = "default_level")]
    pub level: i64,
}

fn default_level() -> i64 {
    5
}

pub fn builtin() -> Result<Deck> {
    let mut all = Vec::new();
    for (i, text) in BUILTIN.iter().enumerate() {
        let deck = parse(text).with_context(|| format!("cartes intégrées invalides (fichier n° {})", i + 1))?;
        all.extend(deck.card);
    }
    Ok(Deck { tier: None, card: all })
}

pub fn from_file(path: &Path) -> Result<Deck> {
    let text = std::fs::read_to_string(path)?;
    parse(&text).with_context(|| format!("fichier invalide : {}", path.display()))
}

/// Lit un paquet et complète niveau et prononciation manquants.
fn parse(text: &str) -> Result<Deck> {
    let mut deck: Deck = toml::from_str(text)?;
    for c in &mut deck.card {
        c.tier = Some(c.tier.or(deck.tier).unwrap_or(1));
        // Prononciation calculée si absente (pas pour l'alphabet : c'est la réponse).
        if c.pron.is_none() && c.kind != "alphabet" {
            c.pron = Some(crate::pron::pron(&c.front));
        }
    }
    Ok(deck)
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
