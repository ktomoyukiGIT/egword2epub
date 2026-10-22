#!/bin/bash
# Mac のアプリ（Egword2EPUB.app）を作る。ターミナルでこのフォルダに移って ./build_mac.sh
# 要るもの：Rust（https://rustup.rs の一行で入る）と、Xcode（またはコマンドラインツール）
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust が入っていません。次の一行をターミナルで実行してから、もう一度どうぞ："
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi
if [ ! -f app/dist/index.html ]; then
  echo "画面（app/dist）がありません。同梱のものを消していないか確かめてください。"
  exit 1
fi

echo "== 組み立てています（初めては 5〜15 分ほどかかります）"
cargo build --release -p egword2epub-desktop

APP="build/Egword2EPUB.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/ja.lproj"
cp target/release/Egword2EPUB "$APP/Contents/MacOS/Egword2EPUB"
cp mac/Info.plist "$APP/Contents/Info.plist"
printf '/* 日本語のアプリとして扱わせるためのもの */\n' > "$APP/Contents/Resources/ja.lproj/InfoPlist.strings"

echo "== アイコン"
ICONSET="build/AppIcon.iconset"
rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s desktop/icons/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d desktop/icons/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

echo "== 署名（この Mac で使うためのもの）"
codesign --force --deep --sign - "$APP"

echo
echo "できました：$(pwd)/$APP"
echo "「アプリケーション」フォルダに移して使ってください（Finder で開くには：open build）"
