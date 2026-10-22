//! 頁の振り分けと章立て・目次（structure.py）

use crate::basics::*;
use crate::blocks::*;
use crate::context::*;
use crate::epub::colophon_html;
use crate::inline::RenderPos;
use crate::notes::*;
use crate::py::*;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Clone)]
struct PageEntry {
    kind: &'static str,
    text: String,
    fn_: String,
    img: Option<String>,
    title: String,
    lead: usize,
    chapter: String,
}

impl PageEntry {
    fn new(kind: &'static str, text: String, fn_: String) -> Self {
        PageEntry { kind, text, fn_, img: None, title: String::new(), lead: 0, chapter: String::new() }
    }
}

#[derive(Clone, Debug)]
pub struct NavItem {
    pub pos: usize,
    pub seq: i64,
    pub level: i64,
    pub title: String,
    pub fn_: String,
    pub anchor: String,
    pub group: bool,
}

impl NavItem {
    fn new(pos: usize, seq: i64, level: i64, title: &str, fn_: &str) -> Self {
        NavItem { pos, seq, level, title: title.into(), fn_: fn_.into(), anchor: String::new(), group: false }
    }
    pub fn href(&self) -> String {
        if self.anchor.is_empty() { self.fn_.clone() } else { format!("{}#{}", self.fn_, self.anchor) }
    }
}

fn starts_chapter(s: &str, split_level: Option<i64>) -> bool {
    if OPENER_RE.is_full(s) || SECTION_RE.is_full(s) { return true; }
    if let Some(sl) = split_level {
        if let Some((lv, _)) = heading_of_line(s) { if lv == sl { return true; } }
        if let Some(m) = BLOCK_HEAD_START_RE.fullmatch(s) { if level_of(g(&m, 1).unwrap()) == sl { return true; } }
    }
    false
}

/// 一頁を、章の始まりの行の前で切る（頁の初めなら切らない）
fn segments(text: &str, split_level: Option<i64>) -> Vec<String> {
    let mut segs: Vec<Vec<&str>> = vec![vec![]];
    for l in text.split('\n') {
        if starts_chapter(py_strip(l), split_level) && segs.last().unwrap().iter().any(|x| !py_strip(x).is_empty()) {
            segs.push(vec![]);
        }
        segs.last_mut().unwrap().push(l);
    }
    segs.into_iter().map(|s| s.join("\n")).collect()
}

struct Planner<'o> {
    opts: &'o Table,
    split_level: Option<i64>,
    plan: Vec<PageEntry>,
    nav: Vec<NavItem>,
    chapter_started: bool,
    chapter: String,
    last_path: Vec<String>,
    cover_re: Rx,
    chapter_image_re: Option<Rx>,
}

impl<'o> Planner<'o> {
    fn new(opts: &'o Table, split_level: Option<i64>) -> Result<Self, ConvertError> {
        let cover = get_str(opts, "cover_image").filter(|s| !s.is_empty()).unwrap_or_else(|| "cover\\.[A-Za-z]+".into());
        let cover_re = Rx::try_new(&cover).map_err(|_| ConvertError(format!("設定 cover_image の正規表現が読めません：{}", cover)))?;
        let chapter_image_re = match get_str(opts, "chapter_image").filter(|s| !s.is_empty()) {
            Some(ci) => Some(Rx::try_new(&ci).map_err(|_| ConvertError(format!("設定 chapter_image の正規表現が読めません：{}", ci)))?),
            None => None,
        };
        Ok(Planner { opts, split_level, plan: vec![], nav: vec![], chapter_started: false, chapter: String::new(),
                     last_path: vec![], cover_re, chapter_image_re })
    }

    fn next_fn(&self) -> String { format!("p{:03}.xhtml", self.plan.len() + 1) }

    fn add_nav_path(&mut self, path: &str, fn_: &str) -> String {
        let parts: Vec<String> = path.split('／').map(|p| py_strip(p).to_string()).filter(|p| !p.is_empty()).collect();
        let pos = self.plan.len();
        for depth in 1..parts.len() {
            let prefix = &parts[..depth];
            if self.last_path.len() < depth || &self.last_path[..depth] != prefix {
                let mut it = NavItem::new(pos, -1, depth as i64, &parts[depth - 1], fn_);
                it.group = true;
                self.nav.push(it);
            }
        }
        let leaf = parts.last().cloned().unwrap_or_default();
        self.nav.push(NavItem::new(pos, -1, parts.len() as i64, &leaf, fn_));
        self.last_path = parts;
        leaf
    }

    fn opener(&mut self, ctx: &mut Context, img: &str, path: Option<&str>, raw_rest: &str) {
        if !ctx.images_src.exists(img) {
            ctx.where_ = self.next_fn();
            ctx.missing.push(img.into());
            ctx.warn(format!("章扉の画像が見つからない：{}", img));
        }
        let fn_ = self.next_fn();
        let title = match path { Some(p) if !p.is_empty() => self.add_nav_path(p, &fn_), _ => String::new() };
        self.chapter = title.clone();
        let mut e = PageEntry::new("opener", String::new(), fn_);
        e.img = Some(img.into());
        e.title = title;
        self.plan.push(e);
        self.chapter_started = true;
        if !py_strip(raw_rest).is_empty() { self.text(ctx, raw_rest, true); }
    }

    fn text(&mut self, ctx: &mut Context, raw: &str, after_opener: bool) {
        let after_opener = after_opener || self.plan.last().map(|e| e.kind == "opener").unwrap_or(false);
        let s = strip_chars(raw, "\n");
        let mut lines: Vec<&str> = Vec::new();
        let mut declared = String::new();
        for l in s.split('\n') {
            if let Some(m) = SECTION_RE.fullmatch(py_strip(l)) {
                let nf = self.next_fn();
                let leaf = self.add_nav_path(g(&m, 1).unwrap(), &nf);
                if declared.is_empty() { declared = leaf; }
                self.chapter = declared.clone();
                self.chapter_started = true;
                continue;
            }
            lines.push(l);
        }
        let first = lines.iter().map(|l| py_strip(l)).find(|l| !l.is_empty()).unwrap_or("");
        if self.split_level.is_some() && starts_chapter(first, self.split_level) {
            self.chapter_started = true;
            self.last_path = vec![];
            let t = heading_title(&lines);
            if !t.is_empty() { self.chapter = t; }
        }
        let (mut body, notes) = split_notes(ctx, &lines.join("\n"));
        if !py_strip(&body).is_empty() {
            let lead = if after_opener { (raw.len() - lstrip_chars(raw, "\n").len()).saturating_sub(1) } else { 0 };
            if get_truthy(self.opts, "squeeze_blank_lines") && self.chapter_started
                && body.split('\n').any(|l| py_strip(l).starts_with("［＃ここから")) {
                body = squeeze_blank(&body);
            }
            let mut e = PageEntry::new("text", rstrip_chars(&body, "\n").to_string(), self.next_fn());
            e.title = declared;
            e.lead = lead;
            e.chapter = self.chapter.clone();
            self.plan.push(e);
        }
        if let Some(n) = notes {
            let mut e = PageEntry::new("notes", n, self.next_fn());
            e.chapter = self.chapter.clone();
            self.plan.push(e);
        }
    }

    fn page(&mut self, ctx: &mut Context, p: &str) {
        let s = strip_chars(p, "\n");
        if py_strip(s).is_empty() { return; }
        if let Some(m) = FIG_RE.fullmatch(py_strip(s)) {
            let name = g(&m, 1).unwrap().to_string();
            if self.cover_re.is_full(&name) && ctx.cover.is_none() {
                ctx.cover = Some(name.clone());
                ctx.add_image(&name);
                return;
            }
            if let Some(cre) = &self.chapter_image_re {
                if let Some(mm) = cre.fullmatch(&name) {
                    let n = if mm.len() > 1 { num(g(&mm, 1).unwrap_or("")) } else { self.plan.len() as i64 + 1 };
                    self.opener(ctx, &name, Some(&format!("第{}章", n)), "");
                    return;
                }
            }
        }
        for seg in segments(p, self.split_level) {
            let lines: Vec<&str> = seg.split('\n').collect();
            let Some(idx) = lines.iter().position(|l| !py_strip(l).is_empty()) else { continue };
            if let Some(mo) = OPENER_RE.fullmatch(py_strip(lines[idx])) {
                let img = g(&mo, 1).unwrap().to_string();
                let path = g(&mo, 2).map(|s| s.to_string());
                self.opener(ctx, &img, path.as_deref(), &lines[idx + 1..].join("\n"));
            } else {
                self.text(ctx, &seg, false);
            }
        }
    }
}

fn heading_title(lines: &[&str]) -> String {
    let content: Vec<&str> = lines.iter().map(|l| py_strip(l)).filter(|l| !l.is_empty()).collect();
    let Some(c0) = content.first() else { return String::new() };
    if let Some((_, rest)) = heading_of_line(c0) { return plain_title(&rest); }
    if BLOCK_HEAD_START_RE.is_full(c0) && content.len() > 1 { return plain_title(content[1]); }
    String::new()
}

fn figure_title(lines: &[&str]) -> String {
    for l in lines {
        if let Some(m) = FIG_RE.fullmatch(py_strip(l)) {
            let cap = plain_title(g(&m, 2).unwrap_or("").split('／').next().unwrap_or(""));
            return if cap.is_empty() { "図版".into() } else { cap };
        }
    }
    "図版".into()
}

fn squeeze_blank(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for l in text.split('\n') {
        if py_strip(l).is_empty() && out.last().map(|x| py_strip(x).is_empty()).unwrap_or(false) { continue; }
        out.push(l);
    }
    out.join("\n")
}

// ---------------------------------------------------------------- 生成

static SPACES_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("[　 ]+"));

fn text_page(ctx: &mut Context, e: &PageEntry, pos: usize) -> (String, String) {
    let all: Vec<&str> = e.text.split('\n').collect();
    let center = all.iter().any(|l| py_strip(l) == "［＃ページの左右中央］");
    let lines: Vec<&str> = all.into_iter().filter(|l| py_strip(l) != "［＃ページの左右中央］").collect();
    let only_fig = lines.iter().all(|l| FIG_RE.is_full(py_strip(l)) || py_strip(l).is_empty());
    let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut body = lines_html(ctx, &owned, RenderPos { pos, mode: "body" });
    if e.lead > 0 { body = format!("{}{}", "<p class=\"blank\">　</p>".repeat(e.lead), body); }
    let first_line = lines.iter().map(|l| plain_title(l)).find(|t| !t.is_empty()).unwrap_or_default();
    let first = first_line.replace('　', "");
    let first_line = SPACES_RE.sub(&first_line, "　");
    let title = if !e.title.is_empty() { e.title.clone() }
        else if first == "目次" || lines.iter().any(|l| py_strip(l) == TOC_LINE) { "目次".into() }
        else if only_fig { figure_title(&lines) }
        else if !e.chapter.is_empty() { e.chapter.clone() }
        else if first_line.is_empty() { "本文".into() }
        else { prefix_chars(&first_line, 20).to_string() };
    if center { body = format!("<div class=\"centerbox\">{}</div>", body); }
    let cls = if only_fig { "figpage" } else if center { "center" } else { "" };
    (xhtml(&title, &body, cls), first)
}

/// 手作りの目次頁：目次の項目と同じ文字の行をリンクにする
fn link_toc_page(x0: &str, items: &[NavItem]) -> String {
    let mut x = x0.to_string();
    for it in items {
        let chars: Vec<String> = it.title.chars().filter(|c| *c != '　').map(|c| fancy_regex::escape(&esc(&c.to_string())).into_owned()).collect();
        let pat = format!(">({})</p>", chars.join("[　]*"));
        let Ok(rx) = Rx::try_new(&pat) else { continue };
        let href = it.href();
        x = rx.sub_fn(&x, 1, |m| format!("><a href=\"{}\">{}</a></p>", href, g(m, 1).unwrap()));
    }
    x
}

fn toc_html(items: &[NavItem]) -> String {
    let rows: Vec<String> = items.iter()
        .map(|it| format!("<p class=\"toc{}\"><a href=\"{}\">{}</a></p>", it.level.min(3), it.href(), esc(&it.title)))
        .collect();
    format!("<div class=\"tocpage\">\n{}\n</div>", rows.join("\n"))
}

#[derive(Clone, Debug)]
pub struct OutputFile { pub name: String, pub xhtml: String, pub kind: String }

pub fn build_pages(ctx: &mut Context, raw: &str, pages: &[&str], opts: &Table, fallback_title: &str)
    -> Result<(Vec<OutputFile>, Vec<NavItem>), ConvertError> {
    let split_level = levels_used(raw).into_iter().min();
    let mut pl = Planner::new(opts, split_level)?;
    for (i, p) in pages.iter().enumerate() {
        let n = pl.plan.len();
        pl.page(ctx, p);
        for e in &pl.plan[n..] { ctx.src_pages.insert(e.fn_.clone(), i); }
    }
    let plan = pl.plan.clone();

    let mut blocks = std::collections::HashMap::new();
    for (pos, e) in plan.iter().enumerate() {
        if e.kind == "notes" {
            ctx.where_ = e.fn_.clone();
            blocks.insert(pos, register(ctx, pos, &e.fn_, &e.text));
        }
    }
    assign_scopes(ctx);
    let refs: Vec<(&str, &str, &str)> = plan.iter()
        .map(|e| (if e.kind == "notes" { "notes" } else { "body" }, e.text.as_str(), e.fn_.as_str())).collect();
    collect_backrefs(ctx, &refs);

    let mut files: Vec<OutputFile> = Vec::new();
    let mut toc_pages: Vec<usize> = Vec::new();
    for (pos, e) in plan.iter().enumerate() {
        ctx.where_ = e.fn_.clone();
        ctx.pos = pos;
        if e.kind == "opener" {
            let t = if e.title.is_empty() { "扉".to_string() } else { e.title.clone() };
            let img = e.img.clone().unwrap_or_default();
            let inner = if ctx.missing.contains(&img) {
                format!("<p class=\"missing\">［図版未着：{}］{}</p>", esc(&img), esc(&t))
            } else {
                ctx.add_image(&img);
                format!("<figure class=\"fig\"><img src=\"images/{}\" alt=\"{}\"/></figure>", img, esc(&t))
            };
            let body = format!("<section epub:type=\"chapter\" aria-label=\"{}\">{}</section>", esc(&t), inner);
            files.push(OutputFile { name: e.fn_.clone(), xhtml: xhtml(&t, &body, "figpage"), kind: "opener".into() });
        } else if e.kind == "notes" {
            let h = notes_html(ctx, blocks[&pos], pos);
            files.push(OutputFile { name: e.fn_.clone(), xhtml: xhtml("注", &h, ""), kind: "notes".into() });
        } else {
            let (x, first) = text_page(ctx, e, pos);
            if first == "目次" && !ctx.toc_files.contains(&e.fn_) { toc_pages.push(files.len()); }
            files.push(OutputFile { name: e.fn_.clone(), xhtml: x, kind: "text".into() });
        }
    }

    if let Some(colophon) = get_table(opts, "colophon") {
        let fn_ = format!("p{:03}.xhtml", plan.len() + 1);
        let title = get_str(colophon, "title").unwrap_or_else(|| "奥付".into());
        let empty = Table::new();
        let body = colophon_html(get_table(opts, "book").unwrap_or(&empty), colophon, fallback_title);
        files.push(OutputFile { name: fn_.clone(), xhtml: xhtml(&title, &body, "colophon"), kind: "colophon".into() });
        pl.nav.push(NavItem::new(plan.len(), -1, 1, &title, &fn_));
    }
    let mut nav = nav_items(ctx, &pl, split_level);
    let leaves: Vec<NavItem> = nav.iter().filter(|it| !it.group).cloned().collect();
    for i in toc_pages { files[i].xhtml = link_toc_page(&files[i].xhtml, &leaves); }
    let toc = toc_html(&nav);
    for f in files.iter_mut() {
        if ctx.toc_files.contains(&f.name) { f.xhtml = f.xhtml.replace(TOC_PLACEHOLDER, &toc); }
    }
    if nav.is_empty() && !files.is_empty() {
        ctx.where_ = String::new();
        ctx.warn("章が見つからなかったので、目次には最初の頁だけを入れた。［＃章扉（画像）：題名］［＃章：題名］か見出し注記で章を示してください");
        nav = vec![NavItem::new(0, 0, 1, "本文", &files[0].name)];
    }
    Ok((files, nav))
}

fn nav_items(ctx: &Context, pl: &Planner, split_level: Option<i64>) -> Vec<NavItem> {
    let mut items = pl.nav.clone();
    let opener_pos: HashSet<usize> = pl.plan.iter().enumerate().filter(|(_, e)| e.kind == "opener").map(|(i, _)| i).collect();
    let mut seen_first: HashSet<String> = HashSet::new();
    for (seq, h) in ctx.headings.iter().enumerate() {
        let lv = h.level - split_level.unwrap_or(h.level) + 1;
        let first_in_file = !seen_first.contains(&h.file);
        seen_first.insert(h.file.clone());
        if lv == 1 && first_in_file && h.pos >= 1 && opener_pos.contains(&(h.pos - 1)) {
            if items.iter().any(|it| it.pos == h.pos - 1 && !it.group) { continue; }
            items.push(NavItem::new(h.pos - 1, -1, 1, &h.title, &pl.plan[h.pos - 1].fn_));
            continue;
        }
        let mut it = NavItem::new(h.pos, seq as i64, lv, &h.title, &h.file);
        if !(lv == 1 && first_in_file) { it.anchor = h.anchor.clone(); }
        items.push(it);
    }
    items.sort_by_key(|it| (it.pos, it.seq)); // 安定な並べ替え（Python と同じ）
    items
}

pub fn nav_html(nav: &[NavItem]) -> String {
    fn build(items: &[NavItem]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < items.len() {
            let it = &items[i];
            let mut j = i + 1;
            while j < items.len() && items[j].level > it.level { j += 1; }
            let a = format!("<a href=\"{}\">{}</a>", it.href(), esc(&it.title));
            if j > i + 1 { out.push_str(&format!("<li>{}<ol>{}</ol></li>", a, build(&items[i + 1..j]))); }
            else { out.push_str(&format!("<li>{}</li>", a)); }
            i = j;
        }
        out
    }
    build(nav)
}
