#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo "La création du bundle macOS doit être exécutée sur un Mac." >&2
  exit 1
fi
command -v node >/dev/null
npm ci --prefix tools --no-audit --no-fund
cargo build --release --locked
bundle="$PWD/dist/MPTE.app"
if [[ -e "$bundle" ]]; then
  echo "dist/MPTE.app existe déjà. Déplacez-le avant de reconstruire." >&2
  exit 1
fi
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources/formatters/node/bin"
cp target/release/mpte "$bundle/Contents/MacOS/MPTE"
cp tools/format.mjs "$bundle/Contents/Resources/formatters/"
cp -R tools/node_modules "$bundle/Contents/Resources/formatters/"
node_binary="$(node -p 'process.execPath')"
# Homebrew Node can reference external dylibs. Require a standalone official
# Node binary so the bundle does not depend on the developer's installation.
if otool -L "$node_binary" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/usr/lib/|/System/Library/)' >/dev/null; then
  echo "Node dépend de bibliothèques externes. Utilisez la distribution officielle autonome de nodejs.org, puis relancez." >&2
  exit 1
fi
cp "$node_binary" "$bundle/Contents/Resources/formatters/node/bin/node"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>MPTE</string>
<key>CFBundleIdentifier</key><string>dev.mpte.editor</string>
<key>CFBundleName</key><string>MPTE</string>
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
