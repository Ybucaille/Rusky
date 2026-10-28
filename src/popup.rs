//! Popups GTK4 en layer-shell : une carte dans un coin de l'écran, par-dessus
//! les fenêtres, sans dépendre du démon de notifications.

use crate::db::Card;
use crate::srs::Grade;
use anyhow::{Result, bail};
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

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
    let popup = Popup::new(&p.title)?;

    let outcome = if card.kind == "grammar" {
        let note = card
            .note
            .as_deref()
            .map(|n| format!("\n\n<i>{}</i>", esc(n)))
            .unwrap_or_default();
        let action = popup.ask(
            &card.front,
            &format!("{}{}", esc(&card.back), note),
            &[("good", "Compris"), ("again", "À revoir"), ("later", "Plus tard")],
        );
        to_outcome(&action)
    } else {
        let first = popup.ask(
            &p.question,
            &format!("<i>{}</i>", p.hint),
            &[("reveal", "Révéler"), ("later", "Plus tard")],
        );
        match first.as_str() {
            "reveal" => to_outcome(&popup.ask(
                &p.question,
                &p.answer,
                &[("good", "Je savais"), ("again", "Pas su")],
            )),
            "later" => Outcome::Later,
            _ => Outcome::Dismissed,
        }
    };
    popup.close();
    Ok(outcome)
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
  background: #18191c;
  color: #ececef;
  border-radius: 16px;
  border: 1px solid alpha(#ffffff, 0.07);
  padding: 14px 18px 18px 18px;
  margin: 28px 28px 44px 44px;  /* place pour l'ombre */
  box-shadow: 0 2px 6px alpha(black, 0.25), 0 12px 32px alpha(black, 0.35);
  opacity: 1;
  transform: none;
  transition: opacity 220ms ease-out, transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.card.hidden {
  opacity: 0;
  transform: translateX(40px);
  transition: opacity 160ms ease-in, transform 160ms ease-in;
}
.header { font-size: 11px; font-weight: 600; letter-spacing: 1px; color: #8b8d94; }
.close { min-width: 24px; min-height: 24px; padding: 0; border-radius: 8px;
         background: none; box-shadow: none; border: none; color: #8b8d94; }
.close:hover { background: alpha(#ffffff, 0.06); color: #ececef; }
.content { transition: opacity 150ms ease-in-out; }
.content.fading { opacity: 0; }
.question { font-size: 30px; font-weight: 700; margin: 12px 0 4px 0; }
.body { font-size: 14px; color: #b4b6bd; }
.actions { margin-top: 16px; }
.actions button { border-radius: 9px; padding: 7px 16px; border: 1px solid alpha(#ffffff, 0.06);
                  background: #26282d; color: #ececef; box-shadow: none; font-weight: 500;
                  transition: background 120ms; }
.actions button:hover { background: #2e3036; }
.actions button.primary { background: #7aa2f7; border-color: transparent; color: #0f1115; font-weight: 600; }
.actions button.primary:hover { background: #8fb2fa; }
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

/// Fait tourner la boucle GTK pendant `ms` (le temps d'une transition CSS).
fn pump(ms: u64) {
    let main = glib::MainLoop::new(None, false);
    let m = main.clone();
    glib::timeout_add_local_once(Duration::from_millis(ms), move || m.quit());
    main.run();
}

/// Une carte à l'écran (en haut à droite), réutilisée entre question et réponse.
struct Popup {
    win: gtk::Window,
    card: gtk::Box,
    content: gtk::Box,
    question: gtk::Label,
    body: gtk::Label,
    actions: gtk::Box,
    main: glib::MainLoop,
    chosen: Rc<RefCell<String>>,
}

impl Popup {
    fn new(header: &str) -> Result<Self> {
        init()?;
        let win = gtk::Window::builder().css_classes(["rusky"]).resizable(false).build();
        win.init_layer_shell();
        win.set_namespace(Some("rusky"));
        win.set_layer(Layer::Overlay);
        win.set_keyboard_mode(KeyboardMode::OnDemand);
        win.set_anchor(Edge::Top, true);
        win.set_anchor(Edge::Right, true);
        // La marge de la carte (ombre) complète ce décalage.
        win.set_margin(Edge::Top, 0);
        win.set_margin(Edge::Right, 0);

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["card", "hidden"])
            .width_request(412)  // 340 px de carte + marges de l'ombre
            .build();

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let head = gtk::Label::builder()
            .label(header.to_uppercase())
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["header"])
            .build();
        let close = gtk::Button::builder().icon_name("window-close-symbolic").css_classes(["close"]).build();
        top.append(&head);
        top.append(&close);
        card.append(&top);

        let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).css_classes(["content"]).build();
        let question = gtk::Label::builder().wrap(true).justify(gtk::Justification::Center).css_classes(["question"]).build();
        let body = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(36)
            .justify(gtk::Justification::Center)
            .css_classes(["body"])
            .build();
        let actions = gtk::Box::builder().spacing(8).halign(gtk::Align::Center).css_classes(["actions"]).build();
        content.append(&question);
        content.append(&body);
        content.append(&actions);
        card.append(&content);
        win.set_child(Some(&card));

        let main = glib::MainLoop::new(None, false);
        let chosen = Rc::new(RefCell::new(String::new()));
        let (m, c) = (main.clone(), chosen.clone());
        close.connect_clicked(move |_| {
            c.replace("__closed".into());
            m.quit();
        });
        let (m, c) = (main.clone(), chosen.clone());
        win.connect_close_request(move |_| {
            c.replace("__closed".into());
            m.quit();
            glib::Propagation::Stop
        });
        let keys = gtk::EventControllerKey::new();
        let (m, c) = (main.clone(), chosen.clone());
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gtk::gdk::Key::Escape {
                c.replace("__closed".into());
                m.quit();
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);

        // Seule la carte capte les clics : la marge transparente laisse passer.
        let c = card.clone();
        win.connect_map(move |w| {
            let c = c.clone();
            let w = w.clone();
            glib::idle_add_local_once(move || {
                if let (Some(b), Some(s)) = (c.compute_bounds(&w), w.surface()) {
                    let r = gtk::cairo::RectangleInt::new(
                        b.x() as i32, b.y() as i32, b.width() as i32, b.height() as i32,
                    );
                    s.set_input_region(Some(&gtk::cairo::Region::create_rectangle(&r)));
                }
                c.remove_css_class("hidden");
            });
        });

        Ok(Self { win, card, content, question, body, actions, main, chosen })
    }

    /// Affiche une question (fondu si la carte est déjà visible) et attend un bouton.
    fn ask(&self, question: &str, body: &str, actions: &[(&str, &str)]) -> String {
        let shown = self.win.is_visible();
        if shown {
            self.content.add_css_class("fading");
            pump(150);
        }
        self.question.set_label(question);
        self.body.set_markup(body);
        self.body.set_visible(!body.is_empty());
        while let Some(child) = self.actions.first_child() {
            self.actions.remove(&child);
        }
        for (i, (id, label)) in actions.iter().enumerate() {
            let b = gtk::Button::with_label(label);
            if i == 0 {
                b.add_css_class("primary");
            }
            let (id, c, m) = (id.to_string(), self.chosen.clone(), self.main.clone());
            b.connect_clicked(move |_| {
                c.replace(id.clone());
                m.quit();
            });
            self.actions.append(&b);
        }
        if shown {
            self.content.remove_css_class("fading");
        } else {
            self.win.present();
        }
        self.main.run();
        self.chosen.take()
    }

    /// Fait glisser la carte vers la droite puis détruit la fenêtre.
    fn close(self) {
        self.card.add_css_class("hidden");
        pump(170);
        self.win.destroy();
        while glib::MainContext::default().iteration(false) {}
    }

    /// Ferme toute seule au bout de `secs` secondes.
    fn wait(&self, secs: u32) {
        let m = self.main.clone();
        glib::timeout_add_seconds_local_once(secs, move || m.quit());
        self.win.present();
        self.main.run();
    }
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
    glib::markup_escape_text(s).to_string()
}

/// Petite popup d'info (pause, erreurs…), refermée au bout de 5 s.
pub fn info(summary: &str, body: &str) {
    let Ok(popup) = Popup::new("Rusky") else {
        eprintln!("{summary} : {body}");
        return;
    };
    popup.question.set_label(summary);
    popup.body.set_markup(&esc(body));
    popup.wait(5);
    popup.close();
}
