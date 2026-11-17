#!/usr/bin/env sh
# Regenera los iconos (PNG e ICO) a partir de assets/logo.svg. Requiere rsvg-convert y ImageMagick.
set -eu
cd "$(dirname "$0")/../assets"
for n in 16 24 32 48 64 128 256 512; do
  rsvg-convert -w "$n" -h "$n" logo.svg -o "icon-$n.png"
done
magick icon-16.png icon-24.png icon-32.png icon-48.png icon-64.png icon-128.png icon-256.png -compress zip icon.ico
cp icon-256.png icon.png
echo "Iconos generados en assets/"
