#!/usr/bin/env bash
# Build the browser (wasm32) version of shooty into ./web — a self-contained,
# static, servable folder: index.html + shooty.js + shooty_bg.wasm + assets/.
#
# This is deliberately a *separate* build from the native/dev one: it strips
# debug symbols, runs wasm-opt, and — most importantly — leaves the Quaternius
# skeletal cast (assets/models/quaternius/) out of assets/ entirely, since
# src/game/roster.rs only offers the boxy cast when compiled for wasm32. The
# native build still has the full roster and the full assets/ tree; nothing
# here touches those.
#
# One-time setup this expects: `rustup target add wasm32-unknown-unknown`,
# `cargo install wasm-bindgen-cli --version <matching src/../Cargo.lock's
# wasm-bindgen version>`, and `brew install binaryen` (for wasm-opt).

set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> cargo build --release --target wasm32-unknown-unknown"
cargo build --release --target wasm32-unknown-unknown --bin shooty

echo "==> wasm-bindgen"
rm -rf web
mkdir -p web
wasm-bindgen --no-typescript --out-dir web --target web \
  target/wasm32-unknown-unknown/release/shooty.wasm

echo "==> wasm-opt -Oz"
wasm-opt -Oz \
  --enable-bulk-memory --enable-sign-ext --enable-mutable-globals \
  --enable-nontrapping-float-to-int --enable-multivalue \
  --output web/shooty_bg.opt.wasm web/shooty_bg.wasm
mv web/shooty_bg.opt.wasm web/shooty_bg.wasm

echo "==> trimming assets for web (dropping quaternius + stray capture frames)"
rsync -a --delete \
  --exclude 'models/quaternius/' \
  --exclude 'models/city/screenshots/' \
  --exclude '.DS_Store' \
  assets/ web/assets/

cat > web/index.html <<'HTML'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Shooty</title>
  <style>
    html, body { margin: 0; height: 100%; background: #111; }
    canvas { display: block; margin: 0 auto; }
  </style>
</head>
<body>
  <script type="module">
    import init from "./shooty.js";
    init();
  </script>
</body>
</html>
HTML

echo "==> done"
du -sh web
echo "Serve locally with: python3 -m http.server 8765 --directory web"
