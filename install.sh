#!/usr/bin/env bash
# Installe rusky et lance le démon de popups.
#   ./install.sh            paquet Arch (makepkg -si) + service systemd utilisateur
#   ./install.sh --cargo    binaire dans ~/.cargo/bin, sans sudo
#   ./install.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

if [[ "${1:-}" == "--uninstall" ]]; then
  systemctl --user disable --now rusky.service 2>/dev/null || true
  rm -f ~/.config/systemd/user/rusky.service
  pacman -Qq rusky &>/dev/null && sudo pacman -R rusky
  cargo uninstall rusky 2>/dev/null || true
  systemctl --user daemon-reload
  say "rusky désinstallé (ta progression reste dans ~/.local/share/rusky)."
  exit 0
fi

if [[ "${1:-}" == "--cargo" ]] || ! command -v makepkg &>/dev/null; then
  say "Compilation avec cargo…"
  cargo install --locked --path .
  bin="$HOME/.cargo/bin/rusky"
  # Le service du dépôt pointe vers /usr/bin : on en pose une copie adaptée.
  mkdir -p ~/.config/systemd/user
  sed "s|/usr/bin/rusky|$bin|" systemd/rusky.service > ~/.config/systemd/user/rusky.service
else
  say "Construction du paquet Arch…"
  makepkg -si --noconfirm --needed
  rm -rf pkg src/target ./*.pkg.tar.zst  # srcdir de makepkg = ./src
fi

systemctl --user daemon-reload
if systemctl --user is-active --quiet graphical-session.target; then
  systemctl --user enable --now rusky.service
  systemctl --user restart rusky.service
  say "Démon lancé : première popup dans quelques instants (rusky now pour tester)."
else
  say "Pas de session systemd graphique : ajoute à ta config Hyprland"
  echo "    exec-once = rusky run"
fi
