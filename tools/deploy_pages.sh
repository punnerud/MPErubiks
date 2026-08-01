#!/usr/bin/env bash
# Build the app for GitHub Pages (served under /MPErubiks/) and push the
# dist to the gh-pages branch. Run from the repo root on a machine with
# trunk (the M3 in practice).
set -euo pipefail
trunk build --release --public-url /MPErubiks/
tmp=$(mktemp -d)
cp -r dist/. "$tmp"/
touch "$tmp/.nojekyll"
git -C "$tmp" init -q -b gh-pages
git -C "$tmp" add -A
git -C "$tmp" -c user.name="$(git config user.name || echo deploy)" \
    -c user.email="$(git config user.email || echo deploy@local)" \
    commit -q -m "Deploy MPErubiks to GitHub Pages"
git -C "$tmp" push -q --force https://github.com/punnerud/MPErubiks.git gh-pages
rm -rf "$tmp"
echo "deployed: https://punnerud.github.io/MPErubiks/"
