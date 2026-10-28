//! Répétition espacée, version simplifiée de SM-2 (l'algo d'origine d'Anki).

use crate::db::Card;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grade {
    /// Pas su : la carte revient vite.
    Again,
    /// Su : l'intervalle s'allonge.
    Good,
}

const MIN: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 86_400;

pub fn schedule(card: &mut Card, grade: Grade, now: i64) {
    match grade {
        Grade::Again => {
            if card.reps > 0 {
                card.lapses += 1;
            }
            card.reps = 0;
            card.ease = (card.ease - 0.2).max(1.3);
            card.interval = 10 * MIN;
        }
        Grade::Good => {
            card.reps += 1;
            card.interval = match card.reps {
                1 => 4 * HOUR,
                2 => DAY,
                3 => 3 * DAY,
                _ => ((card.interval as f64) * card.ease).round() as i64,
            };
        }
    }
    card.due = Some(now + card.interval);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            id: "t".into(),
            kind: "vocab".into(),
            tier: 1,
            front: "кот".into(),
            pron: Some("kot".into()),
            question: None,
            back: "chat".into(),
            wrong: vec![],
            note: None,
            ease: 2.5,
            interval: 0,
            reps: 0,
            lapses: 0,
            due: None,
        }
    }

    #[test]
    fn intervals_grow_then_reset() {
        let mut c = card();
        schedule(&mut c, Grade::Good, 0);
        assert_eq!(c.interval, 4 * HOUR);
        schedule(&mut c, Grade::Good, 0);
        schedule(&mut c, Grade::Good, 0);
        assert_eq!(c.interval, 3 * DAY);
        schedule(&mut c, Grade::Good, 0);
        assert_eq!(c.interval, (3.0 * DAY as f64 * 2.5) as i64);
        schedule(&mut c, Grade::Again, 100);
        assert_eq!((c.reps, c.lapses, c.due), (0, 1, Some(100 + 10 * MIN)));
        assert!((c.ease - 2.3).abs() < 1e-9);
    }
}
