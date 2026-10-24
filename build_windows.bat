@echo off
rem Windows のアプリ（build\Egword2EPUB.exe）を作る。このファイルをダブルクリックするか、このフォルダで build_windows.bat
rem 要るもの：Rust と、Microsoft C++ Build Tools（README の「Windows のアプリ」を参照）
chcp 65001 >nul
cd /d "%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust が入っていません。README の「Windows のアプリ」のとおりに入れてから、もう一度どうぞ。
  pause
  exit /b 1
)

echo == 組み立てています（初めては 5～15 分ほどかかります）
cargo build --release -p egword2epub-desktop
if errorlevel 1 (
  echo.
  echo 組み立てに失敗しました。上の赤い error: の数行を知らせてください。
  pause
  exit /b 1
)

if not exist build mkdir build
copy /y target\release\Egword2EPUB.exe build\Egword2EPUB.exe >nul
echo.
echo == できました：build\Egword2EPUB.exe
echo このファイルを好きな場所に置き、ダブルクリックで起動します。
pause
