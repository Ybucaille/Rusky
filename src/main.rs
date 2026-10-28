mod config;
mod content;
mod db;
mod notify;
mod srs;

use anyhow::{Result, bail};
use chrono::{Local, TimeZone, Timelike};
use clap::{Parser, Subcommand};
use db::{Card, Db};
use notify::Outcome;
use srs::Grade;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "rusky", version, about = "Apprends le russe par petites notifications")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lance le démon : une popup toutes les N minutes
    Run,
    /// Affiche une carte tout de suite
    Now,
    /// Session de révision dans le terminal
    Review {
        /// Nombre de cartes
        #[arg(short, long, default_value_t = 10)]
        count: usize,
    },
    /// Affiche ta progression
    Stats,
    /// Ajoute ta propre carte
    Add {
        /// Russe (ou titre de la règle)
        front: String,
        /// Traduction (ou explication)
        back: String,
        /// alphabet | vocab | grammar | phrase
        #[arg(short, long, default_value = "vocab")]
        kind: String,
        /// Exemple ou remarque
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Importe des cartes depuis un fichier TOML
    Import { file: PathBuf },
    /// Met les popups en pause (en minutes)
    Pause { minutes: u64 },
    /// Relance les popups
    Resume,
    /// Montre où sont la config et la base
    Paths,
}

fn now() -> i64 {
    Local::now().timestamp()
}

fn day_start() -> i64 {
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or_else(now)
}

fn open_db() -> Result<Db> {
    let mut db = Db::open(&config::db_path()?)?;
    // Synchronise les cartes intégrées (nouvelles cartes, corrections de texte).
    db.upsert_all(&content::builtin()?.card)?;
    Ok(db)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run => run(),
        Cmd::Now => {
            let cfg = config::load()?;
            let db = open_db()?;
            if !show_one(&db, &cfg)? {
                notify::info("Rusky", "Rien à réviser pour l'instant. Reviens plus tard !");
            }
            Ok(())
        }
        Cmd::Review { count } => review(count),
        Cmd::Stats => stats(),
        Cmd::Add { front, back, kind, note } => {
            if !["alphabet", "vocab", "grammar", "phrase"].contains(&kind.as_str()) {
                bail!("type inconnu : {kind} (alphabet, vocab, grammar ou phrase)");
            }
            let db = open_db()?;
            let id = format!("perso-{}", Local::now().timestamp_millis());
            db.upsert(&content::CardDef { id, kind, front, back, note, level: 0 })?;
            println!("Carte ajoutée ✓ Elle sortira parmi les prochaines nouvelles cartes.");
            Ok(())
        }
        Cmd::Import { file } => {
            let mut db = open_db()?;
            let deck = content::from_file(&file)?;
            db.upsert_all(&deck.card)?;
            println!("{} cartes importées ✓", deck.card.len());
            Ok(())
        }
        Cmd::Pause { minutes } => {
            let db = open_db()?;
            let until = now() + (minutes as i64) * 60;
            db.set_meta("paused_until", &until.to_string())?;
            let t = Local.timestamp_opt(until, 0).unwrap();
            println!("Pause jusqu'à {} ⏸", t.format("%H:%M"));
            Ok(())
        }
        Cmd::Resume => {
            open_db()?.set_meta("paused_until", "0")?;
            println!("C'est reparti ▶");
            Ok(())
        }
        Cmd::Paths => {
            println!("Config : {}", config::config_path()?.display());
            println!("Base   : {}", config::db_path()?.display());
            Ok(())
        }
    }
}

/// Montre une carte en popup et enregistre la réponse. Renvoie false s'il n'y avait rien.
fn show_one(db: &Db, cfg: &config::Config) -> Result<bool> {
    let t = now();
    let Some(mut card) = db.next_card(t, day_start(), &cfg.kinds, cfg.max_new_per_day)? else {
        return Ok(false);
    };
    match notify::present(&card, t)? {
        Outcome::Graded(g) => grade(db, &mut card, g)?,
        // Une carte déjà vue repoussée revient au tour suivant ; une nouvelle reste nouvelle.
        Outcome::Later => db.snooze(&card, now() + (cfg.interval_minutes as i64) * 60)?,
        Outcome::Dismissed => {}
    }
    Ok(true)
}

fn grade(db: &Db, card: &mut Card, g: Grade) -> Result<()> {
    let t = now();
    srs::schedule(card, g, t);
    db.save_review(card, g, t)
}

fn run() -> Result<()> {
    let db = open_db()?;
    eprintln!("rusky : démon lancé");
    loop {
        // Config relue à chaque tour : les modifs s'appliquent sans relancer.
        let cfg = config::load().unwrap_or_else(|e| {
            eprintln!("rusky : {e:#}, réglages par défaut utilisés");
            config::Config::default()
        });
        let interval = Duration::from_secs(cfg.interval_minutes.max(1) * 60);

        let paused = db.paused_until()?.is_some_and(|u| u > now());
        let active = cfg.in_active_hours(Local::now().hour());

        if !paused && active
            && let Err(e) = show_one(&db, &cfg) {
                // Pas de serveur de notifs (session pas encore prête…) : on réessaie plus tard.
                eprintln!("rusky : impossible d'afficher la popup : {e:#}");
                sleep(Duration::from_secs(60));
                continue;
            }
        sleep(interval);
    }
}

fn review(count: usize) -> Result<()> {
    let cfg = config::load()?;
    let db = open_db()?;
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut read = |prompt: &str| -> Result<Option<String>> {
        print!("{prompt}");
        io::stdout().flush()?;
        Ok(lines.next().transpose()?)
    };

    let mut done = 0;
    while done < count {
        let t = now();
        let Some(mut card) = db.next_card(t, day_start(), &cfg.kinds, cfg.max_new_per_day)? else {
            println!("\nPlus rien à réviser pour l'instant 🎉");
            break;
        };
        println!("\n── {} ──", card.kind);
        println!("  {}", card.front);
        if read("  [Entrée] pour voir la réponse ")?.is_none() {
            break;
        }
        println!("  → {}", card.back);
        if let Some(n) = &card.note {
            println!("    {n}");
        }
        let Some(ans) = read("  Tu savais ? [o]ui / [n]on / [q]uitter : ")? else {
            break;
        };
        match ans.trim().to_lowercase().as_str() {
            "q" => break,
            "n" => grade(&db, &mut card, Grade::Again)?,
            _ => grade(&db, &mut card, Grade::Good)?,
        }
        done += 1;
    }
    println!("{done} carte(s) révisée(s).");
    Ok(())
}

fn stats() -> Result<()> {
    let db = open_db()?;
    let s = db.stats(now(), day_start())?;
    let rate = if s.reviewed_today > 0 {
        format!(" ({} % de réussite)", s.success_today * 100 / s.reviewed_today)
    } else {
        String::new()
    };
    println!("📚 Cartes        : {}", s.total);
    println!("🆕 Jamais vues   : {}", s.unseen);
    println!("🌱 En cours      : {}", s.learning);
    println!("🌳 Acquises      : {}", s.mature);
    println!("⏰ À revoir      : {}", s.due_now);
    println!("✅ Aujourd'hui   : {}{}", s.reviewed_today, rate);
    println!("🔥 Série         : {} jour(s)", s.streak_days);
    if let Some(u) = db.paused_until()?.filter(|&u| u > now()) {
        println!("⏸  En pause jusqu'à {}", Local.timestamp_opt(u, 0).unwrap().format("%H:%M"));
    }
    Ok(())
}
