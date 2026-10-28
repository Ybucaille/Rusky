//! `rusky revision` : tableau de bord (XP, niveaux, activité, points faibles)
//! et sessions de révision dans une vraie fenêtre.

use crate::db::{Card, Db, Session};
use crate::progress;
use crate::quiz::{self, kind_label};
use crate::ui::{self, QuizView};
use anyhow::Result;
use chrono::Local;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Cartes par session lancée à la main.
const SESSION_SIZE: usize = 10;
/// Jours affichés dans le graphique d'activité.
const ACTIVITY_DAYS: i64 = 30;

fn now() -> i64 {
    Local::now().timestamp()
}

fn day_start() -> i64 {
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    midnight.and_local_timezone(Local).earliest().map(|d| d.timestamp()).unwrap_or_else(now)
}

struct App {
    db: Db,
    win: gtk::Window,
    stack: gtk::Stack,
    dashboard: gtk::ScrolledWindow,
    session: RefCell<Option<SessionState>>,
    view: Rc<QuizView>,
    session_top: gtk::Label,
    summary: gtk::Box,
}

struct SessionState {
    cards: Vec<Card>,
    index: usize,
    good: usize,
    xp: i64,
    unlocked: Option<i64>,
}

/// Ouvre la fenêtre et bloque jusqu'à sa fermeture.
pub fn open(db: Db) -> Result<()> {
    ui::init()?;
    glib::set_prgname(Some("rusky"));
    glib::set_application_name("Rusky");
    let win = gtk::Window::builder()
        .title("Rusky")
        .default_width(1064)
        .default_height(780)
        .css_classes(["rusky-app"])
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(180)
        .build();
    let dashboard = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).build();
    stack.add_named(&dashboard, Some("dashboard"));

    // Page de session : la même carte de quiz que la popup, en plus grand.
    let session_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .spacing(14)
        .width_request(480)
        .build();
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let session_top = gtk::Label::builder().xalign(0.0).hexpand(true).css_classes(["session-top"]).build();
    let quit = gtk::Button::builder().label("Arrêter").css_classes(["flat-link"]).build();
    bar.append(&session_top);
    bar.append(&quit);
    session_page.append(&bar);
    let view = QuizView::new();
    let card = gtk::Box::builder().orientation(gtk::Orientation::Vertical).css_classes(["session-card"]).build();
    card.append(&view.root);
    session_page.append(&card);
    stack.add_named(&session_page, Some("session"));

    let summary = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .spacing(10)
        .build();
    stack.add_named(&summary, Some("summary"));
    win.set_child(Some(&stack));

    let app = Rc::new(App {
        db,
        win: win.clone(),
        stack,
        dashboard,
        session: RefCell::new(None),
        view,
        session_top,
        summary,
    });
    let a = app.clone();
    quit.connect_clicked(move |_| a.finish_session());

    let keys = gtk::EventControllerKey::new();
    let a = app.clone();
    keys.connect_key_pressed(move |_, k, _, _| {
        let page = a.stack.visible_child_name();
        match (page.as_deref(), k) {
            (Some("dashboard"), gtk::gdk::Key::Escape) => a.win.close(),
            (Some("session"), gtk::gdk::Key::Escape) => a.finish_session(),
            (Some("session"), gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) => {
                a.view.next();
                return glib::Propagation::Stop;
            }
            (Some(_), gtk::gdk::Key::Escape) => a.show_dashboard(),
            // 1, 2, 3 : choisir une réponse au clavier.
            (Some("session"), k) => {
                if let Some(d) = k.to_unicode().and_then(|c| c.to_digit(10)).filter(|&d| d >= 1) {
                    a.view.press(d as usize - 1);
                    return glib::Propagation::Stop;
                }
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    win.add_controller(keys);

    app.show_dashboard();
    // Tant que la fenêtre est ouverte, le démon ne lance pas de popup par-dessus.
    let a = app.clone();
    let heartbeat = move || {
        if let Err(e) = a.db.set_busy_until(now() + 90) {
            eprintln!("rusky : {e:#}");
        }
        glib::ControlFlow::Continue
    };
    heartbeat();
    glib::timeout_add_seconds_local(60, heartbeat);
    let main = glib::MainLoop::new(None, false);
    let m = main.clone();
    win.connect_close_request(move |_| {
        m.quit();
        glib::Propagation::Proceed
    });
    win.present();
    main.run();
    app.db.set_busy_until(0)?;
    Ok(())
}

impl App {
    fn show_dashboard(self: &Rc<Self>) {
        match self.build_dashboard() {
            Ok(page) => self.dashboard.set_child(Some(&page)),
            Err(e) => self.dashboard.set_child(Some(&gtk::Label::new(Some(&format!("Erreur : {e:#}"))))),
        }
        self.stack.set_visible_child_name("dashboard");
    }

    fn build_dashboard(self: &Rc<Self>) -> Result<gtk::Box> {
        let level = progress::level(&self.db)?;
        // Colonne centrée : lisible même en plein écran.
        let page = vbox(22);
        page.add_css_class("page");
        page.set_halign(gtk::Align::Center);
        page.set_width_request(1000);
        page.append(&self.header()?);
        page.append(&hero(&level));
        page.append(&self.actions(&level)?);
        page.append(&self.tiles()?);

        let activity = panel();
        activity.append(&caption("ACTIVITÉ · 30 DERNIERS JOURS"));
        activity.append(&activity_chart(self.db.daily(now(), ACTIVITY_DAYS)?));
        page.append(&activity);

        let cols = gtk::Box::builder().spacing(12).homogeneous(true).build();
        cols.append(&self.tiers_panel(&level)?);
        let right = vbox(12);
        right.append(&self.kinds_panel()?);
        right.append(&self.hardest_panel()?);
        cols.append(&right);
        page.append(&cols);
        Ok(page)
    }

    /// Logo et bouton de pause.
    fn header(self: &Rc<Self>) -> Result<gtk::Box> {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&gtk::Label::builder().label("rusky").css_classes(["wordmark"]).build());
        header.append(&gtk::Label::builder().label("ру́сский").valign(gtk::Align::Baseline).css_classes(["wordmark-ru"]).build());
        header.append(&gtk::Box::builder().hexpand(true).build());
        header.append(&self.pause_button()?);
        Ok(header)
    }

    /// Les trois façons de lancer une session.
    fn actions(self: &Rc<Self>, level: &progress::Level) -> Result<gtk::Box> {
        let (due, seen, mistakes, fresh) = self.db.session_counts(now(), level.tier)?;
        let review_sub = if due > 0 {
            plural(due, "carte à revoir", "cartes à revoir")
        } else if seen > 0 {
            "Rien de dû : réviser d'avance".to_string()
        } else {
            "Commencer par les premières cartes".to_string()
        };
        let actions = gtk::Box::builder().spacing(12).homogeneous(true).build();
        actions.append(&self.action(Session::Review, "Réviser", &review_sub, true, seen + fresh > 0));
        actions.append(&self.action(Session::Mistakes, "Mes erreurs", &plural(mistakes, "carte à reprendre", "cartes à reprendre"), false, mistakes > 0));
        actions.append(&self.action(Session::New, "Découvrir", &plural(fresh, "carte nouvelle", "cartes nouvelles"), false, fresh > 0));
        Ok(actions)
    }

    /// Chiffres clés : série, réussite, révisions, cartes acquises.
    fn tiles(&self) -> Result<gtk::Box> {
        let db = &self.db;
        let t = now();
        let stats = db.stats(t, day_start())?;
        let (week_total, week_good) = db.accuracy_since(t - 7 * 86_400)?;
        let (all_total, _) = db.accuracy_since(0)?;
        let acquired: i64 = db.tier_overview(progress::ACQUIRED_REPS)?.iter().map(|r| r.2).sum();
        let tiles = gtk::Box::builder().spacing(12).homogeneous(true).build();
        tiles.append(&tile(&stats.streak_days.to_string(), "jours d'affilée", &format!("record : {}", db.best_streak()?)));
        tiles.append(&tile(&percent(week_good, week_total), "réussite sur 7 jours", &plural(week_total, "réponse", "réponses")));
        tiles.append(&tile(&stats.reviewed_today.to_string(), "révisions aujourd'hui", &format!("{all_total} au total")));
        tiles.append(&tile(&acquired.to_string(), "cartes acquises", &format!("sur {}", stats.total)));
        Ok(tiles)
    }

    fn tiers_panel(&self, level: &progress::Level) -> Result<gtk::Box> {
        let tiers = panel();
        tiers.append(&caption("NIVEAUX"));
        for (tier, seen, acq, total) in self.db.tier_overview(progress::ACQUIRED_REPS)? {
            let row = gtk::Box::builder().spacing(10).css_classes(["tier-row"]).build();
            if tier > level.tier {
                row.add_css_class("locked");
            }
            row.append(&gtk::Label::builder().label(tier.to_string()).css_classes(["tier-num"]).build());
            let names = vbox(4);
            names.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.append(&gtk::Label::builder().label(progress::name(tier)).xalign(0.0).hexpand(true).css_classes(["tier-name"]).build());
            let info = if tier > level.tier { "verrouillé".to_string() } else { format!("{acq} / {total}") };
            line.append(&text(&info, "muted"));
            names.append(&line);
            let f = if total > 0 { acq as f64 / total as f64 } else { 0.0 };
            let meter = gtk::ProgressBar::builder().fraction(f).css_classes(["meter", "thin", "ok"]).build();
            meter.set_tooltip_text(Some(&format!("{seen} vues, {acq} acquises sur {total}")));
            names.append(&meter);
            row.append(&names);
            tiers.append(&row);
        }
        Ok(tiers)
    }

    fn kinds_panel(&self) -> Result<gtk::Box> {
        let kinds = panel();
        kinds.append(&caption("RÉUSSITE PAR TYPE"));
        let acc = self.db.kind_accuracy()?;
        if acc.is_empty() {
            kinds.append(&paragraph("Réponds à quelques cartes pour voir tes points forts.", "muted"));
        }
        for kind in ["alphabet", "vocab", "phrase", "grammar"] {
            if let Some((_, total, good)) = acc.iter().find(|(k, _, _)| k == kind) {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.set_margin_top(4);
                line.append(&gtk::Box::builder().valign(gtk::Align::Center).css_classes(["dot", &format!("k-{kind}")]).build());
                line.append(&gtk::Label::builder().label(kind_label(kind)).xalign(0.0).hexpand(true).build());
                line.append(&text(&format!("{} · {}", percent(*good, *total), plural(*total, "réponse", "réponses")), "muted"));
                kinds.append(&line);
            }
        }
        Ok(kinds)
    }

    fn hardest_panel(&self) -> Result<gtk::Box> {
        let hard = panel();
        hard.append(&caption("À RETRAVAILLER"));
        let hardest = self.db.hardest(5)?;
        if hardest.is_empty() {
            hard.append(&text("Aucune carte ratée pour l'instant.", "muted"));
        }
        for c in hardest {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            line.set_margin_top(4);
            line.append(&gtk::Label::builder().label(&c.front).xalign(0.0).css_classes(["hard-front"]).build());
            line.append(
                &gtk::Label::builder()
                    .label(&c.back)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .css_classes(["muted"])
                    .build(),
            );
            line.append(&text(&format!("{}×", c.lapses), "muted"));
            hard.append(&line);
        }
        Ok(hard)
    }

    fn pause_button(self: &Rc<Self>) -> Result<gtk::Button> {
        let paused = self.db.paused_until()?.filter(|&u| u > now());
        let label = match paused {
            Some(u) => format!("En pause jusqu'à {} · reprendre", Local.timestamp_opt(u, 0).unwrap().format("%H:%M")),
            None => "Pause d'une heure".to_string(),
        };
        let b = gtk::Button::builder().label(label).valign(gtk::Align::Center).css_classes(["flat-link"]).build();
        let a = self.clone();
        b.connect_clicked(move |_| {
            let until = if paused.is_some() { 0 } else { now() + 3600 };
            if let Err(e) = a.db.set_meta("paused_until", &until.to_string()) {
                eprintln!("rusky : {e:#}");
            }
            a.show_dashboard();
        });
        Ok(b)
    }

    fn action(self: &Rc<Self>, mode: Session, title: &str, sub: &str, primary: bool, enabled: bool) -> gtk::Button {
        let inner = vbox(2);
        inner.append(&gtk::Label::builder().label(title).xalign(0.0).css_classes(["action-title"]).build());
        inner.append(&gtk::Label::builder().label(sub).xalign(0.0).css_classes(["action-sub"]).build());
        let b = gtk::Button::builder().child(&inner).css_classes(["action"]).sensitive(enabled).build();
        if primary {
            b.add_css_class("primary");
        }
        let a = self.clone();
        b.connect_clicked(move |_| a.start_session(mode));
        b
    }

    fn start_session(self: &Rc<Self>, mode: Session) {
        let cards = self
            .db
            .unlocked_tier()
            .and_then(|tier| self.db.session_cards(mode, SESSION_SIZE, now(), tier))
            .unwrap_or_default();
        // Rien de dû : on révise les nouvelles cartes à la place.
        let cards = if cards.is_empty() && mode == Session::Review {
            self.db
                .unlocked_tier()
                .and_then(|tier| self.db.session_cards(Session::New, SESSION_SIZE, now(), tier))
                .unwrap_or_default()
        } else {
            cards
        };
        if cards.is_empty() {
            return;
        }
        self.session.replace(Some(SessionState { cards, index: 0, good: 0, xp: 0, unlocked: None }));
        self.stack.set_visible_child_name("session");
        self.next_card();
    }

    fn next_card(self: &Rc<Self>) {
        let current = self.session.borrow().as_ref().map(|s| (s.cards.get(s.index).cloned(), s.index, s.cards.len()));
        let (card, index, total) = match current {
            None => return,
            Some((None, ..)) => return self.finish_session(),
            Some((Some(card), index, total)) => (card, index, total),
        };
        let quiz = match quiz::build(&self.db, &card, now()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("rusky : {e:#}");
                return self.finish_session();
            }
        };
        self.session_top.set_label(&format!("CARTE {} / {}", index + 1, total));

        let a = self.clone();
        let on_answer: Rc<dyn Fn(bool)> = Rc::new(move |ok| {
            let mut s = a.session.borrow_mut();
            let Some(s) = s.as_mut() else { return };
            let card = &mut s.cards[s.index];
            match progress::answer(&a.db, card, ok, now()) {
                Ok(unlocked) => s.unlocked = s.unlocked.or(unlocked),
                Err(e) => eprintln!("rusky : {e:#}"),
            }
            if ok {
                s.good += 1;
                s.xp += progress::XP_GOOD;
            } else {
                s.xp += progress::XP_MISS;
            }
        });
        let a = self.clone();
        let on_next: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(s) = a.session.borrow_mut().as_mut() {
                s.index += 1;
            }
            let a = a.clone();
            // Hors du gestionnaire de clic : la carte est reconstruite.
            glib::idle_add_local_once(move || a.next_card());
        });
        self.view.show(&quiz, on_answer, on_next);
    }

    fn finish_session(self: &Rc<Self>) {
        let Some(s) = self.session.take() else {
            return self.show_dashboard();
        };
        let answered = s.index.min(s.cards.len());
        if answered == 0 {
            return self.show_dashboard();
        }
        while let Some(c) = self.summary.first_child() {
            self.summary.remove(&c);
        }
        let centered = |l: gtk::Label| {
            l.set_xalign(0.5);
            l.set_justify(gtk::Justification::Center);
            self.summary.append(&l);
        };
        centered(caption("SESSION TERMINÉE"));
        centered(text(&format!("{} / {}", s.good, answered), "summary-score"));
        let verdict = match s.good * 100 / answered.max(1) {
            90.. => "Impeccable.",
            70..=89 => "Très bien, continue comme ça.",
            40..=69 => "Ça rentre. Les cartes ratées reviendront vite.",
            _ => "Normal au début : elles reviendront dans 10 minutes.",
        };
        centered(paragraph(verdict, "level-sub"));
        centered(text(&format!("+{} XP", s.xp), "big-number"));
        if let Some(tier) = s.unlocked {
            centered(text(&format!("Niveau {tier} débloqué : {}", progress::name(tier)), "prompt"));
        }
        let back = gtk::Button::builder().label("Retour au tableau de bord").css_classes(["primary"]).margin_top(14).build();
        let a = self.clone();
        back.connect_clicked(move |_| a.show_dashboard());
        self.summary.append(&back);
        self.stack.set_visible_child_name("summary");
        back.grab_focus();
    }
}

/// Niveau en cours, progression vers le suivant et XP.
fn hero(level: &progress::Level) -> gtk::Box {
    let hero = panel();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    let left = vbox(4);
    left.set_hexpand(true);
    left.append(&caption(&format!("NIVEAU {} SUR {}", level.tier, level.max_tier)));
    left.append(&text(progress::name(level.tier), "level-name"));
    let need = (level.total as f64 * progress::UNLOCK_SHARE).ceil() as i64;
    let sub = if level.tier < level.max_tier {
        format!(
            "{} / {need} cartes acquises pour ouvrir « {} »",
            level.acquired.min(need),
            progress::name(level.tier + 1)
        )
    } else {
        format!("{} / {} cartes acquises : dernier niveau", level.acquired, level.total)
    };
    left.append(&paragraph(&sub, "level-sub"));
    left.append(&gtk::ProgressBar::builder().fraction(level.progress()).css_classes(["meter"]).margin_top(10).build());
    row.append(&left);
    let xp = vbox(0);
    xp.set_valign(gtk::Align::Center);
    xp.append(&text(&level.xp.to_string(), "big-number"));
    xp.append(&text("XP", "tile-label"));
    row.append(&xp);
    hero.append(&row);
    hero
}

/// Barres d'activité : hauteur = révisions du jour, partie claire = réussies.
fn activity_chart(days: Vec<(i64, i64)>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_height(120).hexpand(true).build();
    let max = days.iter().map(|d| d.0).max().unwrap_or(0).max(5) as f64;
    area.set_draw_func(move |_, cr, w, h| {
        let n = days.len() as f64;
        let gap = 4.0;
        let bw = ((w as f64 - gap * (n - 1.0)) / n).max(2.0);
        let base = h as f64 - 16.0;
        for (i, &(total, good)) in days.iter().enumerate() {
            let x = i as f64 * (bw + gap);
            // Rail de fond pour chaque jour.
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.05);
            rounded(cr, x, 0.0, bw, base, 3.0);
            let _ = cr.fill();
            if total > 0 {
                let th = base * total as f64 / max;
                cr.set_source_rgba(0.478, 0.635, 0.969, 0.35);
                rounded(cr, x, base - th, bw, th, 3.0);
                let _ = cr.fill();
                let gh = base * good as f64 / max;
                cr.set_source_rgba(0.478, 0.635, 0.969, 1.0);
                rounded(cr, x, base - gh, bw, gh, 3.0);
                let _ = cr.fill();
            }
        }
        // Repères : il y a 30 jours, aujourd'hui.
        cr.set_source_rgba(0.545, 0.553, 0.580, 1.0);
        cr.set_font_size(11.0);
        cr.move_to(0.0, h as f64 - 2.0);
        let _ = cr.show_text("il y a 30 jours");
        if let Ok(ext) = cr.text_extents("aujourd'hui") {
            cr.move_to(w as f64 - ext.width() - 2.0, h as f64 - 2.0);
            let _ = cr.show_text("aujourd'hui");
        }
    });
    area
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    cr.close_path();
}

fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

fn panel() -> gtk::Box {
    let b = vbox(0);
    b.add_css_class("panel");
    b
}

fn caption(s: &str) -> gtk::Label {
    gtk::Label::builder().label(s).xalign(0.0).css_classes(["panel-title"]).build()
}

/// Libellé sur une ligne (le retour à la ligne GTK rogne les libellés courts).
fn text(s: &str, class: &str) -> gtk::Label {
    gtk::Label::builder().label(s).xalign(0.0).css_classes([class]).build()
}

/// Texte long, qui peut passer à la ligne.
fn paragraph(s: &str, class: &str) -> gtk::Label {
    gtk::Label::builder().label(s).xalign(0.0).wrap(true).css_classes([class]).build()
}

fn tile(value: &str, label: &str, hint: &str) -> gtk::Box {
    let b = panel();
    b.append(&text(value, "big-number"));
    b.append(&text(label, "tile-label"));
    b.append(&text(hint, "tile-hint"));
    b
}

fn percent(good: i64, total: i64) -> String {
    if total == 0 { "—".into() } else { format!("{} %", good * 100 / total) }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

use chrono::TimeZone;
