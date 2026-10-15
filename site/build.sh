#!/bin/sh
# Puts the site and the images and font it uses into the folder given ($1),
# which GitHub Pages serves next to the APT repository.
set -eu
out="$1"
here="$(cd "$(dirname "$0")" && pwd)"
root="$here/.."
mkdir -p "$out/img" "$out/fonts"
cp "$here/index.html" "$here/style.css" "$out/"
cp "$root/static/app-icon.png" "$out/img/"
cp "$root/static/fonts/Inter-Variable.woff2" "$out/fonts/"
for shot in overview chart library; do
  cp "$root/manual/screenshots/$shot.png" "$out/img/"
done
