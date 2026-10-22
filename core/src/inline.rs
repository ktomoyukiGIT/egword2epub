//! 行内の注記（inline.py）

use crate::basics::*;
use crate::context::Context;
use crate::ir::*;
use crate::py::*;
use std::collections::HashSet;
use std::sync::LazyLock;

static URL_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("(https?://[A-Za-z0-9\\-._~:/?#\\[\\]@!$&;=%+,]+[A-Za-z0-9/#=_\\-])"));
static MAIL_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("(?<![\\w.@/])([A-Za-z0-9._%+\\-]+@[A-Za-z0-9\\-]+(?:\\.[A-Za-z0-9\\-]+)+)"));
static XH_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("((?:Ｘ|X)\\s*[：:]\\s*)@([A-Za-z0-9_]{1,15})"));
static POSTFIX_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("「([^」]+)」[はに](.+)"));
pub static SIZE_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new(SIZE_PATTERN));
static GAIJI_UCS_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("U\\+([0-9A-Fa-f]{4,6})"));
static TAG_SPLIT: LazyLock<Rx> = LazyLock::new(|| Rx::new("(<[^>]+>)"));

fn size_end(body: &str) -> Option<&'static str> {
    match body { "小さな文字終わり" => Some("小さな文字"), "大きな文字終わり" => Some("大きな文字"), _ => None }
}

fn base_pattern() -> String {
    format!(concat!(
        "(?P<gaiji>※［＃(?P<gaiji_body>[^］]*)］)",
        "|(?P<ann>［＃(?P<ann_body>[^］]*)］)",
        "|(?P<per2>《《(?P<pname2>[^》]+)》》)",
        "|｜(?P<xbt>[^｜《]+)《﹅+》",
        "|｜(?P<xrb>[^｜《]+)《(?P<xrt>[^《》﹅]+)》",
        "|(?P<rb>{}+)《(?P<rt>[^《》﹅]+)》",
        "|(?P<bt>《(?P<dots>﹅+)》)",
        "|(?P<per>《(?P<pname>[^》]+)》)"), KANJI)
}

/// 注記の名前（「傍線」「１段階小さな文字」など）→ class
pub fn decoration_class(name: &str) -> Option<String> {
    if let Some(c) = decoration(name) { return Some(c.to_string()); }
    SIZE_RE.fullmatch(name).map(|m| size_class(g(&m, 1).unwrap(), g(&m, 2).unwrap()))
}

fn token_re<'c>(ctx: &'c mut Context) -> &'c Rx {
    if ctx.token_re.is_none() {
        let mut pat = base_pattern();
        for s in &ctx.systems { pat.push_str(&format!("|(?P<{}>{})", s.key, s.ref_pattern)); }
        ctx.token_re = Some(Rx::new(&pat));
    }
    ctx.token_re.as_ref().unwrap()
}

// ---------------------------------------------------------------- 解析

fn push_text(nodes: &mut Vec<Node>, s: &str) {
    if s.is_empty() { return; }
    if let Some(Node::Text(t)) = nodes.last_mut() { t.push_str(s); } else { nodes.push(Node::Text(s.to_string())); }
}

fn marks(nodes: &[Node], top: bool, out: &mut Vec<bool>) {
    for n in nodes {
        match n {
            Node::RangeMark { .. } => out.push(top),
            Node::Span(_, c) => marks(c, false, out),
            Node::Ruby(b, _) => marks(b, false, out),
            _ => {}
        }
    }
}

fn strip_marks(nodes: &[Node]) -> Vec<Node> {
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::RangeMark { .. } => continue,
            Node::Span(cls, c) => out.push(Node::Span(cls.clone(), strip_marks(c))),
            Node::Ruby(b, rt) => out.push(Node::Ruby(strip_marks(b), rt.clone())),
            _ => out.push(n.clone()),
        }
    }
    out
}

fn resolve_ranges(ctx: &mut Context, nodes: Vec<Node>) -> Vec<Node> {
    let mut ms = Vec::new();
    marks(&nodes, true, &mut ms);
    if ms.is_empty() { return nodes; }
    if !ms.iter().all(|t| *t) {
        ctx.warn("範囲の文字飾りの注記が、ほかの文字飾りやルビの中にあるので無視した");
        return strip_marks(&nodes);
    }
    let mut out: Vec<Node> = Vec::new();
    let mut stack: Vec<(String, String, Vec<Node>)> = Vec::new();
    for n in nodes {
        if let Node::RangeMark { key, open, cls } = &n {
            if *open {
                stack.push((key.clone(), cls.clone(), std::mem::take(&mut out)));
            } else {
                if stack.last().map(|t| &t.0 != key).unwrap_or(true) {
                    ctx.warn(format!("［＃{}終わり］に対応する始まりがない（または入れ子が崩れている）", key));
                    continue;
                }
                let (_, cls, mut outer) = stack.pop().unwrap();
                outer.push(Node::Span(cls, std::mem::take(&mut out)));
                out = outer;
            }
        } else {
            out.push(n);
        }
    }
    while let Some((key, _, mut outer)) = stack.pop() {
        ctx.warn(format!("［＃{}］が行の終わりまで閉じていない（文字飾りにしなかった）", key));
        outer.append(&mut out);
        out = outer;
    }
    out
}

fn gaiji(ctx: &mut Context, body: &str) -> String {
    if let Some(m) = GAIJI_UCS_RE.search(body) {
        if let Some(c) = u32::from_str_radix(g(&m, 1).unwrap(), 16).ok().and_then(char::from_u32) {
            return c.to_string();
        }
    }
    ctx.warn(format!("外字注記を〓にした（U+ の符号位置がない）：※［＃{}］", body));
    "〓".into()
}

fn annotation(ctx: &mut Context, mut nodes: Vec<Node>, body: &str) -> Vec<Node> {
    if let Some(pm) = POSTFIX_RE.fullmatch(body) {
        if let Some(cls) = decoration_class(g(&pm, 2).unwrap()) {
            let target = g(&pm, 1).unwrap();
            if let Some(k) = plain(&nodes).rfind(target) {
                return wrap(&nodes, k, k + target.len(), &cls);
            }
            ctx.warn(format!("後置注記の対象「{}」が、注記より前の同じ行に見つからない", target));
            return nodes;
        }
    }
    if let Some(cls) = decoration_class(body) {
        let key = if decoration(body).is_none() {
            format!("{}文字", g(&SIZE_RE.fullmatch(body).unwrap(), 2).unwrap())
        } else { body.to_string() };
        nodes.push(Node::RangeMark { key, open: true, cls });
        return nodes;
    }
    if let Some(stem) = body.strip_suffix("終わり") {
        let key = size_end(body).unwrap_or(stem);
        if decoration(key).is_some() || key == "小さな文字" || key == "大きな文字" {
            nodes.push(Node::RangeMark { key: key.to_string(), open: false, cls: String::new() });
            return nodes;
        }
    }
    ctx.warn(format!("未対応の注記を取り除いた：［＃{}］", body));
    nodes
}

/// 一行ぶんのテキストを行内要素の並びにする
pub fn parse_inline(ctx: &mut Context, text: &str) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut pos = 0;
    let caps: Vec<_> = token_re(ctx).finditer(text).collect();
    let keys: Vec<String> = ctx.systems.iter().map(|s| s.key.clone()).collect();
    for m in caps {
        push_text(&mut nodes, &text[pos..start(&m)]);
        pos = end(&m);
        if has(&m, "gaiji") {
            let s = gaiji(ctx, gn(&m, "gaiji_body").unwrap());
            push_text(&mut nodes, &s);
        } else if has(&m, "ann") {
            nodes = annotation(ctx, nodes, gn(&m, "ann_body").unwrap());
        } else if has(&m, "per2") {
            push_text(&mut nodes, &format!("《{}》", gn(&m, "pname2").unwrap()));
        } else if has(&m, "xbt") {
            nodes.push(Node::Span("bouten".into(), vec![Node::Text(gn(&m, "xbt").unwrap().into())]));
        } else if has(&m, "xrb") {
            nodes.push(Node::Ruby(vec![Node::Text(gn(&m, "xrb").unwrap().into())], gn(&m, "xrt").unwrap().into()));
        } else if has(&m, "rb") {
            let (rb, rt) = (gn(&m, "rb").unwrap(), gn(&m, "rt").unwrap());
            if ctx.periodicals.contains(rt) {
                push_text(&mut nodes, &format!("{}《{}》", rb, rt));
            } else {
                nodes.push(Node::Ruby(vec![Node::Text(rb.into())], rt.into()));
            }
        } else if has(&m, "bt") {
            let dots = gn(&m, "dots").unwrap();
            let n = dots.chars().count();
            let p = plain(&nodes);
            if p.chars().count() >= n && !suffix_chars(&p, n).contains(OBJ) {
                let k = p.len() - suffix_chars(&p, n).len();
                nodes = wrap(&nodes, k, p.len(), "bouten");
            } else {
                ctx.warn(format!("傍点《{}》の前に、傍点を付ける文字が足りない", dots));
            }
        } else if has(&m, "per") {
            let pname = gn(&m, "pname").unwrap();
            if !ctx.periodicals.contains(pname) {
                let before = suffix_chars(&text[..start(&m)], 6);
                let tail = if before.is_empty() { "" } else { suffix_chars(before, 2) };
                ctx.warn(format!(
                    "「{b}《{p}》」はルビにしていない。漢字以外にルビを付けるときは「｜{t}…《{p}》」のように｜で範囲を示し、誌名なら《《{p}》》と書く",
                    b = before, p = pname, t = tail));
            }
            push_text(&mut nodes, &format!("《{}》", pname));
        } else {
            for k in &keys {
                if has(&m, k) {
                    let n = num(gn(&m, &format!("{}_n", k)).unwrap());
                    nodes.push(Node::NoteRef(k.clone(), n.to_string()));
                    break;
                }
            }
        }
    }
    push_text(&mut nodes, &text[pos..]);
    resolve_ranges(ctx, nodes)
}

// ---------------------------------------------------------------- 生成

#[derive(Clone, Copy)]
pub struct RenderPos<'m> { pub pos: usize, pub mode: &'m str }

/// URL・メールアドレス・「X：@name」をリンクにする（既存の <a> の中は触らない）
pub fn autolink(buf: &str, kinds: &HashSet<String>) -> String {
    let mut out = String::new();
    let mut in_a = false;
    for part in TAG_SPLIT.split(buf) {
        if part.starts_with('<') {
            if part.starts_with("<a ") { in_a = true; }
            if part.starts_with("</a") { in_a = false; }
            out.push_str(&part);
            continue;
        }
        let mut part = part;
        if !in_a && !part.is_empty() {
            if kinds.contains("url") {
                part = URL_RE.sub_fn(&part, 0, |m| { let u = g(m, 1).unwrap(); format!("<a class=\"ext\" href=\"{}\">{}</a>", u, u) });
            }
            if kinds.contains("mail") && !part.contains("<a ") {
                part = MAIL_RE.sub_fn(&part, 0, |m| { let u = g(m, 1).unwrap(); format!("<a class=\"ext\" href=\"mailto:{}\">{}</a>", u, u) });
            }
            if kinds.contains("x") {
                part = XH_RE.sub_fn(&part, 0, |m| format!("{}<a class=\"ext\" href=\"https://x.com/{}\">@{}</a>",
                                                          g(m, 1).unwrap(), g(m, 2).unwrap(), g(m, 2).unwrap()));
            }
        }
        out.push_str(&part);
    }
    out
}

fn render_ref(ctx: &mut Context, system: &str, n: &str, rp: RenderPos) -> String {
    let (cls, disp, popup) = { let s = ctx.system(system); (s.cls.clone(), s.display(n), s.popup) };
    let hit = ctx.resolve(system, n, rp.pos);
    if rp.mode != "body" {
        return match hit {
            Some(h) => format!("<a class=\"{}\" href=\"{}\">{}</a>", cls, ctx.notes_blocks[h].href(system, n), disp),
            None => format!("<span class=\"{}\">{}</span>", cls, disp),
        };
    }
    let scope = hit.map(|h| ctx.notes_blocks[h].scope(system)).unwrap_or_default();
    let mut rid = format!("{}ref{}{}", cls, scope, n);
    if let Some(c) = ctx.seen.get_mut(&rid) {
        *c += 1;
        rid = format!("{}-{}", rid, c);
    } else {
        ctx.seen.insert(rid.clone(), 1);
    }
    match hit {
        None => format!("<span class=\"{}\">{}</span>", cls, disp),
        Some(h) => format!("<a{} class=\"{}\" id=\"{}\" href=\"{}\">{}</a>",
                           if popup { " epub:type=\"noteref\"" } else { "" }, cls, rid,
                           ctx.notes_blocks[h].href(system, n), disp),
    }
}

pub fn render_inline(ctx: &mut Context, nodes: &[Node], rp: RenderPos) -> String {
    let mut out = String::new();
    let mut buf = String::new();
    for n in nodes {
        if let Node::Text(s) = n { buf.push_str(s); continue; }
        if !buf.is_empty() { out.push_str(&autolink(&esc_nq(&buf), &ctx.autolink)); buf.clear(); }
        match n {
            Node::Ruby(base, rt) => {
                let b = render_inline(ctx, base, rp);
                out.push_str(&format!("<ruby>{}<rt>{}</rt></ruby>", b, esc_nq(rt)));
            }
            Node::Span(cls, c) => {
                let b = render_inline(ctx, c, rp);
                out.push_str(&format!("<span class=\"{}\">{}</span>", cls, b));
            }
            Node::NoteRef(sys, num) => { let r = render_ref(ctx, sys, num, rp); out.push_str(&r); }
            _ => {}
        }
    }
    if !buf.is_empty() { out.push_str(&autolink(&esc_nq(&buf), &ctx.autolink)); }
    out
}

pub fn inline(ctx: &mut Context, text: &str, rp: RenderPos) -> String {
    let nodes = parse_inline(ctx, text);
    render_inline(ctx, &nodes, rp)
}
