//! 設定（TOML）の読み方・注の系統（notesys.py）・変換中の状態（context.py）

use crate::py::*;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
pub use toml::{Table, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ConvertError(pub String);

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str(&self.0) }
}

// ---------------------------------------------------------------- 設定の値

/// Python の str() に当たるもの
pub fn vstr(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => if f.fract() == 0.0 && f.is_finite() { format!("{:.1}", f) } else { f.to_string() },
        Value::Boolean(b) => if *b { "True".into() } else { "False".into() },
        Value::Datetime(d) => d.to_string(),
        Value::Array(a) => format!("[{}]", a.iter().map(vstr).collect::<Vec<_>>().join(", ")),
        Value::Table(_) => "{…}".into(),
    }
}

/// Python の真偽（空の文字列・0・false・空の配列は偽）
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::String(s) => !s.is_empty(),
        Value::Integer(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Boolean(b) => *b,
        Value::Datetime(_) => true,
        Value::Array(a) => !a.is_empty(),
        Value::Table(t) => !t.is_empty(),
    }
}

pub fn get_str(t: &Table, key: &str) -> Option<String> { t.get(key).map(vstr) }
pub fn get_truthy(t: &Table, key: &str) -> bool { t.get(key).map(truthy).unwrap_or(false) }
pub fn get_table<'a>(t: &'a Table, key: &str) -> Option<&'a Table> { t.get(key).and_then(|v| v.as_table()) }

// ---------------------------------------------------------------- 注の系統

/// 書式名 → (本文中の参照の正規表現（番号は {n} の位置）, 表示の前, 表示の後)
pub(crate) const NOTE_FORMATS: [(&str, &str, &str, &str); 6] = [
    ("※1", "※{n}", "※", ""),
    ("＊1", "[＊*]{n}", "＊", ""),
    ("*1", "[＊*]{n}", "＊", ""),
    ("注1", "注{n}", "注", ""),
    ("[1]", "\\[{n}\\]", "［", "］"),
    ("1]", "(?<![\\[0-9０-９]){n}\\]", "", "］"),
];
const NOTE_DIGITS: &str = "[0-9０-９]+";

struct NoteDefault { key: &'static str, format: &'static str, label: &'static str, cls: &'static str, section_cls: &'static str, popup: bool }
const NOTE_DEFAULTS: [NoteDefault; 2] = [
    NoteDefault { key: "footnote", format: "※1", label: "脚注", cls: "gn", section_cls: "gnotes", popup: true },
    NoteDefault { key: "endnote", format: "[1]", label: "文末注", cls: "tn", section_cls: "tnotes", popup: false },
];

pub struct NoteSystem {
    pub key: String,
    pub fmt: String,
    pub label: String,
    pub heading_lines: Vec<String>,
    pub cls: String,
    pub section_cls: String,
    pub popup: bool,
    pub prefix: String,
    pub suffix: String,
    pub ref_pattern: String,
    pub ref_re: Rx,
    pub entry_re: Rx,
    pub start_re: Rx,
}

impl NoteSystem {
    pub fn display(&self, n: &str) -> String { format!("{}<span class=\"tcy\">{}</span>{}", self.prefix, n, self.suffix) }
}

/// 設定の [notes.footnote] [notes.endnote] から系統を作る。format = "none" で使わない
pub fn build_systems(conf: &Table) -> Result<Vec<NoteSystem>, ConvertError> {
    let empty = Table::new();
    let notes = get_table(conf, "notes").unwrap_or(&empty);
    let mut systems: Vec<NoteSystem> = Vec::new();
    for d in NOTE_DEFAULTS.iter() {
        let c = get_table(notes, d.key).unwrap_or(&empty);
        let fmt = match c.get("format") {
            None => d.format.to_string(),
            Some(Value::Boolean(false)) => "none".into(),
            Some(v) => vstr(v),
        };
        if fmt == "none" || fmt.is_empty() { continue; }
        let f = NOTE_FORMATS.iter().find(|f| f.0 == fmt).ok_or_else(|| {
            let names: Vec<&str> = NOTE_FORMATS.iter().map(|f| f.0).filter(|n| *n != "*1").collect();
            ConvertError(format!("設定 notes.{}.format の「{}」には対応していません。使えるもの：{}、none", d.key, fmt, names.join("、")))
        })?;
        let heading = match c.get("heading_lines") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a.iter().map(vstr).collect(),
            _ => vec![],
        };
        let label = c.get("label").map(vstr).unwrap_or_else(|| d.label.to_string());
        let popup = match c.get("popup") { Some(Value::Boolean(b)) => *b, _ => d.popup };
        let ref_pattern = f.1.replace("{n}", &format!("(?P<{}_n>{})", d.key, NOTE_DIGITS));
        let bare = f.1.replace("{n}", &format!("({})", NOTE_DIGITS));
        systems.push(NoteSystem {
            key: d.key.into(), fmt: fmt.clone(), label, heading_lines: heading,
            cls: c.get("cls").map(vstr).unwrap_or_else(|| d.cls.into()),
            section_cls: c.get("section_cls").map(vstr).unwrap_or_else(|| d.section_cls.into()),
            popup, prefix: f.2.into(), suffix: f.3.into(),
            ref_re: Rx::new(&ref_pattern), ref_pattern,
            entry_re: Rx::new(&format!("{}\\s?(.*)", bare)),
            start_re: Rx::new(&format!("{}[ 　]", bare)),
        });
    }
    let pats: Vec<&str> = systems.iter().map(|s| NOTE_FORMATS.iter().find(|f| f.0 == s.fmt).unwrap().1).collect();
    let uniq: HashSet<&&str> = pats.iter().collect();
    if uniq.len() != pats.len() {
        return Err(ConvertError("脚注と文末注に同じ書式は使えません。違う書式を設定してください。".into()));
    }
    Ok(systems)
}

// ---------------------------------------------------------------- 状態

/// 章の終わりなどにある、注の本文の並び一つ
pub struct NotesBlock {
    pub pos: usize,
    pub file: String,
    pub text: String,
    pub numbers: HashMap<String, Vec<String>>,
    pub scopes: HashMap<String, String>,
    pub cls: HashMap<String, String>,
}

impl NotesBlock {
    pub fn scope(&self, system: &str) -> String { self.scopes.get(system).cloned().unwrap_or_default() }
    pub fn anchor(&self, system: &str, n: &str) -> String {
        format!("{}{}{}", self.cls.get(system).cloned().unwrap_or_default(), self.scope(system), n)
    }
    pub fn href(&self, system: &str, n: &str) -> String { format!("{}#{}", self.file, self.anchor(system, n)) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Warning { pub file: String, pub message: String }

pub struct HeadingEntry { pub pos: usize, pub level: i64, pub title: String, pub file: String, pub anchor: String }

/// 画像の置き場（ファイルの場所でも、ブラウザで選んだファイルでもよい）
pub trait ImageSource {
    fn exists(&self, name: &str) -> bool;
    /// 画像の中身（大きさを読むのと、EPUB に入れるのに使う）
    fn bytes(&self, name: &str) -> Option<Vec<u8>>;
}

pub struct Context<'a> {
    pub images_src: &'a dyn ImageSource,
    pub systems: Vec<NoteSystem>,
    pub notes_blocks: Vec<NotesBlock>,
    /// (系統, 並びの番号, 番号) → 本文で最初に参照したファイル名
    pub backref: HashMap<(String, usize, String), String>,
    pub images: Vec<String>,
    pub missing: Vec<String>,
    pub periodicals: HashSet<String>,
    pub seen: HashMap<String, i64>,
    pub cover: Option<String>,
    pub warnings: Vec<Warning>,
    pub where_: String,
    pub pos: usize,
    pub horizontal: bool,
    pub autolink: HashSet<String>,
    pub headings: Vec<HeadingEntry>,
    pub toc_files: HashSet<String>,
    pub encoding: String,
    pub src_pages: indexmap::IndexMap<String, usize>,
    pub token_re: Option<Rx>,
}

impl<'a> Context<'a> {
    pub fn new(images_src: &'a dyn ImageSource, systems: Vec<NoteSystem>) -> Context<'a> {
        Context {
            images_src, systems, notes_blocks: vec![], backref: HashMap::new(), images: vec![], missing: vec![],
            periodicals: HashSet::new(), seen: HashMap::new(), cover: None, warnings: vec![], where_: String::new(),
            pos: 0, horizontal: false, autolink: ["url", "mail"].iter().map(|s| s.to_string()).collect(),
            headings: vec![], toc_files: HashSet::new(), encoding: "utf-8".into(),
            src_pages: indexmap::IndexMap::new(), token_re: None,
        }
    }

    pub fn system(&self, key: &str) -> &NoteSystem { self.systems.iter().find(|s| s.key == key).unwrap() }

    pub fn add_image(&mut self, name: &str) {
        if !self.images.iter().any(|i| i == name) { self.images.push(name.to_string()); }
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        let w = Warning { file: self.where_.clone(), message: msg.into() };
        if !self.warnings.contains(&w) { self.warnings.push(w); }
    }

    pub fn add_heading(&mut self, level: i64, title: String) -> String {
        let anchor = format!("h{}", self.headings.len() + 1);
        self.headings.push(HeadingEntry { pos: self.pos, level, title, file: self.where_.clone(), anchor: anchor.clone() });
        anchor
    }

    /// 参照（系統, 番号, 位置）を注の並びに結びつける。返り値は並びの番号
    pub fn resolve(&self, system: &str, n: &str, pos: usize) -> Option<usize> {
        let mut before = None;
        for (i, b) in self.notes_blocks.iter().enumerate() {
            if b.numbers.get(system).map(|v| v.iter().any(|x| x == n)).unwrap_or(false) {
                if b.pos >= pos { return Some(i); }
                before = Some(i);
            }
        }
        before
    }
}

static PERIODICAL_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("(?m)(^|[^一-鿿㐀-䶿々〆ヶ〇])《([^《》﹅]+)》"));

/// 漢字以外の直後の《…》を誌名とみなす（本書向けの推定）
pub fn guess_periodicals(raw: &str) -> HashSet<String> {
    PERIODICAL_RE.finditer(raw).map(|m| g(&m, 2).unwrap().to_string()).collect()
}
