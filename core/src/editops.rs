//! エディタの中身（editops.py）：注記の組み立て・頁の位置・警告の位置
//! 位置は、画面の側（JavaScript）に合わせて UTF-16 で数える

use crate::convert::normalize;
use crate::py::*;
use indexmap::IndexMap;
use std::sync::LazyLock;

fn is_kanji(c: char) -> bool {
    matches!(c as u32, 0x3005 | 0x3007 | 0x303B | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3FFFF)
}

pub fn fullwidth(n: i64) -> String {
    n.to_string().chars().map(|c| match c.to_digit(10) { Some(d) => char::from_u32(0xFF10 + d).unwrap(), None => c }).collect()
}

/// ルビの注記。親文字が漢字だけで、直前が漢字でなければ「｜」を省く
pub fn ruby(base: &str, reading: &str, prev: &str) -> String {
    let all_kanji = !base.is_empty() && base.chars().all(is_kanji);
    let prev_kanji = prev.chars().last().map(is_kanji).unwrap_or(false);
    if all_kanji && !prev_kanji { format!("{}《{}》", base, reading) } else { format!("｜{}《{}》", base, reading) }
}

/// 行内の飾り：名前 → (後ろから付ける形の述語, 範囲の始め, 範囲の終わり)
pub fn decoration_forms(name: &str) -> Option<(String, String, String)> {
    let (post, end) = match name {
        "傍点" | "白ゴマ傍点" | "丸傍点" | "白丸傍点" | "傍線" | "二重傍線" | "鎖線" | "破線" | "波線" =>
            (format!("に{}", name), format!("{}終わり", name)),
        "太字" | "斜体" | "縦中横" | "大見出し" | "中見出し" | "小見出し" => (format!("は{}", name), format!("{}終わり", name)),
        "１段階小さな文字" | "２段階小さな文字" => (format!("は{}", name), "小さな文字終わり".into()),
        "１段階大きな文字" | "２段階大きな文字" => (format!("は{}", name), "大きな文字終わり".into()),
        _ => return None,
    };
    Some((post, name.to_string(), end))
}

fn u16len(s: &str) -> usize { s.encode_utf16().count() }

/// 選んだ文字に飾りを付けた文字列と、カーソルを置く位置（UTF-16）
pub fn decorate(name: &str, selection: &str) -> (String, usize) {
    let (post, start, end) = decoration_forms(name).expect("知らない飾り");
    if !selection.is_empty() {
        let t = format!("{}［＃「{}」{}］", selection, selection, post);
        let n = u16len(&t);
        return (t, n);
    }
    let head = format!("［＃{}］", start);
    let n = u16len(&head);
    (format!("{}［＃{}］", head, end), n)
}

/// 選んだ行を範囲の注記で囲む
pub fn block(open: &str, close: &str, selection: &str) -> (String, usize) {
    if !selection.is_empty() {
        let body = if selection.ends_with('\n') { selection.to_string() } else { format!("{}\n", selection) };
        let t = format!("{}\n{}{}\n", open, body, close);
        let n = u16len(&t);
        return (t, n);
    }
    let head = format!("{}\n", open);
    (format!("{}\n{}\n", head, close), u16len(&head))
}

pub fn indent_block(first: i64, rest: Option<i64>) -> (String, String) {
    let mut parts = vec![format!("ここから{}字下げ", fullwidth(first))];
    if let Some(r) = rest { if r != first { parts.push(format!("折り返して{}字下げ", fullwidth(r))); } }
    (format!("［＃{}］", parts.join("、")), "［＃ここで字下げ終わり］".into())
}

pub fn gaiji(description: &str, code: &str) -> String {
    let c = code.trim().to_uppercase().replace("U+", "");
    if c.is_empty() { format!("※［＃「{}」］", description) } else { format!("※［＃「{}」、U+{}］", description, c) }
}

pub fn figure(img: &str, caption: &str) -> String {
    let cap = caption.trim().replace('\n', "／");
    if cap.is_empty() { format!("［＃図（{}）入る］", img) } else { format!("［＃図（{}）入る：{}］", img, cap) }
}

pub fn opener(img: &str, title: &str) -> String {
    if title.trim().is_empty() { format!("［＃章扉（{}）］", img) } else { format!("［＃章扉（{}）：{}］", img, title) }
}

pub fn separator(style: &str) -> String {
    if style.is_empty() || style == "実線" { "［＃区切り線］".into() } else { format!("［＃区切り線（{}）］", style) }
}

/// 各頁の始まりの行番号（1 から）
pub fn page_start_lines(text: &str) -> Vec<usize> {
    let norm = normalize(text);
    let mut starts = vec![1];
    let mut line = 1;
    for c in norm.chars() {
        if c == '\n' { line += 1; } else if c == '\u{0C}' { starts.push(line); }
    }
    starts
}

pub fn page_of_line(starts: &[usize], line: usize) -> usize {
    let mut page = 0;
    for (i, s) in starts.iter().enumerate() { if *s <= line { page = i; } }
    page
}

/// その頁から作った出力ファイル。なければ前の頁のもの
pub fn file_for_page(src_pages: &IndexMap<String, usize>, page: usize) -> Option<String> {
    let mut names: Vec<&String> = src_pages.keys().collect();
    names.sort();
    let mut best: Option<&String> = None;
    for f in names {
        let p = src_pages[f];
        if p == page { return Some(f.clone()); }
        if p < page && best.map(|b| src_pages[b] <= p).unwrap_or(true) { best = Some(f); }
    }
    best.cloned()
}

static NEEDLES: LazyLock<Vec<Rx>> = LazyLock::new(|| vec![
    Rx::new("「([^「」]+)」"), Rx::new("(［＃[^］]+］)"), Rx::new("注 (\\S+) を"), Rx::new("：(\\S+)$"),
]);

/// 警告の場所を (行, 列の始め, 列の終わり) で返す（列は UTF-16）。列が分からなければ (行, 0, 0)
pub fn locate_warning(text: &str, src_pages: &IndexMap<String, usize>, file: &str, msg: &str) -> Option<(usize, usize, usize)> {
    let page = *src_pages.get(file)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let starts = page_start_lines(text);
    let first = starts[page];
    let last = if page + 1 < starts.len() { starts[page + 1] - 1 } else { lines.len() };
    for rx in NEEDLES.iter() {
        for m in rx.finditer(msg) {
            let mut needle = g(&m, 1).unwrap().trim().to_string();
            if needle.contains('…') { needle = needle.split('…').next().unwrap().to_string(); }
            if needle.is_empty() { continue; }
            for ln in first..=last.max(first) {
                if ln == 0 || ln > lines.len() { continue; }
                if let Some(col) = lines[ln - 1].find(&needle) {
                    let a = u16len(&lines[ln - 1][..col]);
                    return Some((ln, a, a + u16len(&needle)));
                }
            }
        }
    }
    Some((first, 0, 0))
}
