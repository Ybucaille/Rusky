# rusky 🇷🇺

Une popup toutes les 25 minutes pour apprendre le russe : un mot, une expression, une lettre ou une règle de grammaire. Tu choisis la bonne réponse parmi trois, et la répétition espacée (façon Anki) décide quand la carte revient.

965 cartes réparties en 6 niveaux, avec les accents toniques et la prononciation :

| Niveau | Thème | Cartes |
|---|---|---|
| 1 | Premiers pas : l'alphabet complet, les premiers mots | 137 |
| 2 | Le quotidien : nombres, jours, repas, famille, couleurs | 184 |
| 3 | En ville : transports, courses, restaurant, chemin | 165 |
| 4 | Raconter : le passé, le corps, la météo, les sentiments | 163 |
| 5 | Discuter : donner son avis, travail, études, loisirs | 160 |
| 6 | Nuances : expressions idiomatiques, proverbes, préfixes | 156 |

## Niveaux et XP

Chaque réponse rapporte de l'XP : +10 si elle est juste, +2 sinon (l'effort compte). Les nouvelles cartes ne viennent que des niveaux débloqués. Le niveau suivant s'ouvre quand 70 % des cartes du niveau en cours sont acquises (réussies deux fois d'affilée) ; une popup te prévient. `rusky stats` montre ton XP et où tu en es.

## Installation sur Arch

```bash
./install.sh            # paquet Arch + service systemd lancé
./install.sh --cargo    # sans sudo, binaire dans ~/.cargo/bin
./install.sh --uninstall
```

À la main : `makepkg -si`

Ou sans paquet : `cargo install --path .` (le binaire va dans `~/.cargo/bin`).

## Lancement sous Hyprland

Le plus simple, dans `~/.config/hypr/hyprland.conf` :

```
exec-once = rusky run
```

Si tu utilises uwsm (ou une session avec `graphical-session.target`), tu peux passer par systemd à la place :

```bash
systemctl --user enable --now rusky.service
```

## La popup

Rusky affiche sa propre popup (GTK4 + layer-shell) en haut à droite de l'écran, par-dessus les fenêtres, sans passer par le démon de notifications. Il faut un compositeur qui gère `wlr-layer-shell` (Hyprland, Sway, niri…).

Chaque carte est une question à choix multiple : tu cliques sur une des trois réponses, la popup te dit si c'est juste et affiche l'explication, puis « Continuer ». Une bonne réponse espace la carte, une mauvaise la fait revenir vite.

Sous chaque mot russe, sa prononciation à la française, avec la syllabe sur laquelle on appuie en MAJUSCULES : молоко́ se lit « ma-la-KO ».

« Plus tard », le ✕ ou Échap ferment sans noter. Sans réponse, la popup se referme seule au bout de 45 s (la fine barre en bas montre le temps restant et se met en pause quand ta souris est sur la carte) : la carte est alors reportée comme avec « Plus tard ». Tant que tu n'as pas répondu, aucune autre popup n'arrive.

## Commandes

| Commande | Effet |
|---|---|
| `rusky run` | Lance le démon |
| `rusky now` | Une carte tout de suite |
| `rusky review -c 20` | Le même quiz dans le terminal |
| `rusky stats` | Progression, réussite du jour, série |
| `rusky pause 60` | Pause d'une heure (`rusky resume` pour reprendre) |
| `rusky add "кот" "chat" -p "kot"` | Ajoute ta carte (`-n` explication, `-k grammar`, `phrase`, `alphabet`) |
| `rusky import mes-cartes.toml` | Importe un paquet de cartes |
| `rusky paths` | Où sont la config et la base |

Astuce Hyprland : `bind = SUPER, R, exec, rusky now` pour une carte à la demande.

## Configuration

`~/.config/rusky/config.toml`, relu à chaque popup (pas besoin de relancer) :

```toml
interval_minutes = 25   # écart entre deux popups
active_start = 9        # pas de popups avant 9 h
active_end = 23         # ni après 23 h (peut passer minuit : 18 → 2)
max_new_per_day = 8     # nouvelles cartes par jour
kinds = ["alphabet", "vocab", "grammar", "phrase"]
```

## Ajouter tes propres cartes

La prononciation est calculée toute seule à partir du mot accentué (`pron` reste possible pour corriger une exception).

Même format que `data/cards.toml`. Le caractère ´ des accents se tape avec la touche morte, ou copie-colle-le.

```toml
[[card]]
id = "perso-cuisine-001"   # unique, sert à garder ta progression
kind = "vocab"
front = "сыр"
back = "fromage"
note = "Я люблю́ сыр (ya liou-BLIOU syr) = J'aime le fromage."
level = 1                  # plus petit = apparaît plus tôt
# Optionnels : question = "…" (la consigne) et wrong = ["…", "…"] (les mauvaises
# réponses ; sinon elles sont tirées des autres cartes du même type).
```

Réimporter un fichier met à jour le texte des cartes sans perdre ta progression.

## Comment marche la répétition

- Bonne réponse → revient dans 4 h, puis 1 jour, 3 jours, puis l'intervalle est multiplié (×2,5 au départ).
- Mauvaise réponse → revient dans 10 minutes, et deviendra un peu plus fréquente à l'avenir.
- Une fois connue, une carte de vocabulaire ou d'expression est parfois posée à l'envers (français → russe).

Les données sont dans `~/.local/share/rusky/rusky.db` (SQLite).

## Modifier le paquet intégré

Les cartes intégrées s'écrivent dans `data/src/niveau-N.txt`, une carte par ligne (`russe accentué | sens | note`), puis se convertissent avec :

```bash
python3 data/src/build.py data/src/niveau-2.txt data/niveau-2.toml t2
```

Le script refuse un mot de plusieurs syllabes sans accent tonique, un doublon ou une question de grammaire mal formée.
