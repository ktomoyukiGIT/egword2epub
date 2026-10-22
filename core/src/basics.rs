//! 小道具（util.py）・雛形（templates.py）・見出し（headings.py）

use crate::py::*;
use std::sync::LazyLock;

pub const KANJI: &str = "[一-鿿㐀-䶿々〆ヶ〇]";
pub const SIZE_PATTERN: &str = "([０-９\\d]+)段階(小さな|大きな)文字";

/// 「１段階小さな文字」→ sho1、「２段階大きな文字」→ dai2（3 段階まで）
pub fn size_class(n: &str, kind: &str) -> String {
    format!("{}{}", if kind == "小さな" { "sho" } else { "dai" }, num(n).min(3))
}

pub fn decoration(name: &str) -> Option<&'static str> {
    Some(match name {
        "傍点" => "bouten", "白ゴマ傍点" => "bouten-wsesame", "丸傍点" => "bouten-circle", "白丸傍点" => "bouten-wcircle",
        "傍線" => "bosen", "二重傍線" => "bosen-double", "鎖線" => "bosen-dotted", "破線" => "bosen-dashed",
        "波線" => "bosen-wave", "太字" => "bold", "斜体" => "italic", "縦中横" => "tcy",
        _ => return None,
    })
}

// ---------------------------------------------------------------- templates

pub fn xhtml(title: &str, body: &str, cls: &str) -> String {
    let c = if cls.is_empty() { String::new() } else { format!(" class=\"{}\"", cls) };
    let hcls = if cls == "figpage" || cls == "cover" { " class=\"hz\"" } else { "" };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"ja\" lang=\"ja\"{}>\n\
<head><meta charset=\"UTF-8\"/><title>{}</title>\n\
<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head>\n\
<body{}>\n{}\n</body></html>\n",
        hcls, esc(title), c, body
    )
}

pub const CONTAINER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
<rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles>\n\
</container>\n";

// ---------------------------------------------------------------- headings

pub fn level_of(s: &str) -> i64 { match s { "大" => 1, "中" => 2, _ => 3 } }
pub fn head_class(level: i64) -> &'static str {
    match level { 1 => "o-midashi", 2 => "naka-midashi", _ => "ko-midashi" }
}

pub static INLINE_HEAD_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃(大|中|小)見出し］(.*?)［＃\\1見出し終わり］"));
pub static POSTFIX_HEAD_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃「([^」]+)」は(大|中|小)見出し］"));
pub static BLOCK_HEAD_START_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃ここから(大|中|小)見出し］"));
pub static BLOCK_HEAD_END_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃ここで(大|中|小)見出し終わり］"));
pub static OPENER_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃章扉（([^）]+)）(?:：(.+))?］"));
pub static SECTION_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃章：(.+?)］"));
pub const TOC_LINE: &str = "［＃目次］";

/// 見出しの行なら (段階, 見出しの注記を除いた行)
pub fn heading_of_line(line: &str) -> Option<(i64, String)> {
    if let Some(m) = INLINE_HEAD_RE.search(line) {
        return Some((level_of(g(&m, 1).unwrap()),
                     format!("{}{}{}", &line[..start(&m)], g(&m, 2).unwrap(), &line[end(&m)..])));
    }
    if let Some(m) = POSTFIX_HEAD_RE.search(line) {
        return Some((level_of(g(&m, 2).unwrap()), format!("{}{}", &line[..start(&m)], &line[end(&m)..])));
    }
    None
}

/// 本文で使われている見出しの段階
pub fn levels_used(raw: &str) -> Vec<i64> {
    let mut found: Vec<i64> = Vec::new();
    for (rx, gi) in [(&*INLINE_HEAD_RE, 1), (&*POSTFIX_HEAD_RE, 2), (&*BLOCK_HEAD_START_RE, 1)] {
        for m in rx.finditer(raw) {
            let l = level_of(g(&m, gi).unwrap());
            if !found.contains(&l) { found.push(l); }
        }
    }
    found
}

static PT_ANN: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃[^］]*］"));
static PT_RUBY: LazyLock<Rx> = LazyLock::new(|| Rx::new("(?<!《)《[^《》]*》(?!》)"));
static PT_PER: LazyLock<Rx> = LazyLock::new(|| Rx::new("《《([^》]*)》》"));

/// 目次に載せる題名：注記・ルビの読み・｜・行頭行末の空白を除く
pub fn plain_title(s0: &str) -> String {
    let s = PT_ANN.sub(s0, "");
    let s = PT_RUBY.sub(&s, "");
    let s = PT_PER.sub_fn(&s, 0, |m| format!("《{}》", g(m, 1).unwrap()));
    let s = s.replace('｜', "").replace('\t', "　");
    strip_chars(&s, "　 ").to_string()
}
