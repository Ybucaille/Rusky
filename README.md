# rusky 🇷🇺

Une popup toutes les 25 minutes pour apprendre le russe : un mot, une expression, une lettre ou une règle de grammaire. Tu réponds « Je savais » ou « Pas su », et la répétition espacée (façon Anki) décide quand la carte revient.

126 cartes de départ (niveau débutant, accents toniques indiqués) :
alphabet (22), vocabulaire (63), grammaire (20), expressions (21).

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

## Les boutons des notifications

Rusky utilise les boutons d'action des notifications. Selon ton démon :

- **swaync** (recommandé) : les boutons s'affichent directement dans la popup.
- **mako** : les boutons ne sont pas affichés. Ajoute un raccourci dans Hyprland :
  `bind = SUPER, N, exec, makoctl menu -- wofi -d -p Rusky`
- **dunst** : ouvre le menu des actions avec le raccourci `context` (Ctrl+Shift+. par défaut).

Un clic sur la popup révèle la réponse. Tant que tu n'as pas répondu, aucune autre popup n'arrive : rien ne s'accumule.

## Commandes

| Commande | Effet |
|---|---|
| `rusky run` | Lance le démon |
| `rusky now` | Une carte tout de suite |
| `rusky review -c 20` | Session de révision dans le terminal |
| `rusky stats` | Progression, réussite du jour, série |
| `rusky pause 60` | Pause d'une heure (`rusky resume` pour reprendre) |
| `rusky add "кот" "chat" -n "Он спит."` | Ajoute ta carte (`-k grammar`, `phrase`, `alphabet`) |
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

Même format que `data/cards.toml`. Le caractère ´ des accents se tape avec la touche morte, ou copie-colle-le.

```toml
[[card]]
id = "perso-cuisine-001"   # unique, sert à garder ta progression
kind = "vocab"
front = "сыр"
back = "fromage"
note = "Я люблю́ сыр."
level = 1                  # plus petit = apparaît plus tôt
```

Réimporter un fichier met à jour le texte des cartes sans perdre ta progression.

## Comment marche la répétition

- Nouvelle carte sue → revient dans 4 h, puis 1 jour, 3 jours, puis l'intervalle est multiplié (×2,5 au départ).
- Pas sue → revient dans 10 minutes, et deviendra un peu plus fréquente à l'avenir.
- Une fois connue, une carte de vocabulaire est parfois posée à l'envers (français → russe).

Les données sont dans `~/.local/share/rusky/rusky.db` (SQLite).
