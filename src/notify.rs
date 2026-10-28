//! Popups via D-Bus (spec freedesktop) : marche avec swaync, mako, dunst…

use crate::db::Card;
use crate::srs::Grade;
use anyhow::Result;
use notify_rust::{Notification, Timeout, Urgency};

pub enum Outcome {
    Graded(Grade),
    /// « Plus tard » : on repousse sans noter.
    Later,
    /// Notification fermée sans réponse.
    Dismissed,
}

/// Présente une carte. Bloque jusqu'à ce que l'utilisateur réponde ou ferme.
pub fn present(card: &Card, now: i64) -> Result<Outcome> {
    let p = Prompt::for_card(card, now);

    if card.kind == "grammar" {
        let note = card
            .note
            .as_deref()
            .map(|n| format!("\n<i>{}</i>", esc(n)))
            .unwrap_or_default();
        let action = ask(
            &p.title,
            &format!("<b>{}</b>\n{}{}", esc(&card.front), esc(&card.back), note),
            &[("good", "Compris ✓"), ("again", "À revoir ✗"), ("later", "Plus tard")],
        )?;
        return Ok(to_outcome(&action));
    }

    let first = ask(
        &p.title,
        &format!("<big><b>{}</b></big>\n<i>{}</i>", esc(&p.question), p.hint),
        &[("default", "Révéler"), ("reveal", "Révéler"), ("later", "Plus tard")],
    )?;
    match first.as_str() {
        "reveal" | "default" => {}
        "later" => return Ok(Outcome::Later),
        _ => return Ok(Outcome::Dismissed),
    }

    let second = ask(
        &esc(&p.question),
        &p.answer,
        &[("good", "Je savais ✓"), ("again", "Pas su ✗")],
    )?;
    Ok(to_outcome(&second))
}

struct Prompt {
    title: String,
    question: String,
    hint: &'static str,
    answer: String,
}

impl Prompt {
    fn for_card(card: &Card, now: i64) -> Self {
        let note = card
            .note
            .as_deref()
            .map(|n| format!("\n<i>{}</i>", esc(n)))
            .unwrap_or_default();
        // Une fois la carte un peu connue, on pose parfois la question à l'envers.
        let reverse = matches!(card.kind.as_str(), "vocab" | "phrase") && card.reps >= 2 && now % 2 == 0;

        let label = match card.kind.as_str() {
            "alphabet" => "🔤 Alphabet",
            "grammar" => "📘 Grammaire",
            "phrase" => "💬 Expression",
            _ => "🇷🇺 Vocabulaire",
        };
        let new_tag = if card.due.is_none() { " · nouveau" } else { "" };

        let (question, hint, answer) = if reverse {
            (
                card.back.clone(),
                "Comment on dit ça en russe ?",
                format!("<big><b>{}</b></big>{}", esc(&card.front), note),
            )
        } else {
            let hint = match card.kind.as_str() {
                "alphabet" => "Quel son ?",
                "grammar" => "",
                _ => "Ça veut dire quoi ?",
            };
            (
                card.front.clone(),
                hint,
                format!("<b>{}</b>{}", esc(&card.back), note),
            )
        };

        Self {
            title: format!("{label}{new_tag}"),
            question,
            hint,
            answer,
        }
    }
}

fn ask(summary: &str, body: &str, actions: &[(&str, &str)]) -> Result<String> {
    let mut n = Notification::new();
    n.appname("Rusky")
        .summary(summary)
        .body(body)
        .icon("accessories-dictionary")
        .urgency(Urgency::Normal)
        .timeout(Timeout::Never);
    for (id, label) in actions {
        n.action(id, label);
    }
    let handle = n.show()?;
    let mut chosen = String::from("__closed");
    handle.wait_for_action(|a| chosen = a.to_string());
    Ok(chosen)
}

fn to_outcome(action: &str) -> Outcome {
    match action {
        "good" => Outcome::Graded(Grade::Good),
        "again" => Outcome::Graded(Grade::Again),
        "later" => Outcome::Later,
        _ => Outcome::Dismissed,
    }
}

/// Échappe le texte pour le balisage des notifications.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Petite notif d'info (pause, erreurs…).
pub fn info(summary: &str, body: &str) {
    let _ = Notification::new()
        .appname("Rusky")
        .summary(summary)
        .body(body)
        .icon("accessories-dictionary")
        .timeout(Timeout::Milliseconds(5000))
        .show();
}
