//! Popups GTK4 en layer-shell : une carte dans un coin de l'écran, par-dessus
//! les fenêtres, sans dépendre du démon de notifications.

use crate::quiz::Quiz;
use crate::srs::Grade;
use crate::ui::{self, QuizView};
use anyhow::{Result, bail};
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Sans réponse, la popup se referme seule au bout de ce délai.
const TIMEOUT_SECS: f64 = 45.0;

pub enum Outcome {
    Graded(Grade),
    /// Partie toute seule ou « Plus tard » : reportée sans pénalité.
    Later,
    /// Popup fermée sans réponse.
    Dismissed,
}

/// Pose la question. Bloque jusqu'à la réponse, la fermeture ou le délai.
pub fn present(quiz: &Quiz) -> Result<Outcome> {
    let popup = Popup::new()?;
    let outcome = Rc::new(RefCell::new(Outcome::Dismissed));

    let later = gtk::Button::builder().label("Plus tard").css_classes(["ghost"]).build();
    popup.view.extra.append(&later);
    let (o, m) = (outcome.clone(), popup.main.clone());
    later.connect_clicked(move |_| {
        o.replace(Outcome::Later);
        m.quit();
    });

    let (o, timer) = (outcome.clone(), popup.timer.clone());
    let on_answer: Rc<dyn Fn(bool)> = Rc::new(move |ok| {
        o.replace(Outcome::Graded(if ok { Grade::Good } else { Grade::Again }));
        // Le décompte repart pour laisser le temps de lire l'explication.
        timer.restart();
    });
    let m = popup.main.clone();
    popup.view.show(quiz, on_answer, Rc::new(move || m.quit()));

    #[cfg(feature = "demo")]
    let rec = if crate::demo::active() { Some(demo_script(&popup, quiz)?) } else { None };
    let expired = popup.run();
    let outcome = outcome.replace(Outcome::Dismissed);
    #[cfg(feature = "demo")]
    if let Some(rec) = rec {
        // On filme aussi la sortie de la carte.
        popup.card.add_css_class("hidden");
        pump(500);
        rec.stop();
    }
    popup.close();
    Ok(match outcome {
        // Partie toute seule avant toute réponse : reportée.
        Outcome::Dismissed if expired => Outcome::Later,
        o => o,
    })
}

/// Vidéo du README : la popup répond juste toute seule, puis continue.
#[cfg(feature = "demo")]
fn demo_script(popup: &Popup, quiz: &Quiz) -> Result<Rc<crate::demo::Recorder>> {
    let rec = crate::demo::Recorder::start(&popup.win.child().unwrap(), DEMO_W, DEMO_H, "popup")?;
    let (v, c) = (popup.view.clone(), quiz.correct);
    crate::demo::after(3000, move || v.press(c));
    let v = popup.view.clone();
    crate::demo::after(8000, move || v.next());
    Ok(rec)
}

#[cfg(feature = "demo")]
const DEMO_W: i32 = 432;
#[cfg(feature = "demo")]
const DEMO_H: i32 = 560;

/// Petite popup d'info (niveau débloqué, pause…), refermée au bout de 6 s.
pub fn info(title: &str, body: &str) {
    let Ok(popup) = Popup::new() else {
        eprintln!("{title} : {body}");
        return;
    };
    popup.view.message(title, body);
    popup.timer.bar.set_visible(false);
    let m = popup.main.clone();
    glib::timeout_add_seconds_local_once(6, move || m.quit());
    popup.run();
    popup.close();
}

/// Fait tourner la boucle GTK pendant `ms` (le temps d'une transition CSS).
fn pump(ms: u64) {
    let main = glib::MainLoop::new(None, false);
    let m = main.clone();
    glib::timeout_add_local_once(Duration::from_millis(ms), move || m.quit());
    main.run();
}

/// Décompte visible sous la carte, en pause quand la souris est dessus.
#[derive(Clone)]
struct Countdown {
    bar: gtk::ProgressBar,
    hover: Rc<Cell<bool>>,
    left: Rc<Cell<f64>>,
    expired: Rc<Cell<bool>>,
    main: glib::MainLoop,
}

impl Countdown {
    fn start(&self) {
        let c = self.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if !c.main.is_running() {
                return glib::ControlFlow::Break;
            }
            if !c.hover.get() {
                c.left.set(c.left.get() - 0.05);
                c.bar.set_fraction((c.left.get() / TIMEOUT_SECS).max(0.0));
            }
            if c.left.get() <= 0.0 {
                c.expired.set(true);
                c.main.quit();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    fn restart(&self) {
        self.left.set(TIMEOUT_SECS);
        self.bar.set_fraction(1.0);
    }
}

struct Popup {
    win: gtk::Window,
    card: gtk::Box,
    view: Rc<QuizView>,
    timer: Countdown,
    main: glib::MainLoop,
}

impl Popup {
    fn new() -> Result<Self> {
        ui::init()?;
        if !gtk4_layer_shell::is_supported() {
            bail!("le compositeur ne gère pas wlr-layer-shell");
        }
        let win = gtk::Window::builder().css_classes(["rusky-popup"]).resizable(false).build();
        win.init_layer_shell();
        win.set_namespace(Some("rusky"));
        win.set_layer(Layer::Overlay);
        win.set_keyboard_mode(KeyboardMode::OnDemand);
        win.set_anchor(Edge::Top, true);
        win.set_anchor(Edge::Right, true);

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["popup-card", "hidden"])
            .width_request(432) // 360 px de carte + marges de l'ombre
            .build();
        let view = QuizView::new();
        view.set_frame(&card);
        let close = gtk::Button::builder().icon_name("window-close-symbolic").css_classes(["close"]).build();
        view.top.append(&close);
        card.append(&view.root);
        let bar = gtk::ProgressBar::builder().fraction(1.0).css_classes(["timer"]).build();
        card.append(&bar);
        win.set_child(Some(&card));
        #[cfg(feature = "demo")]
        if crate::demo::active() {
            crate::demo::hide(&win);
            win.set_size_request(DEMO_W, DEMO_H);
        }

        let main = glib::MainLoop::new(None, false);
        let m = main.clone();
        close.connect_clicked(move |_| m.quit());
        let m = main.clone();
        win.connect_close_request(move |_| {
            m.quit();
            glib::Propagation::Stop
        });
        let keys = gtk::EventControllerKey::new();
        let m = main.clone();
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gtk::gdk::Key::Escape {
                m.quit();
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);

        let hover = Rc::new(Cell::new(false));
        let motion = gtk::EventControllerMotion::new();
        let h = hover.clone();
        motion.connect_enter(move |_, _, _| h.set(true));
        let h = hover.clone();
        motion.connect_leave(move |_| h.set(false));
        card.add_controller(motion);

        // Seule la carte capte les clics : la marge transparente laisse passer.
        // La carte grandit après la réponse : la zone suit sa taille.
        let c = card.clone();
        win.connect_map(move |w| {
            let (c, w) = (c.clone(), w.clone());
            if let Some(surface) = w.surface() {
                let (c, w) = (c.clone(), w.clone());
                surface.connect_layout(move |_, _, _| update_input_region(&w, &c));
            }
            glib::idle_add_local_once(move || {
                update_input_region(&w, &c);
                c.remove_css_class("hidden");
            });
        });

        let timer = Countdown {
            bar,
            hover,
            left: Rc::new(Cell::new(TIMEOUT_SECS)),
            expired: Rc::new(Cell::new(false)),
            main: main.clone(),
        };
        Ok(Self { win, card, view, timer, main })
    }

    /// Affiche la popup et attend. Renvoie true si le délai a expiré.
    fn run(&self) -> bool {
        self.win.present();
        self.timer.start();
        self.main.run();
        self.timer.expired.get()
    }

    /// Fait glisser la carte vers la droite puis détruit la fenêtre.
    fn close(self) {
        self.card.add_css_class("hidden");
        pump(170);
        self.win.destroy();
        while glib::MainContext::default().iteration(false) {}
    }
}

fn update_input_region(win: &gtk::Window, card: &gtk::Box) {
    #[cfg(feature = "demo")]
    if crate::demo::active() {
        return;
    }
    if let (Some(b), Some(s)) = (card.compute_bounds(win), win.surface()) {
        let r = gtk::cairo::RectangleInt::new(b.x() as i32, b.y() as i32, b.width() as i32, b.height() as i32);
        s.set_input_region(Some(&gtk::cairo::Region::create_rectangle(&r)));
    }
}
