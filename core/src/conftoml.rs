//! 設定ファイルを画面で直すための部品（Python 版の conftoml.py と同じ動き）
//!
//! - `form_from_config`：設定から画面の欄の値を作る
//! - `form_to_config`：元の設定に欄の値を重ねる（欄にない項目はそのまま残す）
//! - `dump_toml`：設定を TOML の文字列にする
//! - `check_config`：変換の前に分かる書き損じを説明の文で返す
use crate::context::{vstr, NOTE_FORMATS};
use crate::convert::KNOWN_KEYS;
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

pub const FORM_NOTE_FORMATS: [&str; 6] = ["※1", "＊1", "注1", "[1]", "1]", "none"];
pub const LINK_KINDS: [&str; 3] = ["url", "mail", "x"];
const DEFAULT_LINKS: [&str; 2] = ["url", "mail"];
const FORM_TEXT_KEYS: [&str; 6] = ["title", "title_file_as", "publisher", "date", "description", "identifier"];
const HEADER: &str = "# egword2epub の設定ファイル\n";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatorForm {
    pub name: String,
    pub role: String,
    pub file_as: String,
    /// 元の設定の何人目か（欄にない項目を引き継ぐ）。新しく足した人は None
    pub src: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteForm {
    pub format: String,
    pub label: String,
    pub heading_lines: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigForm {
    pub title: String,
    pub title_file_as: String,
    pub publisher: String,
    pub date: String,
    pub description: String,
    pub identifier: String,
    pub language: String,
    pub creators: Vec<CreatorForm>,
    pub writing_mode: String,
    pub css: String,
    pub autolink: Vec<String>,
    pub footnote: NoteForm,
    pub endnote: NoteForm,
    pub guess_periodicals: bool,
    pub squeeze_blank_lines: bool,
    pub chapter_image: String,
    pub cover_image: String,
    pub colophon: bool,
    pub colophon_title: String,
    pub colophon_date_label: String,
    pub colophon_lines: String,
}

impl ConfigForm {
    fn text(&self, key: &str) -> &str {
        match key {
            "title" => &self.title,
            "title_file_as" => &self.title_file_as,
            "publisher" => &self.publisher,
            "date" => &self.date,
            "description" => &self.description,
            "identifier" => &self.identifier,
            _ => "",
        }
    }
    fn set_text(&mut self, key: &str, v: String) {
        match key {
            "title" => self.title = v,
            "title_file_as" => self.title_file_as = v,
            "publisher" => self.publisher = v,
            "date" => self.date = v,
            "description" => self.description = v,
            "identifier" => self.identifier = v,
            _ => {}
        }
    }
}

fn note_default(key: &str) -> (&'static str, &'static str) {
    if key == "footnote" { ("※1", "脚注") } else { ("[1]", "文末注") }
}

/// Python の空とみなす値（''・None・[]・{}・False・0）
fn is_empty(v: &Value) -> bool {
    match v {
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Table(t) => t.is_empty(),
        Value::Boolean(b) => !*b,
        Value::Integer(i) => *i == 0,
        Value::Float(f) => *f == 0.0,
        Value::Datetime(_) => false,
    }
}

/// 値が空なら項目ごと消す。既定と同じ値は、もとから書いてあるときだけ残す
fn set(d: &mut Table, key: &str, value: Value, default: Option<&Value>) {
    if is_empty(&value) {
        d.remove(key);
    } else if default == Some(&value) && !d.contains_key(key) {
    } else {
        d.insert(key.to_string(), value);
    }
}

fn s(v: &str) -> Value { Value::String(v.to_string()) }

/// 読点・コンマ・空白で区切る
pub fn split_words(text: &str) -> Vec<String> {
    text.replace('、', " ").replace(',', " ").split_whitespace().map(|w| w.to_string()).collect()
}

fn get_s(t: &Table, key: &str, default: &str) -> String { t.get(key).map(vstr).unwrap_or_else(|| default.to_string()) }

/// 設定から欄の値を作る
pub fn form_from_config(d: &Table) -> ConfigForm {
    let empty = Table::new();
    let book = d.get("book").and_then(|v| v.as_table()).unwrap_or(&empty);
    let mut f = ConfigForm::default();
    for k in FORM_TEXT_KEYS { f.set_text(k, get_s(book, k, "")); }
    f.language = get_s(book, "language", "ja");
    if let Some(Value::Array(cs)) = book.get("creators") {
        for (i, c) in cs.iter().enumerate() {
            f.creators.push(match c {
                Value::Table(t) => CreatorForm { name: get_s(t, "name", ""), role: get_s(t, "role", "aut"),
                                                 file_as: get_s(t, "file_as", ""), src: Some(i) },
                other => CreatorForm { name: vstr(other), role: "aut".into(), file_as: String::new(), src: None },
            });
        }
    }
    if f.creators.is_empty() {
        f.creators.push(CreatorForm { role: "aut".into(), ..Default::default() });
    }
    f.writing_mode = get_s(d, "writing_mode", "vertical");
    f.css = get_s(d, "css", "");
    let links: Vec<String> = match d.get("autolink") {
        None => DEFAULT_LINKS.iter().map(|x| x.to_string()).collect(),
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(|x| x.to_string())).collect(),
        Some(_) => vec![],
    };
    f.autolink = LINK_KINDS.iter().filter(|k| links.iter().any(|l| l == *k)).map(|k| k.to_string()).collect();
    let notes = d.get("notes").and_then(|v| v.as_table()).unwrap_or(&empty);
    for key in ["footnote", "endnote"] {
        let sub = notes.get(key).and_then(|v| v.as_table()).unwrap_or(&empty);
        let (dfmt, dlabel) = note_default(key);
        let format = match sub.get("format") {
            None => dfmt.to_string(),
            Some(v) if !matches!(v, Value::String(_)) && !crate::context::truthy(v) => "none".into(),
            Some(Value::String(x)) if x.is_empty() => "none".into(),
            Some(v) => vstr(v),
        };
        let heading_lines = match sub.get("heading_lines") {
            None => String::new(),
            Some(Value::String(x)) => x.clone(),
            Some(Value::Array(a)) => a.iter().map(vstr).collect::<Vec<_>>().join(" "),
            Some(v) => vstr(v),
        };
        let n = NoteForm { format, label: get_s(sub, "label", dlabel), heading_lines };
        if key == "footnote" { f.footnote = n } else { f.endnote = n }
    }
    f.guess_periodicals = d.get("guess_periodicals").map(crate::context::truthy).unwrap_or(false);
    f.squeeze_blank_lines = d.get("squeeze_blank_lines").map(crate::context::truthy).unwrap_or(false);
    f.chapter_image = get_s(d, "chapter_image", "");
    f.cover_image = get_s(d, "cover_image", "");
    let col = d.get("colophon").and_then(|v| v.as_table());
    f.colophon = col.is_some();
    let col = col.unwrap_or(&empty);
    f.colophon_title = get_s(col, "title", "");
    f.colophon_date_label = get_s(col, "date_label", "");
    f.colophon_lines = match col.get("lines") {
        None => String::new(),
        Some(Value::Array(a)) => a.iter().map(vstr).collect::<Vec<_>>().join("\n"),
        Some(v) => vstr(v),
    };
    f
}

fn sub_table<'a>(d: &'a mut Table, key: &str) -> &'a mut Table {
    if !matches!(d.get(key), Some(Value::Table(_))) {
        d.insert(key.to_string(), Value::Table(Table::new()));
    }
    d.get_mut(key).unwrap().as_table_mut().unwrap()
}

/// 元の設定に欄の値を重ねる（欄にない項目はそのまま残る）
pub fn form_to_config(data: &Table, form: &ConfigForm) -> Table {
    let mut d = data.clone();
    let mut book = sub_table(&mut d, "book").clone();
    for k in FORM_TEXT_KEYS { set(&mut book, k, s(form.text(k).trim()), None); }
    set(&mut book, "language", s(form.language.trim()), Some(&s("ja")));
    let old: Vec<Value> = match book.get("creators") { Some(Value::Array(a)) => a.clone(), _ => vec![] };
    let mut creators = vec![];
    for row in &form.creators {
        let name = row.name.trim();
        if name.is_empty() && row.file_as.trim().is_empty() { continue; }
        let role = row.role.trim();
        let mut c = Table::new();
        c.insert("name".into(), s(name));
        c.insert("role".into(), s(if role.is_empty() { "aut" } else { role }));
        if let Some(Value::Table(extra)) = row.src.and_then(|i| old.get(i)) {
            for (k, v) in extra {
                if k != "name" && k != "role" && k != "file_as" { c.insert(k.clone(), v.clone()); }
            }
        }
        set(&mut c, "file_as", s(row.file_as.trim()), None);
        creators.push(Value::Table(c));
    }
    set(&mut book, "creators", Value::Array(creators), None);
    set(&mut d, "book", Value::Table(book), Some(&Value::Table(Table::new())));

    set(&mut d, "writing_mode", s(&form.writing_mode), Some(&s("vertical")));
    set(&mut d, "css", s(form.css.trim()), None);
    let links: Vec<&str> = LINK_KINDS.iter().copied().filter(|k| form.autolink.iter().any(|l| l == k)).collect();
    if links != DEFAULT_LINKS || d.contains_key("autolink") {
        d.insert("autolink".into(), Value::Array(links.iter().map(|x| s(x)).collect()));
    }
    let mut notes = sub_table(&mut d, "notes").clone();
    for (key, n) in [("footnote", &form.footnote), ("endnote", &form.endnote)] {
        let (dfmt, dlabel) = note_default(key);
        let mut c = notes.get(key).and_then(|v| v.as_table()).cloned().unwrap_or_default();
        set(&mut c, "format", s(&n.format), Some(&s(dfmt)));
        set(&mut c, "label", s(n.label.trim()), Some(&s(dlabel)));
        set(&mut c, "heading_lines", Value::Array(split_words(&n.heading_lines).into_iter().map(Value::String).collect()), None);
        set(&mut notes, key, Value::Table(c), Some(&Value::Table(Table::new())));
    }
    set(&mut d, "notes", Value::Table(notes), Some(&Value::Table(Table::new())));
    set(&mut d, "guess_periodicals", Value::Boolean(form.guess_periodicals), None);
    set(&mut d, "squeeze_blank_lines", Value::Boolean(form.squeeze_blank_lines), None);
    set(&mut d, "chapter_image", s(form.chapter_image.trim()), None);
    set(&mut d, "cover_image", s(form.cover_image.trim()), None);

    if form.colophon {
        let mut col = d.get("colophon").and_then(|v| v.as_table()).cloned().unwrap_or_default();
        set(&mut col, "title", s(form.colophon_title.trim()), None);
        set(&mut col, "date_label", s(form.colophon_date_label.trim()), None);
        let lines = form.colophon_lines.trim_end_matches('\n');
        let arr: Vec<Value> = if lines.trim().is_empty() { vec![] } else { lines.split('\n').map(s).collect() };
        set(&mut col, "lines", Value::Array(arr), None);
        d.insert("colophon".into(), Value::Table(col));
    } else {
        d.remove("colophon");
    }
    d
}

/// 新しい設定の雛形（識別子と日付は呼ぶ側で作る）
pub fn new_config(title: &str, identifier: &str, date: &str) -> Table {
    let mut creator = Table::new();
    creator.insert("name".into(), s(""));
    creator.insert("role".into(), s("aut"));
    let mut book = Table::new();
    book.insert("title".into(), s(title));
    book.insert("identifier".into(), s(identifier));
    book.insert("language".into(), s("ja"));
    book.insert("date".into(), s(date));
    book.insert("creators".into(), Value::Array(vec![Value::Table(creator)]));
    let mut d = Table::new();
    d.insert("book".into(), Value::Table(book));
    d
}

// ---------------------------------------------------------------- 書き出し

fn order(path: &str) -> &'static [&'static str] {
    match path {
        "" => &["writing_mode", "css", "autolink", "guess_periodicals", "squeeze_blank_lines",
                "chapter_image", "cover_image", "book", "notes", "colophon"],
        "book" => &["title", "title_file_as", "identifier", "language", "publisher", "date", "description", "creators"],
        "book.creators" => &["name", "role", "file_as"],
        "notes" => &["footnote", "endnote"],
        "notes.footnote" | "notes.endnote" => &["format", "label", "heading_lines"],
        "colophon" => &["title", "date_label", "lines"],
        _ => &[],
    }
}

/// JSON の文字列（Python の json.dumps(ensure_ascii=False) と同じ）。TOML の基本文字列としても正しい
fn json_str(x: &str) -> String {
    let mut o = String::with_capacity(x.len() + 2);
    o.push('"');
    for ch in x.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn key(k: &str) -> String {
    if !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { k.to_string() } else { json_str(k) }
}

/// Python の repr(float)
fn float_repr(f: f64) -> String {
    if f.is_nan() { return "nan".into(); }
    if f.is_infinite() { return if f > 0.0 { "inf".into() } else { "-inf".into() }; }
    let a = f.abs();
    if a == 0.0 || (1e-4..1e16).contains(&a) {
        return format!("{:?}", f);
    }
    let e = format!("{:e}", f);   // 1.5e-5 のような形
    let (m, x) = e.split_once('e').unwrap();
    let x: i32 = x.parse().unwrap();
    format!("{}e{}{:02}", m, if x < 0 { '-' } else { '+' }, x.abs())
}

fn value(v: &Value) -> String {
    match v {
        Value::Boolean(b) => if *b { "true".into() } else { "false".into() },
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => float_repr(*f),
        Value::Datetime(d) => d.to_string(),
        Value::String(x) => json_str(x),
        Value::Array(a) => format!("[{}]", a.iter().map(value).collect::<Vec<_>>().join(", ")),
        Value::Table(t) => format!("{{ {} }}", t.iter().map(|(k, x)| format!("{} = {}", key(k), value(x))).collect::<Vec<_>>().join(", ")),
    }
}

fn ordered<'a>(d: &'a Table, path: &str) -> Vec<(&'a String, &'a Value)> {
    let ord = order(path);
    let mut out: Vec<(&String, &Value)> = ord.iter().filter_map(|k| d.get_key_value(*k)).collect();
    out.extend(d.iter().filter(|(k, _)| !ord.contains(&k.as_str())));
    out
}

fn is_table_array(v: &Value) -> bool {
    matches!(v, Value::Array(a) if !a.is_empty() && a.iter().all(|x| x.is_table()))
}

fn emit(d: &Table, path: &str, out: &mut Vec<String>) {
    let (mut scalars, mut tables, mut arrays) = (vec![], vec![], vec![]);
    for (k, v) in ordered(d, &path.replace('"', "")) {
        match v {
            Value::Table(t) => tables.push((k, t)),
            _ if is_table_array(v) => arrays.push((k, v.as_array().unwrap())),
            _ => scalars.push((k, v)),
        }
    }
    if !path.is_empty() && (!scalars.is_empty() || (tables.is_empty() && arrays.is_empty())) {
        out.push(format!("\n[{}]", path));
    }
    for (k, v) in scalars { out.push(format!("{} = {}", key(k), value(v))); }
    for (k, items) in arrays {
        let sub = if path.is_empty() { key(k) } else { format!("{}.{}", path, key(k)) };
        for item in items {
            out.push(format!("\n[[{}]]", sub));
            for (ik, iv) in ordered(item.as_table().unwrap(), &sub.replace('"', "")) {
                out.push(format!("{} = {}", key(ik), value(iv)));
            }
        }
    }
    for (k, t) in tables {
        let sub = if path.is_empty() { key(k) } else { format!("{}.{}", path, key(k)) };
        emit(t, &sub, out);
    }
}

/// 設定を TOML の文字列にする
pub fn dump_toml(d: &Table) -> String {
    let mut out = vec![];
    emit(d, "", &mut out);
    format!("{}{}\n", HEADER, out.join("\n").trim_start_matches('\n'))
}

// ---------------------------------------------------------------- 確認

fn is_iso_date(x: &str) -> bool {
    let b = x.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    match b.len() {
        4 => digits(0..4),
        7 => digits(0..4) && b[4] == b'-' && digits(5..7),
        10 => digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10),
        _ => false,
    }
}

/// 変換の前に分かる書き損じを、説明の文の一覧で返す（空なら問題なし）。
/// css_exists：CSS（設定ファイルの場所から見た位置）があるか
pub fn check_config(conf: &Table, css_exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut problems = vec![];
    let mut unknown: Vec<&String> = conf.keys().filter(|k| !KNOWN_KEYS.contains(&k.as_str())).collect();
    if !unknown.is_empty() {
        unknown.sort();
        problems.push(format!("知らない項目があります：{}", unknown.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("、")));
    }
    let mode = conf.get("writing_mode").map(vstr).unwrap_or_else(|| "vertical".into());
    if !matches!(conf.get("writing_mode"), None | Some(Value::String(_))) || (mode != "vertical" && mode != "horizontal") {
        problems.push("writing_mode は \"vertical\"（縦書き）か \"horizontal\"（横書き）にしてください。".into());
    }
    let links_ok = match conf.get("autolink") {
        None => true,
        Some(Value::Array(a)) => a.iter().all(|v| matches!(v, Value::String(x) if LINK_KINDS.contains(&x.as_str()))),
        Some(_) => false,
    };
    if !links_ok { problems.push("autolink に使えるのは url・mail・x です。".into()); }
    if let Some(css) = conf.get("css") {
        if crate::context::truthy(css) {
            let name = vstr(css);
            if !css_exists(&name) {
                problems.push(format!("CSS「{}」が見つかりません（設定ファイルの場所から見た位置で書きます）。", name));
            }
        }
    }
    let empty = Table::new();
    let book = match conf.get("book") {
        None => &empty,
        Some(Value::Table(t)) => t,
        Some(_) => { problems.push("[book] は表にしてください。".into()); &empty }
    };
    if let Some(date) = book.get("date") {
        if crate::context::truthy(date) && !is_iso_date(&vstr(date)) {
            problems.push(format!("発行日「{}」は 2026-10-19 の形で書いてください。", vstr(date)));
        }
    }
    if let Some(Value::Array(cs)) = book.get("creators") {
        for (i, c) in cs.iter().enumerate() {
            let ok = matches!(c, Value::Table(t) if !get_s(t, "name", "").trim().is_empty());
            if !ok { problems.push(format!("{} 人目の作者の名前が空です。", i + 1)); }
        }
    }
    let notes = conf.get("notes").and_then(|v| v.as_table()).unwrap_or(&empty);
    let mut used: Vec<&str> = vec![];
    for (key, label) in [("footnote", "脚注"), ("endnote", "文末注")] {
        let fmt = notes.get(key).and_then(|v| v.as_table()).and_then(|t| t.get("format")).cloned()
            .unwrap_or_else(|| s(note_default(key).0));
        let skip = match &fmt { Value::String(x) => x == "none" || x.is_empty(), Value::Boolean(b) => !*b, _ => false };
        if skip { continue; }
        match NOTE_FORMATS.iter().find(|f| matches!(&fmt, Value::String(x) if x == f.0)) {
            None => problems.push(format!("{}の書式「{}」には対応していません。", label, vstr(&fmt))),
            Some(f) => used.push(f.1),
        }
    }
    if used.len() == 2 && used[0] == used[1] {
        problems.push("脚注と文末注に同じ書式は使えません。".into());
    }
    problems
}

/// 設定を JSON にする（画面の側で持っておくため。日付は toml の決まりの形で残る）
pub fn table_to_json(d: &Table) -> String { serde_json::to_string(d).unwrap() }

pub fn table_from_json(text: &str) -> Result<Table, String> { serde_json::from_str(text).map_err(|e| e.to_string()) }
