//! コマンドで変換する
//!   egword2epub-rs 入力.txt 画像フォルダ 出力.epub [--config 設定.toml] [--css 体裁.css]

use egword_core::*;
use std::path::{Path, PathBuf};
use std::process::exit;

/// いまの時刻（UTC）を 2026-10-21T00:00:00Z の形で
fn now_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // 日数 → 年月日（Howard Hinnant の方法）
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

fn fail(msg: &str) -> ! {
    eprintln!("変換できませんでした：{}", msg);
    exit(1)
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut config: Option<String> = None;
    let mut css: Option<String> = None;
    let mut pos: Vec<String> = Vec::new();
    while !args.is_empty() {
        let a = args.remove(0);
        match a.as_str() {
            "--config" if !args.is_empty() => config = Some(args.remove(0)),
            "--css" if !args.is_empty() => css = Some(args.remove(0)),
            "-h" | "--help" => {
                println!("使い方：egword2epub-rs 入力.txt 画像フォルダ 出力.epub [--config 設定.toml] [--css 体裁.css]");
                return;
            }
            _ => pos.push(a),
        }
    }
    if pos.len() != 3 {
        eprintln!("使い方：egword2epub-rs 入力.txt 画像フォルダ 出力.epub [--config 設定.toml] [--css 体裁.css]");
        exit(2);
    }
    let (src, imgdir, out) = (&pos[0], &pos[1], &pos[2]);
    let data = std::fs::read(src).unwrap_or_else(|e| fail(&format!("{}：{}", src, e)));
    let (text, enc, _) = decode_text(&data).unwrap_or_else(|e| fail(&e.0));

    let conf = match &config {
        Some(p) => {
            let t = std::fs::read_to_string(p).unwrap_or_else(|e| fail(&format!("{}：{}", p, e)));
            parse_config(&t).unwrap_or_else(|e| fail(&e.0))
        }
        None => Table::new(),
    };
    // CSS：--css → 設定の css（設定ファイルの場所から見た位置）→ 同梱のもの
    let css_path: Option<PathBuf> = css.map(PathBuf::from).or_else(|| {
        let c = conf.get("css").and_then(|v| v.as_str()).filter(|s| !s.is_empty())?;
        let base = Path::new(config.as_deref().unwrap()).parent().unwrap_or(Path::new("."));
        Some(base.join(c))
    });
    let css_text = css_path.map(|p| std::fs::read_to_string(&p).unwrap_or_else(|e| fail(&format!("{}：{}", p.display(), e))));

    let title = Path::new(src).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let images = DirImages(PathBuf::from(imgdir));
    let modified = now_utc();
    let input = ConvertInput {
        text: &text, encoding: enc, config: &conf, css: css_text.as_deref(), fallback_title: &title,
        images: &images, modified: &modified,
    };
    let book = build_book(&input).unwrap_or_else(|e| fail(&e.0));
    std::fs::write(out, book.to_epub(&images)).unwrap_or_else(|e| fail(&format!("{}：{}", out, e)));
    let label = if enc == TextEncoding::Cp932 { "Shift_JIS（CP932）" } else { enc.label() };
    println!("{}：{} ファイル、画像 {} 点（入力の文字コード {}）", out, book.files.len(), book.images.len(), label);
    if !book.warnings.is_empty() {
        eprintln!("警告 {} 件（変換は済んでいます）：", book.warnings.len());
        for w in &book.warnings {
            eprintln!("  {}：{}", if w.file.is_empty() { "-" } else { &w.file }, w.message);
        }
    }
}
