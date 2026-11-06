#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo "La création du bundle macOS doit être exécutée sur un Mac." >&2
  exit 1
fi
bundle="$PWD/dist/MPTE.app"
if [[ -e "$bundle" ]]; then
  echo "dist/MPTE.app existe déjà. Déplacez-le avant de reconstruire." >&2
  exit 1
fi
if ! node_binary="$(node -p 'process.execPath')"; then
  echo "Node.js est requis. Activez une distribution autonome de Node.js avant de relancer." >&2
  exit 1
fi
# The bundle only includes Node itself, so its dylibs must be provided by macOS.
if ! linked_libraries="$(otool -L "$node_binary")"; then
  printf 'Impossible de vérifier les bibliothèques de Node.js (%s).\n' "$node_binary" >&2
  exit 1
fi
external_libraries="$(printf '%s\n' "$linked_libraries" | awk '
  /^[[:space:]]/ && NF && $1 !~ /^\/(usr\/lib|System\/Library)\// {
    sub(/^[[:space:]]+/, "")
    sub(/[[:space:]]+\(compatibility version .*$/, "")
    print
  }
')"
if [[ -n "$external_libraries" ]]; then
  printf 'Node.js (%s) dépend de bibliothèques externes :\n%s\n' "$node_binary" "$external_libraries" >&2
  echo "Activez le binaire officiel de nodejs.org, par exemple avec nvm install et nvm use depuis le dépôt." >&2
  exit 1
fi
npm ci --prefix tools --no-audit --no-fund
cargo build --release --locked
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources/formatters/node/bin"
cp target/release/mpte "$bundle/Contents/MacOS/MPTE"
cp assets/MPTE.icns "$bundle/Contents/Resources/"
cp tools/format.mjs "$bundle/Contents/Resources/formatters/"
cp -R tools/node_modules "$bundle/Contents/Resources/formatters/"
cp "$node_binary" "$bundle/Contents/Resources/formatters/node/bin/node"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>MPTE</string>
<key>CFBundleIdentifier</key><string>dev.mpte.editor</string>
<key>CFBundleName</key><string>MPTE</string>
<key>CFBundleIconFile</key><string>MPTE.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist"
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
echo "Bundle local créé : $bundle (architecture du Mac actuel, signature ad hoc)."
