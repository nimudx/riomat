#!/usr/bin/env sh
# Instala Riomat en Linux con su icono y entrada en el menú de aplicaciones.
#   scripts/install-linux.sh                # para tu usuario (~/.local), sin sudo
#   sudo PREFIX=/usr/local scripts/install-linux.sh   # para todo el sistema
#   scripts/install-linux.sh --desinstalar
# Variables: PREFIX (por defecto ~/.local), SIN_COMPILAR=1 usa target/release/riomat ya compilado.
set -eu
cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
APPS="$PREFIX/share/applications"
ICONOS="$PREFIX/share/icons/hicolor"

if [ "${1:-}" = "--desinstalar" ]; then
  rm -f "$BIN/riomat" "$APPS/riomat.desktop" "$ICONOS/scalable/apps/riomat.svg"
  for n in 16 24 32 48 64 128 256 512; do rm -f "$ICONOS/${n}x${n}/apps/riomat.png"; done
  echo "Riomat desinstalado de $PREFIX"
  exit 0
fi

if [ "${SIN_COMPILAR:-}" != "1" ]; then
  cargo build -p riomat --release
fi
[ -x target/release/riomat ] || { echo "No existe target/release/riomat; compila primero." >&2; exit 1; }

install -Dm755 target/release/riomat "$BIN/riomat"
install -Dm644 packaging/linux/riomat.desktop "$APPS/riomat.desktop"
install -Dm644 assets/logo.svg "$ICONOS/scalable/apps/riomat.svg"
for n in 16 24 32 48 64 128 256 512; do
  install -Dm644 "assets/icon-$n.png" "$ICONOS/${n}x${n}/apps/riomat.png"
done
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$ICONOS" 2>/dev/null || true
echo "Riomat instalado en $PREFIX"
echo "Búscalo como «Riomat» en el menú de aplicaciones o ejecuta: riomat"
case ":$PATH:" in *":$BIN:"*) ;; *) echo "Aviso: $BIN no está en tu PATH." ;; esac
