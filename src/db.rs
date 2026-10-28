use crate::content::CardDef;
use crate::srs::Grade;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Card {
    pub id: String,
    pub kind: String,
    pub front: String,
    pub back: String,
    pub note: Option<String>,
    pub ease: f64,
    /// Intervalle courant en secondes.
    pub interval: i64,
    pub reps: i64,
    pub lapses: i64,
    /// Prochaine révision (timestamp Unix). None = carte jamais vue.
    pub due: Option<i64>,
}

pub struct Stats {
    pub total: i64,
    pub unseen: i64,
    pub learning: i64,
    pub mature: i64,
    pub due_now: i64,
    pub reviewed_today: i64,
    pub success_today: i64,
    pub streak_days: i64,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS cards (
                id         TEXT PRIMARY KEY,
                kind       TEXT NOT NULL,
                front      TEXT NOT NULL,
                back       TEXT NOT NULL,
                note       TEXT,
                level      INTEGER NOT NULL DEFAULT 5,
                ease       REAL NOT NULL DEFAULT 2.5,
                interval   INTEGER NOT NULL DEFAULT 0,
                reps       INTEGER NOT NULL DEFAULT 0,
                lapses     INTEGER NOT NULL DEFAULT 0,
                due        INTEGER,
                first_seen INTEGER
            );
            CREATE TABLE IF NOT EXISTS log (
                ts      INTEGER NOT NULL,
                card_id TEXT NOT NULL,
                good    INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self { conn })
    }

    /// Ajoute ou met à jour le texte d'une carte, sans toucher à sa progression.
    pub fn upsert(&self, c: &CardDef) -> Result<()> {
        self.conn.execute(
            "INSERT INTO cards (id, kind, front, back, note, level)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind, front = excluded.front, back = excluded.back,
                note = excluded.note, level = excluded.level",
            params![c.id, c.kind, c.front, c.back, c.note, c.level],
        )?;
        Ok(())
    }

    pub fn upsert_all(&mut self, cards: &[CardDef]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for c in cards {
            tx.execute(
                "INSERT INTO cards (id, kind, front, back, note, level)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                    kind = excluded.kind, front = excluded.front, back = excluded.back,
                    note = excluded.note, level = excluded.level",
                params![c.id, c.kind, c.front, c.back, c.note, c.level],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Choisit la prochaine carte : d'abord celles à revoir, sinon une nouvelle
    /// (dans la limite quotidienne).
    pub fn next_card(
        &self,
        now: i64,
        day_start: i64,
        kinds: &[String],
        max_new_per_day: u32,
    ) -> Result<Option<Card>> {
        let allowed = |k: &str| kinds.is_empty() || kinds.iter().any(|x| x == k);

        let mut stmt = self.conn.prepare(
            "SELECT id, kind, front, back, note, ease, interval, reps, lapses, due
             FROM cards WHERE due IS NOT NULL AND due <= ?1 ORDER BY due",
        )?;
        let due: Vec<Card> = stmt
            .query_map([now], row_to_card)?
            .collect::<Result<_, _>>()?;
        if let Some(c) = due.into_iter().find(|c| allowed(&c.kind)) {
            return Ok(Some(c));
        }

        let new_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE first_seen >= ?1",
            [day_start],
            |r| r.get(0),
        )?;
        if new_today >= max_new_per_day as i64 {
            return Ok(None);
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, kind, front, back, note, ease, interval, reps, lapses, due
             FROM cards WHERE due IS NULL ORDER BY level, rowid",
        )?;
        let fresh: Vec<Card> = stmt
            .query_map([], row_to_card)?
            .collect::<Result<_, _>>()?;
        Ok(fresh.into_iter().find(|c| allowed(&c.kind)))
    }

    pub fn save_review(&self, card: &Card, grade: Grade, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE cards SET ease = ?2, interval = ?3, reps = ?4, lapses = ?5, due = ?6,
                first_seen = COALESCE(first_seen, ?7)
             WHERE id = ?1",
            params![card.id, card.ease, card.interval, card.reps, card.lapses, card.due, now],
        )?;
        self.conn.execute(
            "INSERT INTO log (ts, card_id, good) VALUES (?1, ?2, ?3)",
            params![now, card.id, (grade == Grade::Good) as i64],
        )?;
        Ok(())
    }

    /// Repousse une carte sans la noter (« Plus tard »).
    pub fn snooze(&self, card: &Card, until: i64) -> Result<()> {
        if card.due.is_some() {
            self.conn
                .execute("UPDATE cards SET due = ?2 WHERE id = ?1", params![card.id, until])?;
        }
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn paused_until(&self) -> Result<Option<i64>> {
        Ok(self.get_meta("paused_until")?.and_then(|v| v.parse().ok()))
    }

    pub fn stats(&self, now: i64, day_start: i64) -> Result<Stats> {
        let one = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Result<i64> {
            Ok(self.conn.query_row(sql, p, |r| r.get(0))?)
        };
        let total = one("SELECT COUNT(*) FROM cards", &[])?;
        let unseen = one("SELECT COUNT(*) FROM cards WHERE due IS NULL", &[])?;
        let learning = one(
            "SELECT COUNT(*) FROM cards WHERE due IS NOT NULL AND reps < 3",
            &[],
        )?;
        let mature = one("SELECT COUNT(*) FROM cards WHERE reps >= 3", &[])?;
        let due_now = one(
            "SELECT COUNT(*) FROM cards WHERE due IS NOT NULL AND due <= ?1",
            &[&now],
        )?;
        let reviewed_today = one("SELECT COUNT(*) FROM log WHERE ts >= ?1", &[&day_start])?;
        let success_today = one(
            "SELECT COUNT(*) FROM log WHERE ts >= ?1 AND good = 1",
            &[&day_start],
        )?;

        // Série : jours consécutifs (jusqu'à aujourd'hui ou hier) avec au moins une révision.
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT CAST(julianday(date(ts, 'unixepoch', 'localtime')) AS INTEGER) AS d
             FROM log ORDER BY d DESC",
        )?;
        let days: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let today: i64 = self.conn.query_row(
            "SELECT CAST(julianday(date(?1, 'unixepoch', 'localtime')) AS INTEGER)",
            [now],
            |r| r.get(0),
        )?;
        let mut streak = 0;
        if let Some(&first) = days.first()
            && today - first <= 1 {
                let mut expected = first;
                for d in days {
                    if d == expected {
                        streak += 1;
                        expected -= 1;
                    } else {
                        break;
                    }
                }
            }

        Ok(Stats {
            total,
            unseen,
            learning,
            mature,
            due_now,
            reviewed_today,
            success_today,
            streak_days: streak,
        })
    }
}

fn row_to_card(r: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        kind: r.get(1)?,
        front: r.get(2)?,
        back: r.get(3)?,
        note: r.get(4)?,
        ease: r.get(5)?,
        interval: r.get(6)?,
        reps: r.get(7)?,
        lapses: r.get(8)?,
        due: r.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srs::{self, Grade};

    fn def(id: &str, kind: &str, level: i64) -> CardDef {
        CardDef {
            id: id.into(),
            kind: kind.into(),
            front: "f".into(),
            back: "b".into(),
            note: None,
            level,
        }
    }

    #[test]
    fn picks_due_before_new_and_respects_daily_limit() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_all(&[def("a", "vocab", 2), def("b", "vocab", 1), def("c", "grammar", 1)])
            .unwrap();
        let kinds = vec!["vocab".to_string()];

        let mut c = db.next_card(1000, 0, &kinds, 1).unwrap().unwrap();
        assert_eq!(c.id, "b"); // plus petit niveau, type autorisé
        srs::schedule(&mut c, Grade::Again, 1000);
        db.save_review(&c, Grade::Again, 1000).unwrap();

        // Limite de 1 nouvelle carte atteinte, rien n'est encore dû.
        assert!(db.next_card(1001, 0, &kinds, 1).unwrap().is_none());
        // Dix minutes plus tard, « b » revient.
        assert_eq!(db.next_card(1000 + 600, 0, &kinds, 1).unwrap().unwrap().id, "b");
        // Le texte peut être mis à jour sans perdre la progression.
        db.upsert(&def("b", "vocab", 1)).unwrap();
        assert_eq!(db.next_card(1000 + 600, 0, &kinds, 1).unwrap().unwrap().lapses, 0);
        let s = db.stats(2000, 0).unwrap();
        assert_eq!((s.total, s.unseen, s.reviewed_today), (3, 2, 1));
    }
}
