//! XP et niveaux : un niveau se débloque quand assez de cartes du précédent sont acquises.

use crate::db::Db;
use anyhow::Result;

pub const XP_GOOD: i64 = 10;
/// Une erreur rapporte un peu : l'effort compte.
pub const XP_MISS: i64 = 2;
/// Part des cartes d'un niveau à avoir acquises pour ouvrir le suivant.
pub const UNLOCK_SHARE: f64 = 0.7;
/// Une carte est acquise après ce nombre de bonnes réponses d'affilée.
pub const ACQUIRED_REPS: i64 = 2;

const NAMES: [&str; 6] = [
    "Premiers pas",
    "Le quotidien",
    "En ville",
    "Raconter",
    "Discuter",
    "Nuances",
];

pub fn name(tier: i64) -> &'static str {
    NAMES.get((tier - 1).max(0) as usize).copied().unwrap_or("Expert")
}

pub struct Level {
    pub tier: i64,
    pub max_tier: i64,
    pub xp: i64,
    pub acquired: i64,
    pub total: i64,
}

impl Level {
    /// Avancement vers le niveau suivant, de 0 à 1.
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        (self.acquired as f64 / (self.total as f64 * UNLOCK_SHARE)).min(1.0)
    }
}

pub fn level(db: &Db) -> Result<Level> {
    let tier = db.unlocked_tier()?;
    let (acquired, total) = db.tier_counts(tier, ACQUIRED_REPS)?;
    Ok(Level {
        tier,
        max_tier: db.max_tier()?,
        xp: db.xp()?,
        acquired,
        total,
    })
}

/// Ajoute l'XP d'une réponse. Renvoie le niveau qui vient de s'ouvrir, s'il y en a un.
pub fn record(db: &Db, good: bool) -> Result<Option<i64>> {
    db.add_xp(if good { XP_GOOD } else { XP_MISS })?;
    let l = level(db)?;
    if l.tier < l.max_tier && l.progress() >= 1.0 {
        db.set_unlocked_tier(l.tier + 1)?;
        return Ok(Some(l.tier + 1));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::CardDef;
    use crate::srs::Grade;
    use std::path::Path;

    fn def(id: &str, tier: i64) -> CardDef {
        CardDef {
            id: id.into(),
            kind: "vocab".into(),
            front: id.into(),
            pron: None,
            question: None,
            back: id.into(),
            wrong: vec![],
            note: None,
            tier: Some(tier),
            level: 1,
        }
    }

    #[test]
    fn next_tier_opens_at_seventy_percent() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        let mut cards: Vec<CardDef> = (0..10).map(|i| def(&format!("a{i}"), 1)).collect();
        cards.push(def("b0", 2));
        db.upsert_all(&cards).unwrap();
        let kinds = vec![];

        // Le niveau 2 reste fermé tant que 7 cartes sur 10 ne sont pas acquises.
        assert_eq!(db.next_card(0, 0, &kinds, 100, 1).unwrap().unwrap().tier, 1);
        for i in 0..7 {
            let mut c = db.card(&format!("a{i}")).unwrap();
            c.reps = ACQUIRED_REPS;
            db.save_review(&c, Grade::Good, 0).unwrap();
            let opened = record(&db, true).unwrap();
            assert_eq!(opened, if i == 6 { Some(2) } else { None }, "carte {i}");
        }
        // Les cartes du niveau 2 peuvent maintenant sortir.
        let fresh = db.next_card(0, 0, &kinds, 100, db.unlocked_tier().unwrap()).unwrap().unwrap();
        assert!(fresh.tier <= 2);
        assert_eq!(db.unlocked_tier().unwrap(), 2);
        assert_eq!(db.xp().unwrap(), 7 * XP_GOOD);
    }
}
