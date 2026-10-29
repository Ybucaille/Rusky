//! Enregistrement de la vidéo du README (compilé seulement avec `--features demo`).
//!
//! La popup et la fenêtre s'ouvrent invisibles et laissent passer les clics : rusky
//! dessine lui-même chaque image hors écran et l'envoie à ffmpeg. Rien du bureau
//! n'apparaît dans la vidéo ; `scripts/make-demo.sh` compose ensuite les scènes.

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk::{gdk, glib, graphene};
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

const FPS: u64 = 30;

static OUT: OnceLock<PathBuf> = OnceLock::new();

/// Active le mode démo : les scènes seront écrites dans `out`.
pub fn enable(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    OUT.set(out.to_path_buf()).ok();
    Ok(())
}

pub fn active() -> bool {
    OUT.get().is_some()
}

/// Cache une fenêtre (layer-shell) sous toutes les autres et la rend transparente aux
/// clics. GTK continue de la dessiner ; son contenu passe dans une boîte intermédiaire
/// qu'on filme, pour garder les transitions de la carte.
pub fn hide(win: &gtk::Window) {
    use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
    win.set_layer(Layer::Background);
    win.set_keyboard_mode(KeyboardMode::None);
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if let Some(child) = win.child() {
        win.set_child(None::<&gtk::Widget>);
        wrapper.append(&child);
    }
    win.set_child(Some(&wrapper));
    win.connect_map(|w| {
        if let Some(s) = w.surface() {
            s.set_input_region(Some(&gtk::cairo::Region::create()));
        }
    });
}

/// Filme `widget` à 30 images/s dans `<out>/<name>.mov` (avec transparence).
pub struct Recorder {
    widget: gtk::Widget,
    w: i32,
    h: i32,
    stdin: RefCell<Option<ChildStdin>>,
    child: RefCell<Option<Child>>,
    running: Cell<bool>,
}

impl Recorder {
    pub fn start(widget: &impl IsA<gtk::Widget>, w: i32, h: i32, name: &str) -> Result<Rc<Self>> {
        let path = OUT.get().context("mode démo inactif")?.join(format!("{name}.mov"));
        let mut child = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
            .arg(format!("{w}x{h}"))
            .args(["-r", &FPS.to_string(), "-i", "-", "-c:v", "qtrle"])
            .arg(&path)
            .stdin(Stdio::piped())
            .spawn()
            .context("ffmpeg introuvable")?;
        let stdin = child.stdin.take();
        let rec = Rc::new(Self {
            widget: widget.clone().upcast(),
            w,
            h,
            stdin: RefCell::new(stdin),
            child: RefCell::new(Some(child)),
            running: Cell::new(true),
        });
        let (r, t0, written) = (rec.clone(), Instant::now(), Rc::new(Cell::new(0u64)));
        glib::timeout_add_local(std::time::Duration::from_millis(1000 / FPS), move || {
            if !r.running.get() {
                return glib::ControlFlow::Break;
            }
            // On rattrape le temps réel : une image lente est répétée, la vidéo garde sa vitesse.
            let due = t0.elapsed().as_millis() as u64 * FPS / 1000 + 1;
            if written.get() < due {
                let frame = r.frame();
                while written.get() < due {
                    r.write(&frame);
                    written.set(written.get() + 1);
                }
            }
            glib::ControlFlow::Continue
        });
        Ok(rec)
    }

    fn frame(&self) -> Vec<u8> {
        let (w, h) = (self.w as usize, self.h as usize);
        let blank = vec![0; w * h * 4];
        let Some(renderer) = self.widget.native().and_then(|n| n.renderer()) else { return blank };
        // Le parent dessine l'enfant comme lors d'un vrai rendu (transformations comprises).
        let snapshot = gtk::Snapshot::new();
        if let Some(parent) = self.widget.parent() {
            parent.snapshot_child(&self.widget, &snapshot);
        }
        let Some(node) = snapshot.to_node() else { return blank };
        let viewport = graphene::Rect::new(0.0, 0.0, self.w as f32, self.h as f32);
        let texture = renderer.render_texture(&node, Some(&viewport));
        let mut downloader = gdk::TextureDownloader::new(&texture);
        downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
        let (bytes, stride) = downloader.download_bytes();
        let mut out = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            out.extend_from_slice(&bytes[row * stride..row * stride + w * 4]);
        }
        out
    }

    fn write(&self, frame: &[u8]) {
        if let Some(s) = self.stdin.borrow_mut().as_mut() {
            let _ = s.write_all(frame);
        }
    }

    /// Termine la vidéo.
    pub fn stop(&self) {
        self.running.set(false);
        self.stdin.borrow_mut().take();
        if let Some(mut c) = self.child.borrow_mut().take() {
            let _ = c.wait();
        }
    }
}

/// Lance `f` après `ms` millisecondes.
pub fn after(ms: u64, f: impl FnOnce() + 'static) {
    glib::timeout_add_local_once(std::time::Duration::from_millis(ms), f);
}
