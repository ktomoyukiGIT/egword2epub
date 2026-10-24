#!/bin/bash
# Linux のアプリを作り、自分のメニューに加える。ターミナルでこのフォルダに移って ./build_linux.sh
# 要るもの：Rust と、WebKitGTK などの部品（README の「Linux のアプリ」を参照）
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
if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  echo "WebKitGTK（webkit2gtk-4.1）の開発用の部品が見つかりません。README の「Linux のアプリ」の一行を先に実行してください。"
  exit 1
fi

echo "== 組み立てています（初めては 5〜15 分ほどかかります）"
cargo build --release -p egword2epub-desktop

mkdir -p build
cp target/release/Egword2EPUB build/Egword2EPUB
echo "== できました：build/Egword2EPUB"

# メニューに加える（自分のホームの中だけ。管理者の権限は要らない）
BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$BIN" "$APPS" "$ICONS"
cp build/Egword2EPUB "$BIN/egword2epub"
if command -v convert >/dev/null 2>&1; then
  convert desktop/icons/icon-1024.png -resize 256x256 "$ICONS/egword2epub.png"
else
  cp desktop/icons/icon.png "$ICONS/egword2epub.png"
fi
cat > "$APPS/egword2epub.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Egword2EPUB
Comment=原稿のエディタと縦書きの EPUB への変換
Exec=$BIN/egword2epub %f
Icon=egword2epub
Terminal=false
Categories=Office;TextEditor;
MimeType=text/plain;
DESKTOP
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" >/dev/null 2>&1 || true
echo "== アプリのメニューに「Egword2EPUB」を加えました（$BIN/egword2epub）"
