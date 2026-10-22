//! 段落と範囲注記（blocks.py）

use crate::basics::*;
use crate::context::Context;
use crate::inline::*;
use crate::ir::*;
use crate::py::*;
use std::sync::LazyLock;

pub const BLANK: &str = "<p class=\"blank\">　</p>";
pub const TOC_PLACEHOLDER: &str = "<!--egword2epub:toc-->";
pub static FIG_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃(?:図|挿絵)（([^）]+)）入る(?:：(.*))?］"));
static SEP_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃区切り線(?:：(.*)|（([^）]*)）)?］"));
pub static BLOCK_START_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃ここから(.+)］"));
pub static BLOCK_END_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃ここで(.+)終わり］"));

const N: &str = "([０-９\\d]+)";
static SPEC_SPLIT: LazyLock<Rx> = LazyLock::new(|| Rx::new("[、,]"));
static SPEC_FIRST: LazyLock<Rx> = LazyLock::new(|| Rx::new(&format!("(?:天から)?{}字下げ", N)));
static SPEC_WRAP: LazyLock<Rx> = LazyLock::new(|| Rx::new(&format!("折り返して{}字下げ", N)));
static SPEC_BOTTOM: LazyLock<Rx> = LazyLock::new(|| Rx::new(&format!("地から{}字上げ", N)));
static MARK_COMBO: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃([^］「]*、[^］]*)］"));
static MARK_ONE: LazyLock<Rx> = LazyLock::new(|| Rx::new(&format!("［＃(地付き|地から{}字上げ|(?:天から)?{}字下げ)］", N, N)));
static MARK_END: LazyLock<Rx> = LazyLock::new(|| Rx::new("［＃地付き］$"));
static TRAIL_SPACE: LazyLock<Rx> = LazyLock::new(|| Rx::new("　+$"));
static SEP_LEN: LazyLock<Rx> = LazyLock::new(|| Rx::new("([０-９\\d]+)字"));
static ALT_RUBY: LazyLock<Rx> = LazyLock::new(|| Rx::new("《[^》]*》"));

/// 範囲注記の指定（「２字下げ、折り返して２字下げ、地から２字上げ」を読んだもの）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockSpec {
    pub first: Option<i64>,
    pub wrap: Option<i64>,
    pub bottom: Option<i64>,
    pub size: Option<String>,
    pub end: bool,
    pub center: bool,
}

impl BlockSpec {
    fn only_size(&self) -> bool {
        self.first.is_none() && self.wrap.is_none() && self.bottom.is_none() && !self.end && !self.center
    }
    /// Python の dict.update に当たる（d に書いてあるものだけ上書き）
    fn update(&mut self, d: &BlockSpec) {
        if d.first.is_some() { self.first = d.first; }
        if d.wrap.is_some() { self.wrap = d.wrap; }
        if d.bottom.is_some() { self.bottom = d.bottom; }
        if d.size.is_some() { self.size = d.size.clone(); }
        if d.end { self.end = true; }
        if d.center { self.center = true; }
    }
}

pub fn parse_block_spec(spec: &str, mut ctx: Option<&mut Context>) -> BlockSpec {
    let mut d = BlockSpec::default();
    for p in SPEC_SPLIT.split(spec) {
        let part = py_strip(&p);
        if let Some(m) = SPEC_FIRST.fullmatch(part) { d.first = Some(num(g(&m, 1).unwrap())); continue; }
        if let Some(m) = SPEC_WRAP.fullmatch(part) { d.wrap = Some(num(g(&m, 1).unwrap())); continue; }
        if let Some(m) = SPEC_BOTTOM.fullmatch(part) { d.bottom = Some(num(g(&m, 1).unwrap())); continue; }
        if let Some(m) = SIZE_RE.fullmatch(part) { d.size = Some(size_class(g(&m, 1).unwrap(), g(&m, 2).unwrap())); continue; }
        if part == "改行天付き" { d.first = Some(0); continue; }
        if part == "地付き" { d.end = true; continue; }
        if part == "中央揃え" || part == "中央" { d.center = true; continue; }
        if let Some(c) = ctx.as_deref_mut() {
            c.warn(format!("注記の中の「{}」は読めないので無視した（［＃{}］）", part, spec));
        }
    }
    d
}

/// 範囲注記の入れ子
fn nest_block(outer: Option<&BlockSpec>, inner: BlockSpec) -> BlockSpec {
    let outer = outer.cloned().unwrap_or_default();
    if inner.only_size() {
        let mut r = outer;
        if inner.size.is_some() { r.size = inner.size; }
        return r;
    }
    if outer.size.is_some() && inner.size.is_none() {
        let mut r = inner; r.size = outer.size; return r;
    }
    inner
}

/// 行頭・行末の行注記を読み取り、block を更新して残りの行を返す
fn line_marks(line0: &str, block: &mut BlockSpec, ctx: &mut Context) -> String {
    let mut line = line0.to_string();
    loop {
        if line.starts_with("［＃中央揃え］") || line.starts_with("［＃中央］") {
            block.center = true;
            let i = line.find('］').unwrap() + '］'.len_utf8();
            line = line[i..].to_string();
            continue;
        }
        if let Some(mc) = MARK_COMBO.match_(&line) {
            let spec = g(&mc, 1).unwrap().to_string();
            if !spec.starts_with("ここ") {
                let d = parse_block_spec(&spec, Some(ctx));
                if d.bottom.is_some() || d.end { block.end = true; }
                block.update(&d);
                line = line[end(&mc)..].to_string();
                continue;
            }
        }
        let Some(m) = MARK_ONE.match_(&line) else { break };
        if g(&m, 1).unwrap() == "地付き" {
            block.end = true;
        } else if let Some(g2) = g(&m, 2) {
            block.end = true;
            block.bottom = Some(num(g2));
        } else {
            let n = num(g(&m, 3).unwrap());
            block.first = Some(n);
            if block.wrap.is_none() { block.wrap = Some(n); }
        }
        line = line[end(&m)..].to_string();
    }
    if let Some(m) = MARK_END.search(&line) {
        block.end = true;
        line = line[..start(&m)].to_string();
    }
    line
}

/// 行頭の全角空白の数（字で数える）
fn lead_count(line: &str) -> i64 { line.chars().take_while(|c| *c == '　').count() as i64 }

pub fn parse_para(ctx: &mut Context, line0: &str, block0: Option<&BlockSpec>, cls0: &str) -> Block {
    let mut block = block0.cloned().unwrap_or_default();
    let line = line_marks(line0, &mut block, ctx);
    let lead = lead_count(&line);
    let line = lstrip_chars(&line, "　");
    let first = block.first.unwrap_or(0);
    let wrap = block.wrap.unwrap_or(first);
    let mut st = String::new();
    let ti;
    if block.center { st.push_str("text-align:center;"); ti = 0; }
    else if block.end { st.push_str("text-align:end;"); ti = 0; }
    else { ti = (first - wrap) + lead; }
    if wrap != 0 { st.push_str(&format!("margin-top:{}em;", wrap)); }
    if let Some(b) = block.bottom { if b != 0 { st.push_str(&format!("margin-bottom:{}em;", b)); } }
    st.push_str(&format!("text-indent:{}em;", ti));
    let cls = [cls0, block.size.as_deref().unwrap_or("")].iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    Block::Para { nodes: parse_inline(ctx, line), style: st, tail: String::new(), cls }
}

fn parse_yoko(ctx: &mut Context, line0: &str) -> Block {
    let mut block = BlockSpec::default();
    let line = line_marks(line0, &mut block, ctx);
    let lead = lead_count(&line);
    let align = if block.center { "center" } else if block.end { "end" } else { "" };
    let right = if align == "end" { block.bottom.unwrap_or(0) } else { 0 };
    Block::Yoko { nodes: parse_inline(ctx, lstrip_chars(&line, "　")), indent: if align.is_empty() { lead } else { 0 },
                  align: align.into(), right }
}

fn parse_sep(spec: &str) -> Block {
    let mut style = "solid";
    for (k, v) in [("破線", "dashed"), ("点線", "dotted"), ("二重線", "double"), ("実線", "solid")] {
        if spec.contains(k) { style = v; }
    }
    let length = SEP_LEN.search(spec).map(|m| num(g(&m, 1).unwrap())).unwrap_or(8);
    Block::Sep { style: style.into(), length }
}

fn parse_figure(ctx: &mut Context, fname: &str, cap: Option<&str>) -> Block {
    if !ctx.images_src.exists(fname) {
        ctx.missing.push(fname.into());
        ctx.warn(format!("画像が見つからない：{}", fname));
        return Block::Missing { fname: fname.into(), caption: cap.unwrap_or("").into() };
    }
    ctx.add_image(fname);
    let alt = ALT_RUBY.sub(&cap.unwrap_or("").replace('｜', ""), "").replace('／', "、");
    let caption = match cap {
        Some(c) if !c.is_empty() => c.split('／').map(|p| parse_inline(ctx, p)).collect(),
        _ => vec![],
    };
    Block::Figure { fname: fname.into(), alt, caption }
}

fn parse_heading(ctx: &mut Context, level: i64, line: &str, block: Option<&BlockSpec>) -> Block {
    let Block::Para { nodes, style, .. } = parse_para(ctx, line, block, "") else { unreachable!() };
    let anchor = ctx.add_heading(level, plain_title(line));
    Block::Heading { level, nodes, style, anchor }
}

/// 一頁ぶんの行を段落要素の並びに
pub fn parse_lines(ctx: &mut Context, lines0: &[String]) -> Vec<Block> {
    let mut a = 0;
    let mut b = lines0.len();
    while a < b && py_strip(&lines0[a]).is_empty() { a += 1; }
    while b > a && py_strip(&lines0[b - 1]).is_empty() { b -= 1; }
    let lines = &lines0[a..b];

    let mut root: Vec<Block> = Vec::new();
    let mut yoko: Option<Vec<Block>> = None;
    let mut block: Option<BlockSpec> = None;
    let mut stack: Vec<Option<BlockSpec>> = Vec::new();
    let mut head_block: Option<i64> = None;
    macro_rules! append { ($x:expr) => {{ let v = $x; match yoko.as_mut() { Some(y) => y.push(v), None => root.push(v) } }}; }

    for l in lines {
        let s = py_strip(l);
        if s == "［＃ここから横組み］" {
            if yoko.is_some() { ctx.warn("［＃ここから横組み］が閉じないうちに、また［＃ここから横組み］がある"); continue; }
            yoko = Some(Vec::new());
            continue;
        }
        if s == "［＃ここで横組み終わり］" {
            match yoko.take() {
                None => { ctx.warn("［＃ここで横組み終わり］に対応する［＃ここから横組み］がない"); }
                Some(y) => root.push(Block::Div { cls: "yoko".into(), children: y }),
            }
            continue;
        }
        if let Some(m) = BLOCK_HEAD_START_RE.fullmatch(s) { head_block = Some(level_of(g(&m, 1).unwrap())); continue; }
        if BLOCK_HEAD_END_RE.is_full(s) {
            if head_block.is_none() { ctx.warn(format!("{} に対応する［＃ここから…見出し］がない", s)); }
            head_block = None;
            continue;
        }
        if s == TOC_LINE { append!(Block::TocMark); continue; }
        if let Some(m) = BLOCK_START_RE.fullmatch(s) {
            let spec = parse_block_spec(g(&m, 1).unwrap(), Some(ctx));
            let nb = nest_block(block.as_ref(), spec);
            stack.push(block.take());
            block = Some(nb);
            continue;
        }
        if BLOCK_END_RE.is_full(s) {
            if stack.is_empty() { ctx.warn(format!("{} に対応する［＃ここから…］がない", s)); }
            block = stack.pop().flatten();
            continue;
        }
        if yoko.is_some() {
            if s.is_empty() { append!(Block::Blank); continue; }
            let y = parse_yoko(ctx, l);
            append!(y);
            continue;
        }
        if let Some(m) = SEP_RE.fullmatch(s) { append!(parse_sep(g(&m, 1).or(g(&m, 2)).unwrap_or(""))); continue; }
        if let Some(m) = FIG_RE.fullmatch(s) {
            let f = parse_figure(ctx, g(&m, 1).unwrap(), g(&m, 2));
            append!(f);
            continue;
        }
        if s.is_empty() { append!(Block::Blank); continue; }
        let line = TRAIL_SPACE.sub(&l.replace('\t', "　"), "");
        if let Some((lv, rest)) = heading_of_line(&line) {
            let h = parse_heading(ctx, lv, &rest, block.as_ref());
            append!(h);
            continue;
        }
        if let Some(hb) = head_block {
            let h = parse_heading(ctx, hb, &line, block.as_ref());
            append!(h);
            continue;
        }
        let p = parse_para(ctx, &line, block.as_ref(), "");
        append!(p);
    }
    if head_block.is_some() { ctx.warn("［＃ここから…見出し］が頁の終わりまで閉じていない"); }
    if let Some(y) = yoko {
        root.push(Block::Div { cls: "yoko".into(), children: y });
        ctx.warn("［＃ここから横組み］が頁の終わりまで閉じていない（頁の終わりで閉じた）");
    }
    if !stack.is_empty() { ctx.warn(format!("［＃ここから…］が{}つ、頁の終わりまで閉じていない", stack.len())); }
    root
}

// ---------------------------------------------------------------- 生成

fn style(ctx: &Context, s: &str) -> String {
    if ctx.horizontal { s.replace("margin-top:", "margin-left:").replace("margin-bottom:", "margin-right:") } else { s.to_string() }
}

pub fn render_block(ctx: &mut Context, b: &Block, rp: RenderPos) -> String {
    match b {
        Block::Para { nodes, style: st, tail, cls } => {
            let body = render_inline(ctx, nodes, rp);
            if py_strip(&body).is_empty() { return BLANK.into(); }
            let c = if cls.is_empty() { String::new() } else { format!(" class=\"{}\"", cls) };
            format!("<p{} style=\"{}\">{}{}</p>", c, style(ctx, st), body, tail)
        }
        Block::Heading { level, nodes, style: st, anchor } => {
            let tag = format!("h{}", level + 1);
            let inner = render_inline(ctx, nodes, rp);
            format!("<{t} class=\"{}\" id=\"{}\" style=\"{}\">{}</{t}>", head_class(*level), anchor, style(ctx, st), inner, t = tag)
        }
        Block::TocMark => { ctx.toc_files.insert(ctx.where_.clone()); TOC_PLACEHOLDER.into() }
        Block::Blank => BLANK.into(),
        Block::Yoko { nodes, indent, align, right } => {
            let a = if align.is_empty() { String::new() } else { format!("text-align:{};", align) };
            let r = if *right != 0 { format!("margin-right:{}em;", right) } else { String::new() };
            format!("<p style=\"{}{}text-indent:{}em;\">{}</p>", a, r, indent, render_inline(ctx, nodes, rp))
        }
        Block::Div { cls, children } => {
            let mut parts = vec![format!("<div class=\"{}\">", cls)];
            for c in children { parts.push(render_block(ctx, c, rp)); }
            parts.push("</div>".into());
            parts.join("\n")
        }
        Block::Sep { style: st, length } => {
            let width = if st == "double" { "3px" } else { "1px" };
            if ctx.horizontal {
                format!("<div class=\"sep\" style=\"width:{}em;height:0;margin:1em auto;border-top:{} {} currentColor;\"></div>", length, width, st)
            } else {
                format!("<div class=\"sep\" style=\"height:{}em;border-left:{} {} currentColor;\"></div>", length, width, st)
            }
        }
        Block::Missing { fname, caption } =>
            format!("<p class=\"missing\">［図版未着：{}］{}</p>", esc(fname), esc_nq(&caption.replace('／', " "))),
        Block::Figure { fname, alt, caption } => {
            let mut capx = String::new();
            if !caption.is_empty() {
                let cp = RenderPos { pos: rp.pos, mode: "caption" };
                let parts: Vec<String> = caption.iter().map(|c| render_inline(ctx, c, cp)).collect();
                capx = format!("<figcaption>{}</figcaption>", parts.join("<br/>"));
            }
            format!("<figure class=\"fig\"><img src=\"images/{}\" alt=\"{}\"/>{}</figure>", esc(fname), esc(alt), capx)
        }
    }
}

pub fn render_blocks(ctx: &mut Context, blocks: &[Block], rp: RenderPos) -> String {
    blocks.iter().map(|b| render_block(ctx, b, rp)).collect::<Vec<_>>().join("\n")
}

pub fn lines_html(ctx: &mut Context, lines: &[String], rp: RenderPos) -> String {
    let blocks = parse_lines(ctx, lines);
    render_blocks(ctx, &blocks, rp)
}
