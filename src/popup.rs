//! Popups GTK4 en layer-shell : une carte dans un coin de l'écran, par-dessus
//! les fenêtres, sans dépendre du démon de notifications.

use crate::progress;
use crate::quiz::Quiz;
use crate::srs::Grade;
use anyhow::{Result, bail};
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Sans réponse, la popup se referme seule au bout de ce délai.
const TIMEOUT_SECS: f64 = 45.0;
const TIMEOUT: &str = "__timeout";
const CLOSED: &str = "__closed";

pub enum Outcome {
    Graded(Grade),
    /// Partie toute seule : reportée sans pénalité, comme « Plus tard ».
    Later,
    /// Popup fermée sans réponse.
    Dismissed,
}

/// Pose la question. Bloque jusqu'à la réponse, la fermeture ou le délai.
pub fn present(quiz: &Quiz) -> Result<Outcome> {
    let popup = Popup::new(&quiz.header)?;
    popup.question.set_label(&quiz.big);
    popup.pron.set_label(quiz.pron.as_deref().unwrap_or_default());
    popup.pron.set_visible(quiz.pron.is_some());
    popup.prompt.set_label(&quiz.prompt);

    let buttons: Vec<gtk::Button> = quiz
        .choices
        .iter()
        .enumerate()
        .map(|(i, c)| popup.button(&i.to_string(), c, &["choice"]))
        .collect();
    let later = popup.button("later", "Plus tard", &["later"]);
    popup.win.present();

    let picked = popup.wait();
    let outcome = match picked.parse::<usize>() {
        Ok(i) => {
            let ok = i == quiz.correct;
            for (j, b) in buttons.iter().enumerate() {
                b.set_sensitive(false);
                if j == quiz.correct {
                    b.add_css_class("right");
                } else if j == i {
                    b.add_css_class("wrong");
                }
            }
            later.set_visible(false);
            popup.prompt.set_label(&if ok {
                format!("✓ Bonne réponse !  +{} XP", progress::XP_GOOD)
            } else {
                format!("✗ Pas tout à fait  +{} XP", progress::XP_MISS)
            });
            popup.prompt.add_css_class(if ok { "ok" } else { "ko" });
            if let Some(n) = &quiz.note {
                popup.body.set_label(n);
                popup.body.set_visible(true);
            }
            popup.button("next", "Continuer", &["primary", "next"]).grab_focus();
            popup.wait();
            Outcome::Graded(if ok { Grade::Good } else { Grade::Again })
        }
        Err(_) if picked == "later" || picked == TIMEOUT => Outcome::Later,
        Err(_) => Outcome::Dismissed,
    };
    popup.close();
    Ok(outcome)
}

/// Petite popup d'info (pause, erreurs…), refermée au bout de 5 s.
pub fn info(summary: &str, body: &str) {
    let Ok(popup) = Popup::new("Rusky") else {
        eprintln!("{summary} : {body}");
        return;
    };
    popup.question.set_label(summary);
    popup.prompt.set_label(body);
    popup.timer.set_visible(false);
    let m = popup.main.clone();
    glib::timeout_add_seconds_local_once(5, move || m.quit());
    popup.win.present();
    popup.main.run();
    popup.close();
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
.question { font-size: 30px; font-weight: 700; margin-top: 12px; }
.pron { font-size: 14px; color: #8b8d94; margin-top: 2px; }
.prompt { font-size: 14px; color: #b4b6bd; margin-top: 10px; }
.prompt.ok { color: #8bd5a0; font-weight: 600; }
.prompt.ko { color: #f08a8a; font-weight: 600; }
.body { font-size: 13px; color: #b4b6bd; margin-top: 10px; }
.actions { margin-top: 14px; }
.actions button { border-radius: 9px; padding: 8px 14px; border: 1px solid alpha(#ffffff, 0.06);
                  background: #26282d; color: #ececef; box-shadow: none; font-weight: 500;
                  transition: background 120ms; }
.actions button:hover { background: #2e3036; }
.actions button:disabled { color: #8b8d94; }
.actions button.right, .actions button.right:disabled
  { background: alpha(#8bd5a0, 0.16); border-color: alpha(#8bd5a0, 0.6); color: #c9f0d4; }
.actions button.wrong, .actions button.wrong:disabled
  { background: alpha(#f08a8a, 0.14); border-color: alpha(#f08a8a, 0.55); color: #f6c4c4; }
.actions button.primary { background: #7aa2f7; border-color: transparent; color: #0f1115; font-weight: 600; }
.actions button.primary:hover { background: #8fb2fa; }
.actions button.later { background: none; border: none; color: #8b8d94; padding: 4px; }
.actions button.later:hover { color: #ececef; }
progressbar.timer { margin: 16px 0 -6px 0; }
progressbar.timer trough { min-height: 2px; background: alpha(#ffffff, 0.06); border-radius: 1px; }
progressbar.timer progress { min-height: 2px; background: alpha(#7aa2f7, 0.7); border-radius: 1px; }
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

/// Une carte à l'écran, en haut à droite.
struct Popup {
    win: gtk::Window,
    card: gtk::Box,
    question: gtk::Label,
    pron: gtk::Label,
    prompt: gtk::Label,
    body: gtk::Label,
    actions: gtk::Box,
    timer: gtk::ProgressBar,
    hover: Rc<Cell<bool>>,
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

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["card", "hidden"])
            .width_request(412) // 340 px de carte + marges de l'ombre
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

        let label = |class: &str| {
            gtk::Label::builder()
                .wrap(true)
                .max_width_chars(34)
                .justify(gtk::Justification::Center)
                .css_classes([class])
                .build()
        };
        let (question, pron, prompt, body) = (label("question"), label("pron"), label("prompt"), label("body"));
        body.set_visible(false);
        let actions = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["actions"])
            .build();
        for w in [&question, &pron, &prompt, &body] {
            card.append(w);
        }
        card.append(&actions);
        let timer = gtk::ProgressBar::builder().fraction(1.0).css_classes(["timer"]).build();
        card.append(&timer);
        win.set_child(Some(&card));

        let main = glib::MainLoop::new(None, false);
        let chosen = Rc::new(RefCell::new(String::new()));
        let quit = {
            let (m, c) = (main.clone(), chosen.clone());
            move || {
                c.replace(CLOSED.into());
                m.quit();
            }
        };
        let q = quit.clone();
        close.connect_clicked(move |_| q());
        let q = quit.clone();
        win.connect_close_request(move |_| {
            q();
            glib::Propagation::Stop
        });
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gtk::gdk::Key::Escape {
                quit();
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);

        // Souris sur la carte : le décompte s'arrête.
        let hover = Rc::new(Cell::new(false));
        let motion = gtk::EventControllerMotion::new();
        let h = hover.clone();
        motion.connect_enter(move |_, _, _| h.set(true));
        let h = hover.clone();
        motion.connect_leave(move |_| h.set(false));
        card.add_controller(motion);

        // Seule la carte capte les clics : la marge transparente laisse passer.
        let c = card.clone();
        win.connect_map(move |w| {
            let (c, w) = (c.clone(), w.clone());
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

        Ok(Self { win, card, question, pron, prompt, body, actions, timer, hover, main, chosen })
    }

    /// Ajoute un bouton qui termine l'attente en renvoyant `id`.
    fn button(&self, id: &str, label: &str, classes: &[&str]) -> gtk::Button {
        let text = gtk::Label::builder().label(label).wrap(true).justify(gtk::Justification::Center).build();
        let b = gtk::Button::builder().child(&text).css_classes(classes.to_vec()).build();
        let (id, c, m) = (id.to_string(), self.chosen.clone(), self.main.clone());
        b.connect_clicked(move |_| {
            c.replace(id.clone());
            m.quit();
        });
        self.actions.append(&b);
        b
    }

    /// Attend un clic, avec le décompte (repart à zéro à chaque appel, en pause au survol).
    fn wait(&self) -> String {
        let left = Rc::new(Cell::new(TIMEOUT_SECS));
        let done = Rc::new(Cell::new(false));
        self.timer.set_fraction(1.0);
        let (l, d, h, bar, c, m) = (
            left.clone(), done.clone(), self.hover.clone(), self.timer.clone(),
            self.chosen.clone(), self.main.clone(),
        );
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if d.get() {
                return glib::ControlFlow::Break;
            }
            if !h.get() {
                l.set(l.get() - 0.05);
                bar.set_fraction((l.get() / TIMEOUT_SECS).max(0.0));
            }
            if l.get() <= 0.0 {
                c.replace(TIMEOUT.into());
                m.quit();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        self.main.run();
        done.set(true);
        // La carte a pu changer de taille : la zone cliquable suit.
        if let (Some(b), Some(s)) = (self.card.compute_bounds(&self.win), self.win.surface()) {
            let r = gtk::cairo::RectangleInt::new(b.x() as i32, b.y() as i32, b.width() as i32, 2000);
            s.set_input_region(Some(&gtk::cairo::Region::create_rectangle(&r)));
        }
        self.chosen.take()
    }

    /// Fait glisser la carte vers la droite puis détruit la fenêtre.
    fn close(self) {
        self.card.add_css_class("hidden");
        pump(170);
        self.win.destroy();
        while glib::MainContext::default().iteration(false) {}
    }
}
