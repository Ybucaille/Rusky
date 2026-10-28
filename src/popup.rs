//! Popups GTK4 en layer-shell : une carte dans un coin de l'écran, par-dessus
//! les fenêtres, sans dépendre du démon de notifications.

use crate::db::Card;
use crate::srs::Grade;
use anyhow::{Result, bail};
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;

pub enum Outcome {
    Graded(Grade),
    /// « Plus tard » : on repousse sans noter.
    Later,
    /// Popup fermée sans réponse.
    Dismissed,
}

/// Présente une carte. Bloque jusqu'à ce que l'utilisateur réponde ou ferme.
pub fn present(card: &Card, now: i64) -> Result<Outcome> {
    let p = Prompt::for_card(card, now);

    if card.kind == "grammar" {
        let note = card
            .note
            .as_deref()
            .map(|n| format!("\n\n<i>{}</i>", esc(n)))
            .unwrap_or_default();
        let action = ask(
            &p.title,
            &card.front,
            &format!("{}{}", esc(&card.back), note),
            &[("good", "Compris ✓"), ("again", "À revoir ✗"), ("later", "Plus tard")],
        )?;
        return Ok(to_outcome(&action));
    }

    let first = ask(
        &p.title,
        &p.question,
        &format!("<i>{}</i>", p.hint),
        &[("reveal", "Révéler"), ("later", "Plus tard")],
    )?;
    match first.as_str() {
        "reveal" => {}
        "later" => return Ok(Outcome::Later),
        _ => return Ok(Outcome::Dismissed),
    }

    let second = ask(
        &p.title,
        &p.question,
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
            .map(|n| format!("\n\n<i>{}</i>", esc(n)))
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
                format!("<big><b>{}</b></big>{}", esc(&card.back), note),
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

const CSS: &str = r#"
window.rusky { background: transparent; }
.card {
  background: alpha(#1e1e2e, 0.96);
  color: #e6e6f0;
  border-radius: 18px;
  border: 1px solid alpha(#ffffff, 0.08);
  padding: 14px 18px 16px 18px;
  box-shadow: 0 6px 24px alpha(black, 0.45);
}
.header { font-size: 0.9em; opacity: 0.7; }
.close { min-width: 24px; min-height: 24px; padding: 0; border-radius: 12px;
         background: none; box-shadow: none; border: none; color: inherit; }
.close:hover { background: alpha(#ffffff, 0.1); }
.question { font-size: 30px; font-weight: 700; margin: 10px 0 4px 0; }
.body { font-size: 1.05em; opacity: 0.9; }
.actions button { border-radius: 10px; padding: 6px 14px; border: none;
                  background: alpha(#ffffff, 0.08); color: inherit; box-shadow: none; }
.actions button:hover { background: alpha(#ffffff, 0.16); }
.actions button.primary { background: #cba6f7; color: #1e1e2e; font-weight: 600; }
.actions button.primary:hover { background: #d9bdfa; }
"#;

fn init() -> Result<()> {
    thread_local!(static READY: RefCell<bool> = const { RefCell::new(false) });
    if READY.with(|r| *r.borrow()) {
        return Ok(());
    }
    gtk::init()?;
    if !gtk4_layer_shell::is_supported() {
        bail!("le compositeur ne gère pas wlr-layer-shell");
    }
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    let display = gtk::gdk::Display::default().ok_or_else(|| anyhow::anyhow!("pas d'écran"))?;
    gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    READY.with(|r| *r.borrow_mut() = true);
    Ok(())
}

/// Construit la popup (en haut à droite) et renvoie la fenêtre et sa zone de boutons.
fn popup(header: &str, question: &str, body: &str) -> (gtk::Window, gtk::Box, gtk::Button) {
    let win = gtk::Window::builder().css_classes(["rusky"]).resizable(false).build();
    win.init_layer_shell();
    win.set_namespace(Some("rusky"));
    win.set_layer(Layer::Overlay);
    win.set_keyboard_mode(KeyboardMode::OnDemand);
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Right, true);
    win.set_margin(Edge::Top, 16);
    win.set_margin(Edge::Right, 16);

    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["card"])
        .width_request(340)
        .build();

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let head = gtk::Label::builder().label(header).xalign(0.0).hexpand(true).css_classes(["header"]).build();
    let close = gtk::Button::builder().label("✕").css_classes(["close"]).build();
    top.append(&head);
    top.append(&close);
    card.append(&top);

    card.append(&gtk::Label::builder().label(question).wrap(true).justify(gtk::Justification::Center).css_classes(["question"]).build());
    if !body.is_empty() {
        let b = gtk::Label::builder().wrap(true).max_width_chars(36).justify(gtk::Justification::Center).css_classes(["body"]).build();
        b.set_markup(body);
        card.append(&b);
    }

    let actions = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::Center)
        .margin_top(14)
        .css_classes(["actions"])
        .build();
    card.append(&actions);
    win.set_child(Some(&card));
    (win, actions, close)
}

fn ask(header: &str, question: &str, body: &str, actions: &[(&str, &str)]) -> Result<String> {
    init()?;
    let (win, row, close) = popup(header, question, body);
    let chosen = Rc::new(RefCell::new(String::from("__closed")));
    let main = gtk::glib::MainLoop::new(None, false);

    for (i, (id, label)) in actions.iter().enumerate() {
        let b = gtk::Button::with_label(label);
        if i == 0 {
            b.add_css_class("primary");
        }
        let (id, chosen, main) = (id.to_string(), chosen.clone(), main.clone());
        b.connect_clicked(move |_| {
            *chosen.borrow_mut() = id.clone();
            main.quit();
        });
        row.append(&b);
    }
    let m = main.clone();
    close.connect_clicked(move |_| m.quit());
    let m = main.clone();
    win.connect_close_request(move |_| {
        m.quit();
        gtk::glib::Propagation::Proceed
    });
    // Échap ferme la popup quand elle a le focus.
    let keys = gtk::EventControllerKey::new();
    let m = main.clone();
    keys.connect_key_pressed(move |_, k, _, _| {
        if k == gtk::gdk::Key::Escape {
            m.quit();
        }
        gtk::glib::Propagation::Proceed
    });
    win.add_controller(keys);

    win.present();
    main.run();
    win.destroy();
    // Laisse GTK traiter la fermeture avant la popup suivante.
    while gtk::glib::MainContext::default().iteration(false) {}
    Ok(chosen.take())
}

fn to_outcome(action: &str) -> Outcome {
    match action {
        "good" => Outcome::Graded(Grade::Good),
        "again" => Outcome::Graded(Grade::Again),
        "later" => Outcome::Later,
        _ => Outcome::Dismissed,
    }
}

/// Échappe le texte pour le balisage Pango.
fn esc(s: &str) -> String {
    gtk::glib::markup_escape_text(s).to_string()
}

/// Petite popup d'info (pause, erreurs…), refermée au bout de 5 s.
pub fn info(summary: &str, body: &str) {
    if init().is_err() {
        eprintln!("{summary} : {body}");
        return;
    }
    let (win, _, close) = popup("Rusky", summary, &esc(body));
    let main = gtk::glib::MainLoop::new(None, false);
    let m = main.clone();
    close.connect_clicked(move |_| m.quit());
    let m = main.clone();
    gtk::glib::timeout_add_seconds_local_once(5, move || m.quit());
    win.present();
    main.run();
    win.destroy();
    while gtk::glib::MainContext::default().iteration(false) {}
}
