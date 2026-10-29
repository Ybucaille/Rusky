#!/usr/bin/env python3
"""Fabrique la vidéo du README (docs/demo.mp4 et docs/demo.gif).

1. rusky filme ses propres fenêtres hors écran (rien du bureau n'apparaît) ;
2. ce script compose les deux scènes sur un fond propre, ajoute un fondu et une musique.

    python3 scripts/make-demo.py --db ~/demo.db          # base avec un peu d'historique
    python3 scripts/make-demo.py --scenes /tmp/scenes    # réutilise des scènes déjà filmées

Il faut ffmpeg, numpy et Pillow, et une session Wayland (Hyprland, Sway…).
"""
import argparse
import math
import os
import shutil
import subprocess
import tempfile
import wave

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
W, H, FPS = 1280, 720, 30
BG, ACCENT, TEXT, MUTED = (17, 18, 20), (122, 162, 247), (236, 236, 239), (139, 141, 148)
SANS = "/usr/share/fonts/noto/NotoSans-Bold.ttf"
SANS_REG = "/usr/share/fonts/noto/NotoSans-Regular.ttf"
SERIF = "/usr/share/fonts/noto/NotoSerifDisplay-Medium.ttf"
FADE = 0.7


def run(*cmd):
    subprocess.run(cmd, check=True)


def duration(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path],
                         check=True, capture_output=True, text=True).stdout
    return float(out)


LOGO = os.path.join(ROOT, "packaging", "rusky.svg")
OUTRO = 3.2


def logo(size, work):
    """Le logo (packaging/rusky.svg) en PNG à la taille voulue."""
    path = os.path.join(work, f"logo-{size}.png")
    if not os.path.exists(path):
        run("rsvg-convert", "-w", str(size), "-h", str(size), LOGO, "-o", path)
    return Image.open(path).convert("RGBA")


def background(path, caption=None, work=None):
    """Fond sombre avec un halo bleu discret ; avec une légende, le logo et le texte à gauche."""
    img = Image.new("RGB", (W, H), BG)
    glow = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(glow)
    d.ellipse((W * 0.45, -H * 0.4, W * 1.25, H * 0.9), fill=(30, 40, 66))
    d.ellipse((-W * 0.3, H * 0.55, W * 0.35, H * 1.4), fill=(26, 28, 38))
    img = Image.blend(img, glow.filter(ImageFilter.GaussianBlur(160)), 0.9)
    d = ImageDraw.Draw(img)
    if caption:
        mark = logo(48, work)
        img.paste(mark, (64, 48), mark)
        d.text((126, 52), "rusky", font=ImageFont.truetype(SERIF, 34), fill=TEXT)
        d.text((230, 66), "ру́сский", font=ImageFont.truetype(SERIF, 18), fill=MUTED)
        title, sub = caption
        y = 260
        for line in title:
            d.text((64, y), line, font=ImageFont.truetype(SANS, 40), fill=TEXT)
            y += 54
        y += 14
        for line in sub:
            d.text((64, y), line, font=ImageFont.truetype(SANS_REG, 21), fill=MUTED)
            y += 32
    img.save(path)


def outro(path, work):
    """Carton de fin : le logo, le nom, l'adresse du site."""
    background(path)
    img = Image.open(path).convert("RGB")
    mark = logo(168, work)
    img.paste(mark, ((W - 168) // 2, 168), mark)
    d = ImageDraw.Draw(img)
    centered = lambda y, text, font, fill: d.text(((W - d.textlength(text, font=font)) / 2, y), text, font=font, fill=fill)
    centered(372, "rusky", ImageFont.truetype(SERIF, 64), TEXT)
    centered(462, "Le russe par petites touches · 2 007 cartes · 10 niveaux", ImageFont.truetype(SANS_REG, 22), MUTED)
    centered(506, "ybucaille.github.io/Rusky", ImageFont.truetype(SANS, 22), ACCENT)
    img.save(path)


def music(path, seconds, rate=44100):
    """Nappes d'accords douces avec un petit motif pincé (même recette que LiveQuill)."""
    t = np.arange(int(seconds * rate)) / rate
    out = np.zeros_like(t)
    note = lambda n: 440 * 2 ** ((n - 69) / 12)
    chords = [[45, 52, 57, 60, 64], [41, 48, 52, 57], [48, 55, 59, 64], [43, 50, 55, 59, 62]]   # Am9 Fmaj7 Cmaj7 G6
    bar = 4.0
    for k in range(int(seconds / bar) + 1):
        start, ch = k * bar, chords[k % len(chords)]
        seg = (t >= start) & (t < start + bar + 1.5)
        tt = t[seg] - start
        env = np.minimum(1, tt / 1.2) * np.exp(-np.maximum(0, tt - bar + .3) * 2.5)
        for n in ch:
            f = note(n)
            out[seg] += env * .06 * (np.sin(math.tau * f * tt) + .35 * np.sin(math.tau * f * 1.003 * tt))
        for i, n in enumerate([ch[-1] + 12, ch[1] + 12, ch[2] + 12, ch[0] + 24]):
            s0 = start + i * bar / 4
            seg2 = (t >= s0) & (t < s0 + 1.2)
            u = t[seg2] - s0
            out[seg2] += .045 * np.exp(-u * 4) * np.sin(math.tau * note(n) * u)
    wet = out.copy()
    for dly, g in [(.031, .45), (.047, .38), (.071, .3), (.113, .25)]:
        n = int(dly * rate)
        for _ in range(6):
            wet[n:] += wet[:-n] * g * .5
    mix = .7 * out + .3 * wet / np.max(np.abs(wet))
    mix *= np.minimum(1, t / 2) * np.minimum(1, (seconds - t) / 3)
    mix = mix / np.max(np.abs(mix)) * .55
    stereo = np.stack([mix, np.roll(mix, int(.012 * rate))], -1)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes((stereo * 32767).astype("<i2").tobytes())


def scene(bg, clip, out, scale_h, x, y, length):
    """Pose une scène filmée (avec transparence) sur son fond."""
    run("ffmpeg", "-loglevel", "error", "-y", "-loop", "1", "-r", str(FPS), "-i", bg, "-i", clip,
        "-filter_complex", f"[1]scale=-1:{scale_h}:flags=lanczos,format=rgba[c];[0][c]overlay={x}:{y}:shortest=1,format=yuv420p",
        "-t", f"{length:.3f}", "-r", str(FPS), "-c:v", "libx264", "-crf", "16", "-preset", "slow", out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", help="base de démo (copiée, jamais modifiée)")
    ap.add_argument("--scenes", help="dossier de scènes déjà filmées (popup.mov, window.mov)")
    ap.add_argument("--bin", default=os.path.join(ROOT, "target/release/rusky"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs"))
    a = ap.parse_args()
    work = tempfile.mkdtemp(prefix="rusky-demo-")
    try:
        scenes = a.scenes
        if not scenes:
            run("cargo", "build", "--release", "--features", "demo", "--manifest-path", os.path.join(ROOT, "Cargo.toml"))
            scenes = os.path.join(work, "scenes")
            db = os.path.join(work, "demo.db")
            if a.db:
                shutil.copy(a.db, db)
            subprocess.run([a.bin, "demo-record", scenes], check=True, env={**os.environ, "RUSKY_DB": db})

        popup, window = os.path.join(scenes, "popup.mov"), os.path.join(scenes, "window.mov")
        bg1, bg2 = os.path.join(work, "bg1.png"), os.path.join(work, "bg2.png")
        background(bg1, (["Une carte toutes", "les 25 minutes."],
                         ["Trois réponses, la prononciation", "à la française, une explication", "en une phrase."]), work)
        background(bg2)
        end_png, s3 = os.path.join(work, "outro.png"), os.path.join(work, "s3.mp4")
        outro(end_png, work)
        run("ffmpeg", "-loglevel", "error", "-y", "-loop", "1", "-r", str(FPS), "-i", end_png, "-t", str(OUTRO),
            "-vf", "format=yuv420p", "-c:v", "libx264", "-crf", "16", "-preset", "slow", s3)

        # La popup (432×560) à droite de la légende, la fenêtre (1124×980) centrée.
        d1, d2 = duration(popup), duration(window)
        s1, s2 = os.path.join(work, "s1.mp4"), os.path.join(work, "s2.mp4")
        scene(bg1, popup, s1, 600, 690, 70, d1)
        scene(bg2, window, s2, 700, (W - round(1124 * 700 / 980)) // 2, 10, d2)

        total = d1 + d2 + OUTRO - 2 * FADE
        wav = os.path.join(work, "music.wav")
        music(wav, total)
        os.makedirs(a.out, exist_ok=True)
        mp4 = os.path.join(a.out, "demo.mp4")
        run("ffmpeg", "-loglevel", "error", "-y", "-i", s1, "-i", s2, "-i", s3, "-i", wav, "-filter_complex",
            f"[0][1]xfade=transition=fade:duration={FADE}:offset={d1 - FADE:.3f}[a];"
            f"[a][2]xfade=transition=fade:duration={FADE}:offset={d1 + d2 - 2 * FADE:.3f},fade=in:d=0.5,"
            f"fade=out:st={total - 0.8:.3f}:d=0.8[v]",
            "-map", "[v]", "-map", "3:a", "-c:v", "libx264", "-crf", "22", "-preset", "slow", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart", "-shortest", mp4)
        gif = os.path.join(a.out, "demo.gif")
        run("ffmpeg", "-loglevel", "error", "-y", "-i", mp4, "-filter_complex",
            "fps=12,scale=860:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];"
            "[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle", gif)
        for f in (mp4, gif):
            print(f"{f}  {os.path.getsize(f) / 1e6:.1f} Mo")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
