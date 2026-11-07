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
node_major="$(tr -d '[:space:]' < .nvmrc)"
if [[ ! "$node_major" =~ ^[0-9]+$ ]]; then
  echo ".nvmrc doit indiquer la version majeure de Node.js à embarquer." >&2
  exit 1
fi
case "$(uname -m)" in
  x86_64) node_arch=x64 ;;
  arm64) node_arch=arm64 ;;
  *)
    echo "Architecture macOS non prise en charge." >&2
    exit 1
    ;;
esac
node_cache="$PWD/target/node-macos"
mkdir -p "$node_cache"
node_work="$(mktemp -d "$node_cache/.download.XXXXXX")"
bundle_incomplete=false
cleanup() {
  rm -rf "$node_work"
  if [[ "$bundle_incomplete" == true ]]; then
    rm -rf "$bundle"
  fi
}
trap cleanup EXIT

echo "Préparation du Node.js officiel pour le bundle…"
curl --fail --location --silent --show-error --retry 3 \
  "https://nodejs.org/dist/latest-v$node_major.x/SHASUMS256.txt" \
  --output "$node_work/SHASUMS256.txt"
if ! checksum_line="$(awk -v major="$node_major" -v arch="$node_arch" '
  $2 ~ "^node-v" major "\\.[0-9]+\\.[0-9]+-darwin-" arch "\\.tar\\.gz$" {
    checksum = $0
    count++
  }
  END {
    if (count != 1) exit 1
    print checksum
  }
' "$node_work/SHASUMS256.txt")"; then
  echo "Archive officielle de Node.js introuvable pour ce Mac." >&2
  exit 1
fi
read -r node_checksum node_archive <<< "$checksum_line"
if [[ ! "$node_checksum" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Empreinte SHA-256 de Node.js invalide." >&2
  exit 1
fi
node_version="${node_archive#node-}"
node_version="${node_version%-darwin-*}"
cached_archive="$node_cache/$node_archive"
printf '%s  %s\n' "$node_checksum" "$cached_archive" > "$node_work/cached.sha256"
if ! shasum -a 256 --check "$node_work/cached.sha256" >/dev/null 2>&1; then
  echo "Téléchargement de Node.js $node_version ($node_arch)…"
  curl --fail --location --silent --show-error --retry 3 \
    "https://nodejs.org/dist/$node_version/$node_archive" \
    --output "$node_work/$node_archive"
  printf '%s  %s\n' "$node_checksum" "$node_work/$node_archive" > "$node_work/download.sha256"
  shasum -a 256 --check "$node_work/download.sha256"
  mv -f "$node_work/$node_archive" "$cached_archive"
fi
tar -xzf "$cached_archive" -C "$node_work"
node_dir="$node_work/${node_archive%.tar.gz}"
node_binary="$node_dir/bin/node"
if [[ "$("$node_binary" --version)" != "$node_version" ]]; then
  echo "La version du Node.js extrait ne correspond pas à son archive." >&2
  exit 1
fi
# Use this Node for npm and its subprocesses as well as for the final bundle.
export PATH="$node_dir/bin:$PATH"
# The bundle only includes Node itself, so its dylibs must be provided by macOS.
linked_libraries="$(otool -L "$node_binary")"
external_libraries="$(printf '%s\n' "$linked_libraries" | awk '
  /^[[:space:]]/ && NF && $1 !~ /^\/(usr\/lib|System\/Library)\// { print }
')"
if [[ -n "$external_libraries" ]]; then
  printf 'Le Node.js officiel référence des bibliothèques externes :\n%s\n' "$external_libraries" >&2
  exit 1
fi
"$node_binary" "$node_dir/lib/node_modules/npm/bin/npm-cli.js" ci --prefix tools --no-audit --no-fund
MACOSX_DEPLOYMENT_TARGET=13.5 cargo build --release --locked
mkdir -p "$PWD/dist"
mkdir "$bundle"
bundle_incomplete=true
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources/formatters/node/bin"
cp target/release/mpte "$bundle/Contents/MacOS/MPTE"
cp assets/MPTE.icns "$bundle/Contents/Resources/"
cp tools/format.mjs "$bundle/Contents/Resources/formatters/"
cp -R tools/node_modules "$bundle/Contents/Resources/formatters/"
cp "$node_binary" "$bundle/Contents/Resources/formatters/node/bin/node"
cp "$node_dir/LICENSE" "$bundle/Contents/Resources/Node-LICENSE.txt"
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
<key>LSMinimumSystemVersion</key><string>13.5</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist"
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
bundle_incomplete=false
echo "Bundle local créé : $bundle (architecture du Mac actuel, signature ad hoc)."
