//! 入力テキストの読み込み（reader.py）と、変換全体の流れ（cli.py の convert）

use crate::context::*;
use crate::epub::{assemble, Book};
use crate::py::*;
use crate::structure::build_pages;
use std::sync::LazyLock;

pub const DEFAULT_CSS: &str = include_str!("default.css");

static PAGEBREAK_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("(?m)^[ \\t　]*［＃改(?:ページ|丁|段|見開き)］[ \\t　]*$"));

/// 改行を LF にそろえ、一行の［＃改ページ］を改ページ記号（\f）にする
pub fn normalize(text: &str) -> String {
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    PAGEBREAK_RE.sub(&t, "\u{0C}")
}

pub fn split_pages(raw: &str) -> Vec<&str> { raw.split('\u{0C}').collect() }

/// 文字コード（Python 版と同じ呼び方）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextEncoding { Utf8, Utf16, Cp932 }

impl TextEncoding {
    pub fn name(self) -> &'static str { match self { Self::Utf8 => "utf-8", Self::Utf16 => "utf-16", Self::Cp932 => "cp932" } }
    pub fn label(self) -> &'static str { match self { Self::Utf8 => "UTF-8", Self::Utf16 => "UTF-16", Self::Cp932 => "Shift_JIS" } }
    pub fn from_name(s: &str) -> Self { match s { "cp932" | "shift_jis" => Self::Cp932, "utf-16" => Self::Utf16, _ => Self::Utf8 } }
}

/// バイト列を文字列に。BOM 付き UTF-8 / UTF-16 → UTF-8 → CP932（Shift_JIS）の順に試す。返り値は (文字列, 文字コード, BOM があったか)
pub fn decode_text(data: &[u8]) -> Result<(String, TextEncoding, bool), ConvertError> {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(&data[3..]) { return Ok((s.to_string(), TextEncoding::Utf8, true)); }
    }
    if data.starts_with(&[0xFF, 0xFE]) || data.starts_with(&[0xFE, 0xFF]) {
        let enc = if data[0] == 0xFF { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
        if let Some(s) = enc.decode_without_bom_handling_and_without_replacement(&data[2..]) {
            return Ok((s.into_owned(), TextEncoding::Utf16, true));
        }
    }
    if let Ok(s) = std::str::from_utf8(data) { return Ok((s.to_string(), TextEncoding::Utf8, false)); }
    if let Some(s) = encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(data) {
        return Ok((s.into_owned(), TextEncoding::Cp932, false));
    }
    Err(ConvertError("入力テキストの文字コードが分かりません（UTF-8 でも Shift_JIS でもない）。".into()))
}

/// 文字列をバイト列に。書けない字があれば、その字を返す
pub fn encode_text(text: &str, enc: TextEncoding, bom: bool) -> Result<Vec<u8>, String> {
    match enc {
        TextEncoding::Utf8 => {
            let mut v = if bom { vec![0xEF, 0xBB, 0xBF] } else { vec![] };
            v.extend_from_slice(text.as_bytes());
            Ok(v)
        }
        TextEncoding::Utf16 => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() { v.extend_from_slice(&u.to_le_bytes()); }
            Ok(v)
        }
        TextEncoding::Cp932 => {
            let mut enc = encoding_rs::SHIFT_JIS.new_encoder();
            let mut out = vec![0u8; enc.max_buffer_length_from_utf8_without_replacement(text.len()).unwrap_or(text.len() * 2 + 16)];
            let (res, _read, written) = enc.encode_from_utf8_without_replacement(text, &mut out, true);
            match res {
                encoding_rs::EncoderResult::InputEmpty => { out.truncate(written); Ok(out) }
                encoding_rs::EncoderResult::Unmappable(c) => Err(c.to_string()),
                encoding_rs::EncoderResult::OutputFull => Err(String::new()),
            }
        }
    }
}

// ---------------------------------------------------------------- 設定

pub(crate) const KNOWN_KEYS: [&str; 10] = ["book", "notes", "css", "writing_mode", "guess_periodicals", "autolink",
                                "chapter_image", "cover_image", "squeeze_blank_lines", "colophon"];

/// 設定ファイル（TOML）を読む
pub fn parse_config(text: &str) -> Result<Table, ConvertError> {
    text.parse::<Table>().map_err(|e| ConvertError(format!("設定ファイルの書き方に誤りがあります：{}", e.message())))
}

fn options(conf: &Table) -> Result<(bool, std::collections::HashSet<String>), ConvertError> {
    let mut unknown: Vec<String> = conf.keys().filter(|k| !KNOWN_KEYS.contains(&k.as_str())).cloned().collect();
    if !unknown.is_empty() {
        unknown.sort();
        return Err(ConvertError(format!("設定ファイルに知らない項目があります：{}", unknown.join("、"))));
    }
    let mode = get_str(conf, "writing_mode").unwrap_or_else(|| "vertical".into());
    if mode != "vertical" && mode != "horizontal" {
        return Err(ConvertError("writing_mode は \"vertical\"（縦書き）か \"horizontal\"（横書き）にしてください。".into()));
    }
    let links: std::collections::HashSet<String> = match conf.get("autolink") {
        Some(Value::Array(a)) => a.iter().map(vstr).collect(),
        _ => ["url", "mail"].iter().map(|s| s.to_string()).collect(),
    };
    let mut bad: Vec<String> = links.iter().filter(|k| !["url", "mail", "x"].contains(&k.as_str())).cloned().collect();
    if !bad.is_empty() {
        bad.sort();
        return Err(ConvertError(format!("autolink に使えるのは url・mail・x です：{}", bad.join("、"))));
    }
    Ok((mode == "horizontal", links))
}

// ---------------------------------------------------------------- 変換

pub struct ConvertInput<'a> {
    pub text: &'a str,               // 正規化する前のテキスト
    pub encoding: TextEncoding,
    pub config: &'a Table,
    /// CSS の中身（--css か、設定の css を読んだもの）。None なら同梱の CSS
    pub css: Option<&'a str>,
    pub fallback_title: &'a str,     // 書名がないときに使う（ファイル名）
    pub images: &'a dyn ImageSource,
    pub modified: &'a str,           // dcterms:modified（2026-10-21T00:00:00Z の形）
}

/// テキストから EPUB の中身を作る
pub fn build_book(input: &ConvertInput) -> Result<Book, ConvertError> {
    let conf = input.config;
    let (horizontal, links) = options(conf)?;
    let raw = normalize(input.text);
    let mut ctx = Context::new(input.images, build_systems(conf)?);
    ctx.horizontal = horizontal;
    ctx.autolink = links;
    if get_truthy(conf, "guess_periodicals") { ctx.periodicals = guess_periodicals(&raw); }
    ctx.encoding = input.encoding.name().into();
    let pages = split_pages(&raw);
    let (files, nav) = build_pages(&mut ctx, &raw, &pages, conf, input.fallback_title)?;
    let empty = Table::new();
    let book = get_table(conf, "book").unwrap_or(&empty);
    Ok(assemble(&mut ctx, files, nav, book, input.css.unwrap_or(DEFAULT_CSS), input.fallback_title, input.modified))
}
