#!/bin/bash
# webm → GIF（説明に埋め込む）と、通しの mp4
set -euo pipefail
cd "${1:-$(dirname "$0")/out}"
mkdir -p media
for f in 0*.webm; do
  n=${f%.webm}
  ffmpeg -v error -y -i $f -vf "fps=10,scale=1024:-1:flags=lanczos" -c:v libwebp_anim -lossless 0 -q:v 70 -compression_level 6 -loop 0 media/$n.webp
done
: > list.txt
for f in 0*.webm; do echo "file '$PWD/$f'" >> list.txt; done
ffmpeg -v error -y -f concat -safe 0 -i list.txt -c:v libx264 -preset slow -crf 26 -pix_fmt yuv420p -r 30 -movflags +faststart -an media/tutorial.mp4
cp 06-mobile.png media/
ls -la media
