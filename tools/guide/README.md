# 使い方の動画を撮り直す

画面を変えたときに、`docs/media/` の動画を撮り直すためのものです。ブラウザ版を Playwright で操作して撮ります。

```
cd app && python3 -m http.server 8771 &          # app/dist を出しておく
cd tools/guide
export FONTCONFIG_FILE=$PWD/fonts.conf          # Linux で縦書きの字形を正しく出すため
python3 record.py http://127.0.0.1:8771/dist/ ../../sample out
python3 mobile.py http://127.0.0.1:8771/dist/ ../../sample out
./convert.sh                                     # out/media/ に webp と mp4 ができる
cp out/media/* ../../docs/media/
```

要るもの：Python の playwright（Chromium）、Pillow、ffmpeg（libwebp 入り）、日本語のフォント（Noto CJK）。
