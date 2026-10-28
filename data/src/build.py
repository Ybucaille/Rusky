"""Convertit un fichier de niveau compact en TOML rusky.

Format (une carte par ligne, champs séparés par « | ») :
  # niveau = 2            (ligne d'en-tête obligatoire)
  @vocab | @phrase | @grammar | @alphabet   (section)
  vocab/phrase : russe accentué | sens | note facultative
  alphabet     : lettre | son | note
  grammar      : russe | consigne | bonne réponse | mauvaise 1 ; mauvaise 2 | note
Usage : build_tier.py source.txt sortie.toml préfixe
"""
import sys
import unicodedata

ACUTE = "́"
VOWELS = set("аеёиоуыэюяАЕЁИОУЫЭЮЯ")


def q(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def check_chars(text, where):
    # Lettre latine collée à du cyrillique (а/a, о/o…) ou caractère invisible.
    bad = [w for w in text.split()
           if any("\u0400" <= ch <= "\u04ff" for ch in w) and any("a" <= ch.lower() <= "z" for ch in w)]
    if "\u00ad" in text or "\u200b" in text:
        bad.append("(caractère invisible)")
    if bad:
        print(f"lettres mélangées ({where}) : {' '.join(bad)}", file=sys.stderr)
    return not bad


def check_stress(text, where):
    # Chaque mot russe de 2 syllabes ou plus doit porter un accent (ou un ё).
    bad = []
    for w in text.split():
        w2 = "".join(ch for ch in w if ch.isalpha() or ch == ACUTE)
        if not any("Ѐ" <= ch <= "ӿ" for ch in w2):
            continue
        n = sum(ch in VOWELS for ch in w2)
        if n >= 2 and ACUTE not in w2 and "ё" not in w2.lower():
            bad.append(w)
    if bad:
        print(f"accent manquant ({where}) : {' '.join(bad)}", file=sys.stderr)
    return not bad


def main(src, dst, prefix):
    tier = None
    section = None
    counts = {}
    out = []
    ok = True
    seen = set()
    for n, raw in enumerate(open(src, encoding="utf-8"), 1):
        line = unicodedata.normalize("NFC", raw.strip())
        # NFC fusionne й/ё mais garde l'accent combinant sur les autres voyelles.
        if not line:
            continue
        if line.startswith("# niveau"):
            tier = int(line.split("=")[1])
            continue
        if line.startswith("#"):
            continue
        if line.startswith("@"):
            section = line[1:].strip()
            continue
        f = [x.strip() for x in line.split("|")]
        ok &= all(check_chars(x, f"ligne {n}") for x in f)
        counts[section] = counts.get(section, 0) + 1
        i = counts[section]
        kind = {"vocab": "vocab", "phrase": "phrase", "grammar": "grammar", "alphabet": "alphabet"}[section]
        key = (kind, f[0])
        if key in seen:
            print(f"doublon ligne {n} : {f[0]}", file=sys.stderr)
            ok = False
        seen.add(key)
        card = [f"[[card]]", f"id = {q(f'{prefix}-{kind[:3]}-{i:03}')}", f"kind = {q(kind)}", f"front = {q(f[0])}"]
        if kind in ("vocab", "phrase"):
            if len(f) < 2:
                print(f"ligne {n} incomplète", file=sys.stderr); ok = False; continue
            ok &= check_stress(f[0], f"ligne {n}")
            card.append(f"back = {q(f[1])}")
            if len(f) > 2 and f[2]:
                card.append(f"note = {q(f[2])}")
        elif kind == "alphabet":
            card.append(f"back = {q(f[1])}")
            if len(f) > 2 and f[2]:
                card.append(f"note = {q(f[2])}")
        else:
            if len(f) < 4:
                print(f"ligne {n} incomplète", file=sys.stderr); ok = False; continue
            ok &= check_stress(f[0], f"ligne {n}")
            wrong = [w.strip() for w in f[3].split(";")]
            if f[2] in wrong or len(wrong) != 2:
                print(f"ligne {n} : mauvaises réponses invalides", file=sys.stderr); ok = False
            card.insert(4, f"question = {q(f[1])}")
            card.append(f"back = {q(f[2])}")
            card.append("wrong = [" + ", ".join(q(w) for w in wrong) + "]")
            if len(f) > 4 and f[4]:
                card.append(f"note = {q(f[4])}")
        card.append(f"level = {i}")
        out.append("\n".join(card))
    if tier is None:
        sys.exit("en-tête « # niveau = N » manquant")
    head = f"# Cartes du niveau {tier} de rusky (généré, prononciation calculée).\ntier = {tier}\n"
    open(dst, "w", encoding="utf-8").write(head + "\n" + "\n\n".join(out) + "\n")
    print(dst, counts, "OK" if ok else "AVEC ERREURS")
    if not ok:
        sys.exit(1)


main(*sys.argv[1:])
