//! Ce que la popup et la fenêtre de révision ont en commun : le style et la carte de quiz.

use crate::progress;
use crate::quiz::Quiz;
use anyhow::Result;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Couleur d'accent, aussi utilisée dans le balisage Pango (syllabe accentuée).
pub const ACCENT: &str = "#7aa2f7";

pub const CSS: &str = r#"
@define-color bg #111214;
@define-color surface #18191c;
@define-color raised #1f2125;
@define-color line alpha(#ffffff, 0.07);
@define-color text #ececef;
@define-color muted #8b8d94;
@define-color soft #b4b6bd;
@define-color accent #7aa2f7;
@define-color ok #8bd5a0;
@define-color ko #f08a8a;

/* Noto Sans place correctement l'accent tonique combinant (Adwaita Sans le décale). */
* { font-family: "Noto Sans", sans-serif; }

/* ---- Carte de quiz ---- */
.quiz { color: @text; }
.head { font-size: 11px; font-weight: 600; letter-spacing: 1px; color: @muted; }
.dot { min-width: 7px; min-height: 7px; border-radius: 4px; margin: 0 2px; }
.k-alphabet { background: #e0b973; }
.k-vocab { background: @accent; }
.k-grammar { background: @ok; }
.k-phrase { background: #f0a3c0; }
.question { font-size: 26px; font-weight: 700; margin-top: 14px; }
.question.ru { font-family: "Noto Serif Display", "Noto Serif", serif; font-weight: 500; font-size: 40px; }
.pron { font-size: 15px; color: @muted; margin-top: 2px; letter-spacing: 0.5px; }
.prompt { font-size: 14px; color: @soft; margin-top: 12px; }
.prompt.ok { color: @ok; font-weight: 600; }
.prompt.ko { color: @ko; font-weight: 600; }
.note { font-size: 13px; color: @soft; margin-top: 10px; }
.choices { margin-top: 14px; }
.choices button, .extra button {
  border-radius: 10px; padding: 9px 14px; border: 1px solid @line;
  background: @raised; color: @text; box-shadow: none; font-weight: 500;
  transition: background 120ms, border-color 120ms;
}
.choices button:hover { background: #272a2f; border-color: alpha(#ffffff, 0.12); }
/* Le thème par défaut atténue les boutons désactivés : on garde la lisibilité. */
.choices button:disabled, .choices button:disabled label { filter: none; opacity: 1; color: @muted; }
.choices button.right, .choices button.right:disabled
  { background: alpha(@ok, 0.14); border-color: alpha(@ok, 0.6); color: #c9f0d4; }
.choices button.right:disabled label { color: #c9f0d4; }
.choices button.wrong:disabled label { color: #f6c4c4; }
.choices button.wrong, .choices button.wrong:disabled
  { background: alpha(@ko, 0.12); border-color: alpha(@ko, 0.55); color: #f6c4c4; }
button.primary { background: @accent; border-color: transparent; color: #0f1115; font-weight: 600; }
button.primary:hover { background: #8fb2fa; }
.extra button.ghost { background: none; border: none; color: @muted; padding: 4px; }
.extra button.ghost:hover { color: @text; }

/* ---- Popup ---- */
window.rusky-popup { background: transparent; }
.popup-card {
  background: @surface;
  border-radius: 18px;
  border: 1px solid @line;
  padding: 14px 18px 18px 18px;
  margin: 28px 28px 44px 44px;  /* place pour l'ombre */
  box-shadow: 0 2px 6px alpha(black, 0.25), 0 12px 32px alpha(black, 0.35);
  opacity: 1;
  transform: none;
  transition: opacity 220ms ease-out, transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.popup-card.hidden {
  opacity: 0;
  transform: translateX(40px);
  transition: opacity 160ms ease-in, transform 160ms ease-in;
}
.close { min-width: 24px; min-height: 24px; padding: 0; border-radius: 8px;
         background: none; box-shadow: none; border: none; color: @muted; }
.close:hover { background: alpha(#ffffff, 0.06); color: @text; }
progressbar.timer { margin: 16px 0 -6px 0; }
progressbar.timer trough, progressbar.meter trough
  { min-height: 2px; background: alpha(#ffffff, 0.06); border-radius: 2px; }
progressbar.timer progress { min-height: 2px; background: alpha(@accent, 0.7); border-radius: 2px; }

/* ---- Fenêtre de révision ---- */
window.rusky-app { background: @bg; color: @text; }
window.rusky-app stack { background: @bg; }
.page { padding: 28px 32px; }
/* Vidéo du README : la fenêtre encadrée comme une fenêtre d'application. */
window.rusky-app.demo { background: transparent; }
window.rusky-app.demo stack.demo-frame {
  border-radius: 18px; border: 1px solid @line; margin: 30px;
  box-shadow: 0 16px 48px alpha(black, 0.55);
}
.wordmark { font-family: "Noto Serif Display", "Noto Serif", serif; font-size: 30px; font-weight: 600; }
.wordmark-ru { font-family: "Noto Serif Display", "Noto Serif", serif; font-size: 16px; color: @muted; margin-left: 10px; }
.panel { background: @surface; border: 1px solid @line; border-radius: 16px; padding: 18px 20px; }
.panel-title { font-size: 11px; font-weight: 600; letter-spacing: 1px; color: @muted; margin-bottom: 12px; }
.level-name { font-size: 22px; font-weight: 700; }
.level-sub { color: @soft; font-size: 13px; }
.big-number { font-size: 30px; font-weight: 700; }
.tile-label { font-size: 12px; color: @muted; }
.tile-hint { font-size: 12px; color: @soft; }
progressbar.meter trough { min-height: 8px; border-radius: 4px; }
progressbar.meter progress { min-height: 8px; border-radius: 4px; background: @accent; }
progressbar.meter.thin trough, progressbar.meter.thin progress { min-height: 4px; }
progressbar.meter.ok progress { background: @ok; }
.tier-row { padding: 6px 0; }
.tier-row.locked { opacity: 0.4; }
.tier-num { font-weight: 700; color: @muted; min-width: 22px; }
.tier-name { font-weight: 500; }
.muted { color: @muted; font-size: 12px; }
.hard-front { font-family: "Noto Serif Display", "Noto Serif", serif; font-size: 17px; }
.action {
  border-radius: 14px; padding: 14px 18px; border: 1px solid @line;
  background: @raised; color: @text; box-shadow: none;
}
.action:hover { background: #262930; }
.action.primary { background: @accent; color: #0f1115; border-color: transparent; }
.action.primary:hover { background: #8fb2fa; }
.action:disabled { opacity: 0.45; }
.action-title { font-weight: 700; font-size: 15px; }
.action-sub { font-size: 12px; opacity: 0.75; }
.session-card { background: @surface; border: 1px solid @line; border-radius: 20px; padding: 22px 26px 24px 26px; }
.session-top { color: @muted; font-size: 12px; font-weight: 600; letter-spacing: 1px; }
button.flat-link { background: none; border: none; box-shadow: none; color: @muted; }
button.flat-link:hover { color: @text; }
.summary-score { font-family: "Noto Serif Display", "Noto Serif", serif; font-size: 64px; font-weight: 600; }
"#;

/// Initialise GTK et charge le style (une seule fois).
pub fn init() -> Result<()> {
    thread_local!(static READY: RefCell<bool> = const { RefCell::new(false) });
    if READY.with(|r| *r.borrow()) {
        return Ok(());
    }
    gtk::init()?;
    // Le logo installé dans le thème d'icônes (packaging/rusky.svg).
    gtk::Window::set_default_icon_name("rusky");
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    let display = gtk::gdk::Display::default().ok_or_else(|| anyhow::anyhow!("pas d'écran"))?;
    gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    READY.with(|r| *r.borrow_mut() = true);
    Ok(())
}

/// « ma-la-KO » → « ma·la·ko », la syllabe accentuée en gras et en couleur.
pub fn pron_markup(pron: &str) -> String {
    pron.split(' ')
        .map(|word| {
            word.split('-')
                .map(|syl| {
                    let stressed = syl.chars().any(char::is_alphabetic)
                        && syl.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase)
                        && word.contains('-');
                    let text = glib::markup_escape_text(&syl.to_lowercase()).to_string();
                    if stressed {
                        format!("<span weight=\"bold\" foreground=\"{ACCENT}\">{text}</span>")
                    } else {
                        text
                    }
                })
                .collect::<Vec<_>>()
                .join("·")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Le texte contient-il du cyrillique ? (police à empattements pour le russe)
fn is_russian(s: &str) -> bool {
    s.chars().any(|c| ('\u{400}'..='\u{4FF}').contains(&c))
}

fn label(class: &str) -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .max_width_chars(36)
        .justify(gtk::Justification::Center)
        .css_classes([class])
        .build()
}

/// La carte de quiz : en-tête, question, prononciation, réponses, explication.
pub struct QuizView {
    pub root: gtk::Box,
    /// Ligne d'en-tête : la popup y ajoute son bouton de fermeture.
    pub top: gtk::Box,
    /// Boutons supplémentaires sous les réponses (« Plus tard »), cachés après la réponse.
    pub extra: gtk::Box,
    dot: gtk::Box,
    head: gtk::Label,
    question: gtk::Label,
    pron: gtk::Label,
    prompt: gtk::Label,
    note: gtk::Label,
    choices: gtk::Box,
}

impl QuizView {
    pub fn new() -> Rc<Self> {
        let root = gtk::Box::builder().orientation(gtk::Orientation::Vertical).css_classes(["quiz"]).build();
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let dot = gtk::Box::builder().valign(gtk::Align::Center).css_classes(["dot"]).build();
        let head = gtk::Label::builder().xalign(0.0).hexpand(true).css_classes(["head"]).build();
        top.append(&dot);
        top.append(&head);
        root.append(&top);

        let (question, pron, prompt, note) = (label("question"), label("pron"), label("prompt"), label("note"));
        let choices = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["choices"])
            .build();
        let extra = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(4)
            .css_classes(["extra"])
            .build();
        for w in [&question, &pron, &prompt] {
            root.append(w);
        }
        // L'explication vient sous les réponses : rien ne bouge sous le curseur au clic.
        root.append(&choices);
        root.append(&note);
        root.append(&extra);
        Rc::new(Self { root, top, extra, dot, head, question, pron, prompt, note, choices })
    }

    /// Pose la question. `on_answer(juste)` est appelé au clic sur une réponse,
    /// `on_next()` au clic sur « Continuer ».
    pub fn show(&self, quiz: &Quiz, on_answer: Rc<dyn Fn(bool)>, on_next: Rc<dyn Fn()>) {
        self.dot.set_css_classes(&["dot", &format!("k-{}", quiz.kind)]);
        self.head.set_label(&quiz.header());
        self.set_question(&quiz.big);
        match &quiz.pron {
            Some(p) => self.pron.set_markup(&pron_markup(p)),
            None => self.pron.set_label(""),
        }
        self.pron.set_visible(quiz.pron.is_some());
        self.prompt.set_css_classes(&["prompt"]);
        self.prompt.set_label(&quiz.prompt);
        self.note.set_visible(false);
        self.extra.set_visible(true);
        while let Some(child) = self.choices.first_child() {
            self.choices.remove(&child);
        }
        // Le bouton « Continuer » de la carte précédente.
        if let Some(last) = self.root.last_child().filter(|w| w.has_css_class("next")) {
            self.root.remove(&last);
        }

        let buttons: Rc<RefCell<Vec<gtk::Button>>> = Rc::default();
        for (i, choice) in quiz.choices.iter().enumerate() {
            let text = gtk::Label::builder().label(choice).wrap(true).justify(gtk::Justification::Center).build();
            let b = gtk::Button::builder().child(&text).build();
            let (buttons2, prompt, note, extra, root) = (
                buttons.clone(), self.prompt.clone(), self.note.clone(), self.extra.clone(), self.root.clone(),
            );
            let (on_answer, on_next, correct) = (on_answer.clone(), on_next.clone(), quiz.correct);
            let explanation = quiz.note.clone();
            b.connect_clicked(move |_| {
                let ok = i == correct;
                for (j, b) in buttons2.borrow().iter().enumerate() {
                    b.set_sensitive(false);
                    if j == correct {
                        b.add_css_class("right");
                    } else if j == i {
                        b.add_css_class("wrong");
                    }
                }
                prompt.set_label(&if ok {
                    format!("Bonne réponse  ·  +{} XP", progress::XP_GOOD)
                } else {
                    format!("Pas tout à fait  ·  +{} XP", progress::XP_MISS)
                });
                prompt.add_css_class(if ok { "ok" } else { "ko" });
                if let Some(n) = &explanation {
                    note.set_label(n);
                    note.set_visible(true);
                }
                extra.set_visible(false);
                let next = gtk::Button::builder().label("Continuer").css_classes(["primary", "next"]).margin_top(12).build();
                let on_next = on_next.clone();
                next.connect_clicked(move |_| on_next());
                root.append(&next);
                next.grab_focus();
                on_answer(ok);
            });
            self.choices.append(&b);
            buttons.borrow_mut().push(b);
        }
    }

    /// Choisit la réponse n° `index` (raccourcis clavier), si la question attend encore.
    pub fn press(&self, index: usize) {
        let mut child = self.choices.first_child();
        for _ in 0..index {
            child = child.and_then(|c| c.next_sibling());
        }
        if let Some(b) = child.and_downcast::<gtk::Button>().filter(|b| b.is_sensitive()) {
            b.emit_clicked();
        }
    }

    /// « Continuer », s'il est affiché.
    pub fn next(&self) {
        if let Some(b) = self.root.last_child().filter(|w| w.has_css_class("next")).and_downcast::<gtk::Button>() {
            b.emit_clicked();
        }
    }

    /// Un simple message (déblocage de niveau, pause…), sans réponses.
    pub fn message(&self, title: &str, body: &str) {
        self.dot.set_css_classes(&["dot", "k-vocab"]);
        self.head.set_label("RUSKY");
        self.set_question(title);
        self.pron.set_visible(false);
        self.prompt.set_label(body);
        self.note.set_visible(false);
        self.extra.set_visible(false);
    }

    fn set_question(&self, text: &str) {
        self.question.set_label(text);
        if is_russian(text) {
            self.question.add_css_class("ru");
        } else {
            self.question.remove_css_class("ru");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pron_markup;

    #[test]
    fn stressed_syllable_is_highlighted() {
        let m = pron_markup("ma-la-KO");
        assert!(m.starts_with("ma·la·<span"));
        assert!(m.contains(">ko</span>"));
        // Un mot d'une syllabe n'est pas mis en valeur.
        assert_eq!(pron_markup("kot"), "kot");
    }
}
