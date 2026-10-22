//! Python の文字列と正規表現の振る舞いに合わせるための小道具。
//! 位置はすべて UTF-8 のバイトで数える（正規表現の位置と同じ）。
//! Python は字（符号位置）で数えるので、字の数そのものが結果に出るところ（字下げの数、
//! 先頭 20 字など）は、そこだけ字で数える。

use fancy_regex::{Captures, Regex};

/// Python の str.strip() が取り除く空白（全角空白・改ページなどを含む）
pub fn is_py_space(c: char) -> bool {
    matches!(c as u32,
        0x09..=0x0D | 0x1C..=0x20 | 0x85 | 0xA0 | 0x1680 | 0x2000..=0x200A
        | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000)
}

pub fn py_strip(s: &str) -> &str { s.trim_matches(is_py_space) }
pub fn py_lstrip(s: &str) -> &str { s.trim_start_matches(is_py_space) }
pub fn py_rstrip(s: &str) -> &str { s.trim_end_matches(is_py_space) }

/// Python の s.strip(chars)
pub fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str { s.trim_matches(|c| chars.contains(c)) }
pub fn lstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str { s.trim_start_matches(|c| chars.contains(c)) }
pub fn rstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str { s.trim_end_matches(|c| chars.contains(c)) }

/// 字で数えて先頭 n 字（Python の s[:n]）
pub fn prefix_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) { Some((i, _)) => &s[..i], None => s }
}

/// 字で数えて末尾 n 字（Python の s[-n:]）
pub fn suffix_chars(s: &str, n: usize) -> &str {
    let count = s.chars().count();
    if n >= count { return s; }
    let (i, _) = s.char_indices().nth(count - n).unwrap();
    &s[i..]
}

/// Python の html.escape
pub fn html_escape(s: &str, quote: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quote => out.push_str("&quot;"),
            '\'' if quote => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn esc(s: &str) -> String { html_escape(s, true) }
pub fn esc_nq(s: &str) -> String { html_escape(s, false) }

/// 全角・半角の数字の文字列を数に（Python の util.num）
pub fn num(s: &str) -> i64 {
    let mut v: i64 = 0;
    for c in s.chars() {
        let d = match c {
            '0'..='9' => c as i64 - '0' as i64,
            '０'..='９' => c as i64 - '０' as i64,
            _ => match c.to_digit(10) { Some(d) => d as i64, None => continue },
        };
        v = v.saturating_mul(10).saturating_add(d);
    }
    v
}

// ---------------------------------------------------------------- 正規表現

/// Python の re に似せた正規表現
pub struct Rx {
    pub re: Regex,
    anchored: Regex,
    full: Regex,
}

impl Rx {
    pub fn new(p: &str) -> Rx {
        Rx::try_new(p).unwrap_or_else(|e| panic!("正規表現の誤り：{}：{}", p, e))
    }

    /// 使う人が書いた正規表現（設定の cover_image など）
    pub fn try_new(p: &str) -> Result<Rx, String> {
        let build = |s: String| Regex::new(&s).map_err(|e| e.to_string());
        Ok(Rx {
            re: build(p.to_string())?,
            anchored: build(format!("\\A(?:{})", p))?,
            full: build(format!("\\A(?:{})\\z", p))?,
        })
    }

    pub fn search<'t>(&self, s: &'t str) -> Option<Captures<'t>> { self.re.captures(s).ok().flatten() }
    /// 先頭から当たるか（Python の re.match）
    pub fn match_<'t>(&self, s: &'t str) -> Option<Captures<'t>> { self.anchored.captures(s).ok().flatten() }
    /// 全体が当たるか（Python の re.fullmatch）
    pub fn fullmatch<'t>(&self, s: &'t str) -> Option<Captures<'t>> { self.full.captures(s).ok().flatten() }
    pub fn is_full(&self, s: &str) -> bool { self.full.is_match(s).unwrap_or(false) }
    pub fn finditer<'r, 't>(&'r self, s: &'t str) -> impl Iterator<Item = Captures<'t>> + 'r
    where 't: 'r {
        self.re.captures_iter(s).filter_map(|c| c.ok())
    }

    /// 置き換え（Python の re.sub に関数を渡す形）。count が 0 ならすべて
    pub fn sub_fn(&self, s: &str, count: usize, mut f: impl FnMut(&Captures) -> String) -> String {
        let mut out = String::with_capacity(s.len());
        let mut pos = 0;
        let mut n = 0;
        for c in self.finditer(s) {
            if count > 0 && n >= count { break; }
            let m = c.get(0).unwrap();
            out.push_str(&s[pos..m.start()]);
            out.push_str(&f(&c));
            pos = m.end();
            n += 1;
        }
        out.push_str(&s[pos..]);
        out
    }

    pub fn sub(&self, s: &str, repl: &str) -> String { self.sub_fn(s, 0, |_| repl.to_string()) }

    /// Python の re.split（組があれば区切りも残す）
    pub fn split(&self, s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut pos = 0;
        for c in self.finditer(s) {
            let m = c.get(0).unwrap();
            out.push(s[pos..m.start()].to_string());
            for i in 1..c.len() {
                out.push(c.get(i).map(|g| g.as_str().to_string()).unwrap_or_default());
            }
            pos = m.end();
        }
        out.push(s[pos..].to_string());
        out
    }
}

/// 組の文字列（当たっていなければ None）
pub fn g<'t>(c: &Captures<'t>, i: usize) -> Option<&'t str> { c.get(i).map(|m| m.as_str()) }
pub fn gn<'t>(c: &Captures<'t>, name: &str) -> Option<&'t str> { c.name(name).map(|m| m.as_str()) }
/// 組が当たって、しかも空でない（Python の if m.group(x):）
pub fn has(c: &Captures, name: &str) -> bool { c.name(name).map(|m| !m.as_str().is_empty()).unwrap_or(false) }
pub fn start(c: &Captures) -> usize { c.get(0).unwrap().start() }
pub fn end(c: &Captures) -> usize { c.get(0).unwrap().end() }

/// Python の文字列の大小（符号位置の順。Rust の str の順と同じ）
pub fn sorted(mut v: Vec<String>) -> Vec<String> { v.sort(); v }
