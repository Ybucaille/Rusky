use crate::content::CardDef;
use crate::srs::Grade;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Card {
    pub id: String,
    pub kind: String,
    pub tier: i64,
    pub front: String,
    pub pron: Option<String>,
    pub question: Option<String>,
    pub back: String,
    pub wrong: Vec<String>,
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

/// Sessions lancées depuis la fenêtre de révision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Session {
    /// Cartes dues, puis bientôt dues.
    Review,
    /// Cartes déjà ratées au moins une fois.
    Mistakes,
    /// Cartes jamais vues des niveaux débloqués.
    New,
}

const CARD_COLUMNS: &str =
    "id, kind, front, back, note, ease, interval, reps, lapses, due, pron, question, wrong, tier";

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
        // Colonnes ajoutées après coup : on les crée sur les anciennes bases.
        for col in ["pron TEXT", "question TEXT", "wrong TEXT", "tier INTEGER NOT NULL DEFAULT 1"] {
            let _ = conn.execute(&format!("ALTER TABLE cards ADD COLUMN {col}"), []);
        }
        Ok(Self { conn })
    }

    /// Ajoute ou met à jour le texte d'une carte, sans toucher à sa progression.
    pub fn upsert(&self, c: &CardDef) -> Result<()> {
        upsert(&self.conn, c)
    }

    pub fn upsert_all(&mut self, cards: &[CardDef]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for c in cards {
            upsert(&tx, c)?;
        }
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub fn card(&self, id: &str) -> Result<Card> {
        Ok(self.conn.query_row(
            &format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = ?1"),
            [id],
            row_to_card,
        )?)
    }

    /// Mauvaises réponses tirées au hasard parmi les autres cartes du même type.
    pub fn distractors(&self, card: &Card, reverse: bool, n: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT front, pron, back FROM cards
             WHERE kind = ?1 AND id <> ?2 AND back <> ?3 AND front <> ?4 AND tier BETWEEN ?6 - 1 AND ?6 + 1
             ORDER BY random() LIMIT ?5",
        )?;
        // Mauvaises réponses prises aux niveaux voisins : ni trop faciles, ni des mots jamais vus.
        // On en tire davantage pour écarter celles qui ressemblent à la bonne (synonymes, sens partagé).
        let p = params![card.kind, card.id, card.back, card.front, (n * 8) as i64, card.tier];
        let rows = stmt.query_map(p, |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)))?;
        let rows: Vec<(String, Option<String>, String)> = rows.collect::<Result<_, _>>()?;
        let shown = |(front, pron, back): &(String, Option<String>, String)| {
            if reverse { crate::quiz::with_pron(front, pron.as_deref()) } else { back.clone() }
        };
        let mut chosen: Vec<String> = Vec::new();
        let mut meanings = vec![card.back.clone()];
        for row in &rows {
            if chosen.len() < n && !meanings.iter().any(|m| overlaps(m, &row.2)) {
                meanings.push(row.2.clone());
                chosen.push(shown(row));
            }
        }
        // Rare : pas assez de sens bien distincts ; on complète sans jamais répéter la bonne réponse.
        for row in &rows {
            if chosen.len() < n && !overlaps(&card.back, &row.2) && !chosen.contains(&shown(row)) {
                chosen.push(shown(row));
            }
        }
        Ok(chosen)
    }

    /// Choisit la prochaine carte : d'abord celles à revoir, sinon une nouvelle
    /// (dans la limite quotidienne).
    pub fn next_card(
        &self,
        now: i64,
        day_start: i64,
        kinds: &[String],
        max_new_per_day: u32,
        max_tier: i64,
    ) -> Result<Option<Card>> {
        let allowed = |k: &str| kinds.is_empty() || kinds.iter().any(|x| x == k);

        let mut stmt = self.conn.prepare(
            &format!("SELECT {CARD_COLUMNS} FROM cards WHERE due IS NOT NULL AND due <= ?1 ORDER BY due"),
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
            &format!("SELECT {CARD_COLUMNS} FROM cards WHERE due IS NULL AND tier <= ?1 ORDER BY tier, level, rowid"),
        )?;
        let fresh: Vec<Card> = stmt
            .query_map([max_tier], row_to_card)?
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

    pub fn xp(&self) -> Result<i64> {
        Ok(self.get_meta("xp")?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    pub fn add_xp(&self, n: i64) -> Result<()> {
        self.set_meta("xp", &(self.xp()? + n).to_string())
    }

    pub fn unlocked_tier(&self) -> Result<i64> {
        Ok(self.get_meta("tier")?.and_then(|v| v.parse().ok()).unwrap_or(1))
    }

    pub fn set_unlocked_tier(&self, tier: i64) -> Result<()> {
        self.set_meta("tier", &tier.to_string())
    }

    pub fn max_tier(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(MAX(tier), 1) FROM cards", [], |r| r.get(0))?)
    }

    /// (cartes acquises, cartes au total) d'un niveau.
    pub fn tier_counts(&self, tier: i64, acquired_reps: i64) -> Result<(i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT COUNT(CASE WHEN reps >= ?2 THEN 1 END), COUNT(*) FROM cards WHERE tier = ?1",
            params![tier, acquired_reps],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
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
        let today = self.julian_day(now)?;
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

    /// Révisions par jour sur les `days` derniers jours (le plus ancien d'abord) : (total, réussies).
    pub fn daily(&self, now: i64, days: i64) -> Result<Vec<(i64, i64)>> {
        let today = self.julian_day(now)?;
        let mut out = vec![(0, 0); days as usize];
        let mut stmt = self.conn.prepare(
            "SELECT CAST(julianday(date(ts, 'unixepoch', 'localtime')) AS INTEGER) AS d,
                    COUNT(*), SUM(good)
             FROM log WHERE ts >= ?1 GROUP BY d",
        )?;
        let rows = stmt.query_map([now - days * 86_400 - 86_400], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (d, total, good) = row?;
            let i = days - 1 - (today - d);
            if (0..days).contains(&i) {
                out[i as usize] = (total, good);
            }
        }
        Ok(out)
    }

    /// (révisions, réussies) depuis `since`.
    pub fn accuracy_since(&self, since: i64) -> Result<(i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(good), 0) FROM log WHERE ts >= ?1",
            [since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    /// Réussite par type de carte : (type, révisions, réussies).
    pub fn kind_accuracy(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.kind, COUNT(*), SUM(l.good) FROM log l JOIN cards c ON c.id = l.card_id
             GROUP BY c.kind",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Par niveau : (niveau, cartes vues, cartes acquises, total).
    pub fn tier_overview(&self, acquired_reps: i64) -> Result<Vec<(i64, i64, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT tier, COUNT(due), COUNT(CASE WHEN reps >= ?1 THEN 1 END), COUNT(*)
             FROM cards GROUP BY tier ORDER BY tier",
        )?;
        let rows = stmt.query_map([acquired_reps], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Les cartes les plus souvent ratées.
    pub fn hardest(&self, n: usize) -> Result<Vec<Card>> {
        self.cards_where("lapses > 0 ORDER BY lapses DESC, due LIMIT ?1", params![n as i64])
    }

    /// Plus longue série de jours consécutifs avec au moins une révision.
    pub fn best_streak(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT CAST(julianday(date(ts, 'unixepoch', 'localtime')) AS INTEGER) AS d
             FROM log ORDER BY d",
        )?;
        let days: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let (mut best, mut cur, mut prev) = (0, 0, i64::MIN);
        for d in days {
            cur = if d == prev + 1 { cur + 1 } else { 1 };
            best = best.max(cur);
            prev = d;
        }
        Ok(best)
    }

    /// Cartes d'une session de révision lancée à la main.
    pub fn session_cards(&self, mode: Session, n: usize, now: i64, max_tier: i64) -> Result<Vec<Card>> {
        let n = n as i64;
        match mode {
            // D'abord ce qui est dû, puis ce qui le sera bientôt.
            Session::Review => self.cards_where(
                "due IS NOT NULL ORDER BY (due > ?1), due LIMIT ?2",
                params![now, n],
            ),
            // Dernière réponse fausse (« Pas su » remet reps à 0).
            Session::Mistakes => self.cards_where(
                "due IS NOT NULL AND reps = 0 ORDER BY lapses DESC, due LIMIT ?1",
                params![n],
            ),
            Session::New => self.cards_where(
                "due IS NULL AND tier <= ?1 ORDER BY tier, level, rowid LIMIT ?2",
                params![max_tier, n],
            ),
        }
    }

    /// Nombre de cartes pour chaque session : (dues, déjà vues, ratées, nouvelles).
    pub fn session_counts(&self, now: i64, max_tier: i64) -> Result<(i64, i64, i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT
                (SELECT COUNT(*) FROM cards WHERE due IS NOT NULL AND due <= ?1),
                (SELECT COUNT(*) FROM cards WHERE due IS NOT NULL),
                (SELECT COUNT(*) FROM cards WHERE due IS NOT NULL AND reps = 0),
                (SELECT COUNT(*) FROM cards WHERE due IS NULL AND tier <= ?2)",
            params![now, max_tier],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?)
    }

    /// La fenêtre de révision est ouverte : le démon saute son tour.
    pub fn set_busy_until(&self, until: i64) -> Result<()> {
        self.set_meta("busy_until", &until.to_string())
    }

    pub fn busy(&self, now: i64) -> Result<bool> {
        Ok(self.get_meta("busy_until")?.and_then(|v| v.parse::<i64>().ok()).is_some_and(|u| u > now))
    }

    fn cards_where(&self, clause: &str, p: &[&dyn rusqlite::ToSql]) -> Result<Vec<Card>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE {clause}"))?;
        let rows = stmt.query_map(p, row_to_card)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn julian_day(&self, ts: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT CAST(julianday(date(?1, 'unixepoch', 'localtime')) AS INTEGER)",
            [ts],
            |r| r.get(0),
        )?)
    }

}

/// Deux sens se recoupent-ils ? (même mot significatif : « s'il te plaît / de rien » et « de rien »)
fn overlaps(a: &str, b: &str) -> bool {
    const SKIP: [&str; 16] =
        ["les", "des", "une", "est", "pas", "que", "qui", "dans", "pour", "avec", "sur", "par", "aux", "son", "comme", "venu"];
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 3 && !SKIP.contains(w))
            .map(String::from)
            .collect()
    };
    let (wa, wb) = (words(a), words(b));
    a.trim().eq_ignore_ascii_case(b.trim()) || wa.iter().any(|w| wb.contains(w))
}

fn upsert(conn: &Connection, c: &CardDef) -> Result<()> {
    conn.execute(
        "INSERT INTO cards (id, kind, front, back, note, level, pron, question, wrong, tier)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            kind = excluded.kind, front = excluded.front, back = excluded.back,
            note = excluded.note, level = excluded.level, pron = excluded.pron,
            question = excluded.question, wrong = excluded.wrong, tier = excluded.tier",
        params![
            c.id, c.kind, c.front, c.back, c.note, c.level, c.pron, c.question,
            c.wrong.join("\n"), c.tier.unwrap_or(1)
        ],
    )?;
    Ok(())
}

fn row_to_card(r: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        kind: r.get(1)?,
        front: r.get(2)?,
        pron: r.get(10)?,
        question: r.get(11)?,
        back: r.get(3)?,
        wrong: r
            .get::<_, Option<String>>(12)?
            .map(|w| w.lines().map(String::from).collect())
            .unwrap_or_default(),
        note: r.get(4)?,
        ease: r.get(5)?,
        interval: r.get(6)?,
        reps: r.get(7)?,
        lapses: r.get(8)?,
        due: r.get(9)?,
        tier: r.get(13)?,
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
            pron: None,
            question: None,
            back: "b".into(),
            wrong: vec![],
            note: None,
            tier: None,
            level,
        }
    }

    #[test]
    fn picks_due_before_new_and_respects_daily_limit() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_all(&[def("a", "vocab", 2), def("b", "vocab", 1), def("c", "grammar", 1)])
            .unwrap();
        let kinds = vec!["vocab".to_string()];

        let mut c = db.next_card(1000, 0, &kinds, 1, 1).unwrap().unwrap();
        assert_eq!(c.id, "b"); // plus petit niveau, type autorisé
        srs::schedule(&mut c, Grade::Again, 1000);
        db.save_review(&c, Grade::Again, 1000).unwrap();

        // Limite de 1 nouvelle carte atteinte, rien n'est encore dû.
        assert!(db.next_card(1001, 0, &kinds, 1, 1).unwrap().is_none());
        // Dix minutes plus tard, « b » revient.
        assert_eq!(db.next_card(1000 + 600, 0, &kinds, 1, 1).unwrap().unwrap().id, "b");
        // Le texte peut être mis à jour sans perdre la progression.
        db.upsert(&def("b", "vocab", 1)).unwrap();
        assert_eq!(db.next_card(1000 + 600, 0, &kinds, 1, 1).unwrap().unwrap().lapses, 0);
        let s = db.stats(2000, 0).unwrap();
        assert_eq!((s.total, s.unseen, s.reviewed_today), (3, 2, 1));
    }

    #[test]
    fn mistakes_are_cards_last_answered_wrong() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_all(&[def("a", "vocab", 1), def("b", "vocab", 1), def("c", "vocab", 1)]).unwrap();
        let (mut a, mut b) = (db.card("a").unwrap(), db.card("b").unwrap());
        srs::schedule(&mut a, Grade::Again, 0);
        db.save_review(&a, Grade::Again, 0).unwrap();
        // b ratée puis réussie : ce n'est plus une erreur.
        srs::schedule(&mut b, Grade::Again, 0);
        srs::schedule(&mut b, Grade::Good, 1);
        db.save_review(&b, Grade::Good, 1).unwrap();
        let ids: Vec<String> = db.session_cards(Session::Mistakes, 10, 2, 1).unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["a"]);
        assert_eq!(db.session_counts(2, 1).unwrap(), (0, 2, 1, 1));
    }

    #[test]
    fn distractors_stay_near_the_card_level() {
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        let mut cards: Vec<CardDef> = (0..6).map(|i| def(&format!("x{i}"), "vocab", 1)).collect();
        for (i, c) in cards.iter_mut().enumerate() {
            c.back = ["chat", "chien", "lait", "pain", "eau", "sel"][i].to_string();
            c.front = format!("mot {i}");
            c.tier = Some(if i < 3 { 1 } else { 9 });
        }
        db.upsert_all(&cards).unwrap();
        let card = db.card("x0").unwrap();
        let wrong = db.distractors(&card, false, 5).unwrap();
        assert_eq!(wrong.len(), 2, "seules les cartes du niveau 1 ou 2 : {wrong:?}");
    }

    #[test]
    fn distractors_never_share_meaning_with_the_answer() {
        assert!(overlaps("s'il te plaît / de rien", "De rien"));
        assert!(overlaps("bien / d'accord", "d'accord"));
        assert!(!overlaps("lait", "pain"));
        let mut db = Db::open(Path::new(":memory:")).unwrap();
        db.upsert_all(&crate::content::builtin().unwrap().card).unwrap();
        for id in ["voc-003", "voc-005", "phr-021", "t2-voc-001"] {
            let card = db.card(id).unwrap();
            for _ in 0..30 {
                let wrong = db.distractors(&card, false, 2).unwrap();
                assert_eq!(wrong.len(), 2);
                assert!(wrong.iter().all(|w| !overlaps(&card.back, w)), "{id} : {wrong:?}");
                assert!(!overlaps(&wrong[0], &wrong[1]), "{id} : {wrong:?}");
            }
        }
    }
}

