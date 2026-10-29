#!/usr/bin/env python3
"""Dessine le logo de rusky (SVG autonome : les lettres sont converties en tracés).

    python3 scripts/make-logo.py                 # packaging/rusky.svg (le logo retenu : le marque-page)
    python3 scripts/make-logo.py DOSSIER abc     # les trois pistes étudiées, pour comparer

Il faut fontTools (pip install fonttools) et la police Noto Serif Display.
"""
import os
import sys

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

FONT = "/usr/share/fonts/noto/NotoSerifDisplay-Bold.ttf"
PAPER, INK, RED = "#f4efe4", "#1a1814", "#c0392b"
S = 512


def glyph(font, char, size, cx, baseline):
    """Tracé SVG d'une lettre, centrée horizontalement sur cx."""
    gs = font.getGlyphSet()
    name = font.getBestCmap()[ord(char)]
    scale = size / font["head"].unitsPerEm
    pen = SVGPathPen(gs)
    width = gs[name].width * scale
    tpen = TransformPen(pen, (scale, 0, 0, -scale, cx - width / 2, baseline))
    gs[name].draw(tpen)
    return pen.getCommands(), width


def fitted(font, char, height, cx, cy):
    """Lettre mise à l'échelle pour mesurer `height` px de haut, centrée sur (cx, cy)."""
    gs = font.getGlyphSet()
    name = font.getBestCmap()[ord(char)]
    bp = BoundsPen(gs)
    gs[name].draw(bp)
    x0, y0, x1, y1 = bp.bounds
    scale = height / (y1 - y0)
    pen = SVGPathPen(gs)
    dx = cx - (x0 + x1) / 2 * scale
    dy = cy + (y0 + y1) / 2 * scale
    gs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, dx, dy)))
    return pen.getCommands(), (x1 - x0) * scale


def word(font, text, size, cx, baseline):
    """Tracés d'un mot (sans crénage : suffisant pour deux ou trois lettres)."""
    gs, cmap = font.getGlyphSet(), font.getBestCmap()
    scale = size / font["head"].unitsPerEm
    widths = [gs[cmap[ord(c)]].width * scale for c in text]
    x = cx - sum(widths) / 2
    paths, boxes = [], []
    for c, w in zip(text, widths):
        d, _ = glyph(font, c, size, x + w / 2, baseline)
        paths.append(d)
        boxes.append((x, w))
        x += w
    return paths, boxes


def svg(body, bg):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" width="{S}" height="{S}">'
            f'<rect width="{S}" height="{S}" rx="112" fill="{bg}"/>{body}</svg>\n')


def variant_a(font):
    """Я à l'encre, l'accent tonique ajouté au crayon rouge."""
    d, w = fitted(font, "Я", 270, 250, 290)
    pencil = (f'<path d="M296 112 C 314 96, 334 78, 356 58" fill="none" stroke="{RED}" '
              f'stroke-width="30" stroke-linecap="round"/>')
    return svg(f'<path d="{d}" fill="{INK}"/>{pencil}', PAPER)


def variant_b(font):
    """« ру » : le у entouré au crayon rouge, comme l'en-tête du site."""
    (dr, du), boxes = word(font, "ру", 330, 248, 318)
    ux, uw = boxes[1]
    cx, cy = ux + uw / 2, 286
    circle = (f'<ellipse cx="{cx:.0f}" cy="{cy}" rx="{uw * 0.66:.0f}" ry="132" fill="none" stroke="{RED}" '
              f'stroke-width="16" transform="rotate(-10 {cx:.0f} {cy})" opacity="0.9"/>')
    acute = (f'<path d="M{cx + 2:.0f} 168 L {cx + 38:.0f} 126" fill="none" stroke="{RED}" '
             f'stroke-width="22" stroke-linecap="round"/>')
    return svg(f'<path d="{dr}" fill="{INK}"/><path d="{du}" fill="{RED}"/>{circle}{acute}', PAPER)


def variant_c(font):
    """Я crème sur fond d'encre, et le ruban rouge du marque-page."""
    d, _ = fitted(font, "Я", 280, 222, 270)
    ribbon = f'<path d="M368 0 H 436 V 176 L 402 146 L 368 176 Z" fill="{RED}"/>'
    return svg(f'{ribbon}<path d="{d}" fill="{PAPER}"/>', INK)


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if len(sys.argv) == 1:
        path = os.path.join(root, "packaging", "rusky.svg")
        open(path, "w").write(variant_c(TTFont(FONT)))
        print(path)
        return
    out = sys.argv[1]
    which = sys.argv[2] if len(sys.argv) > 2 else "abc"
    os.makedirs(out, exist_ok=True)
    font = TTFont(FONT)
    for v in which:
        path = os.path.join(out, f"logo-{v}.svg")
        open(path, "w").write({"a": variant_a, "b": variant_b, "c": variant_c}[v](font))
        print(path)


if __name__ == "__main__":
    main()
