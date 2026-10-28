# Paquet local : lance `makepkg -si` depuis ce dossier.
pkgname=rusky
pkgver=0.1.0
pkgrel=1
pkgdesc="Popups pour apprendre le russe (vocabulaire, grammaire, alphabet, expressions)"
arch=('x86_64' 'aarch64')
license=('MIT')
depends=('gcc-libs' 'gtk4' 'gtk4-layer-shell')
makedepends=('cargo')
options=('!lto')

build() {
  cd "$startdir"
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo build --release --locked
}

check() {
  cd "$startdir"
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo test --release --locked
}

package() {
  install -Dm755 "$srcdir/target/release/rusky" "$pkgdir/usr/bin/rusky"
  install -Dm644 "$startdir/systemd/rusky.service" "$pkgdir/usr/lib/systemd/user/rusky.service"
  install -Dm644 "$startdir/data/cards.toml" "$pkgdir/usr/share/rusky/cards-exemple.toml"
}
