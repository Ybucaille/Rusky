//! Transforme une carte en question à choix multiple (popup et terminal).

use crate::db::{Card, Db};
use anyhow::Result;

pub struct Quiz {
    /// « 🔤 Alphabet · nouveau »
    pub header: String,
    /// Le texte en gros (russe, ou français si la question est à l'envers).
    pub big: String,
    /// Prononciation sous le texte en gros.
    pub pron: Option<String>,
    /// La consigne.
    pub prompt: String,
    pub choices: Vec<String>,
    pub correct: usize,
    /// Explication montrée après la réponse.
    pub note: Option<String>,
}

/// Mot russe suivi de sa prononciation : « кот (kot) ».
pub fn with_pron(front: &str, pron: Option<&str>) -> String {
    match pron {
        Some(p) if !p.is_empty() => format!("{front} ({p})"),
        _ => front.to_string(),
    }
}

pub fn build(db: &Db, card: &Card, now: i64) -> Result<Quiz> {
    // Une fois la carte un peu connue, on pose parfois la question à l'envers.
    let reverse = matches!(card.kind.as_str(), "vocab" | "phrase") && card.reps >= 2 && now % 2 == 0;

    let label = match card.kind.as_str() {
        "alphabet" => "🔤 Alphabet",
        "grammar" => "📘 Grammaire",
        "phrase" => "💬 Expression",
        _ => "🇷🇺 Vocabulaire",
    };
    let new_tag = if card.due.is_none() { " · nouveau" } else { "" };
    let label = format!("Niv. {} · {label}", card.tier);

    let (big, pron, prompt, answer) = if reverse {
        (
            card.back.clone(),
            None,
            "Comment dit-on ça en russe ?".to_string(),
            with_pron(&card.front, card.pron.as_deref()),
        )
    } else {
        let prompt = card.question.clone().unwrap_or_else(|| {
            match card.kind.as_str() {
                "alphabet" => "Comment se prononce cette lettre ?",
                "phrase" => "Que veut dire cette phrase ?",
                _ => "Que veut dire ce mot ?",
            }
            .to_string()
        });
        (card.front.clone(), card.pron.clone(), prompt, card.back.clone())
    };

    let mut choices = if card.wrong.is_empty() || reverse {
        db.distractors(card, reverse, 2)?
    } else {
        card.wrong.clone()
    };
    choices.push(answer.clone());
    shuffle(&mut choices, now as u64 ^ card.id.len() as u64);
    let correct = choices.iter().position(|c| *c == answer).unwrap_or(0);

    Ok(Quiz {
        header: format!("{label}{new_tag}"),
        big,
        pron: pron.filter(|p| !p.is_empty()),
        prompt,
        choices,
        correct,
        note: card.note.clone().filter(|n| !n.is_empty()),
    })
}

/// Mélange de Fisher-Yates avec un petit xorshift : pas besoin d'une crate pour ça.
fn shuffle<T>(v: &mut [T], seed: u64) {
    let mut x = seed | 1;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn three_distinct_choices_with_the_answer() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_all(&crate::content::builtin().unwrap().card).unwrap();
        for id in ["abc-001", "voc-010", "gram-003", "phr-001"] {
            let card = db.card(id).unwrap();
            for now in 0..20 {
                let q = build(&db, &card, now).unwrap();
                assert_eq!(q.choices.len(), 3, "{id}");
                assert_eq!(q.choices[q.correct], card.back, "{id}");
                let mut c = q.choices.clone();
                c.dedup();
                c.sort();
                c.dedup();
                assert_eq!(c.len(), 3, "{id} : {:?}", q.choices);
            }
        }
    }
}
