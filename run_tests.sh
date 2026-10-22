#!/bin/bash
# 変換の本体が Python 版と同じ結果を出すかを確かめる（Python 版のテストで記録した 75 例など）
set -euo pipefail
cd "$(dirname "$0")"
cargo test -p egword_core
