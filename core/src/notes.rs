//! 注の本文の並び（notes.py）

use crate::blocks::*;
use crate::context::{Context, NotesBlock};
use crate::inline::*;
use crate::py::*;
use std::collections::{HashMap, HashSet};

fn heading_lines(ctx: &Context) -> HashSet<String> {
    ctx.systems.iter().flat_map(|s| s.heading_lines.iter().cloned()).collect()
}

/// 一頁のテキストを (本文, 注の並び) に分ける
pub fn split_notes(ctx: &Context, text: &str) -> (String, Option<String>) {
    let heads = heading_lines(ctx);
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, l) in lines.iter().enumerate() {
        if heads.contains(py_strip(l)) || ctx.systems.iter().any(|s| s.start_re.match_(l).is_some()) {
            return (lines[..i].join("\n"), Some(lines[i..].join("\n")));
        }
    }
    (text.to_string(), None)
}

pub struct NoteEntry { pub n: String, pub lines: Vec<(String, Option<BlockSpec>)> }

/// 注の並びを、系統ごとの [注] にする
pub fn parse_notes(ctx: &mut Context, text: &str) -> HashMap<String, Vec<NoteEntry>> {
    let heads = heading_lines(ctx);
    let mut entries: HashMap<String, Vec<NoteEntry>> = ctx.systems.iter().map(|s| (s.key.clone(), vec![])).collect();
    let mut cur: Option<(String, usize)> = None;
    let mut block: Option<BlockSpec> = None;
    let mut stack: Vec<Option<BlockSpec>> = Vec::new();
    for l in strip_chars(text, "\n").split('\n') {
        let st = py_strip(l);
        if heads.contains(st) { continue; }
        if let Some(m) = BLOCK_START_RE.fullmatch(st) {
            let spec = parse_block_spec(g(&m, 1).unwrap(), Some(ctx));
            stack.push(block.take());
            block = Some(spec);
            continue;
        }
        if BLOCK_END_RE.is_full(st) { block = stack.pop().flatten(); continue; }
        let mut hit: Option<(String, String, String)> = None;
        if block.is_none() {
            for s in &ctx.systems {
                if let Some(mm) = s.entry_re.match_(l) {
                    hit = Some((s.key.clone(), num(g(&mm, 1).unwrap()).to_string(), g(&mm, 2).unwrap_or("").to_string()));
                    break;
                }
            }
        }
        if let Some((key, n, first)) = hit {
            let list = entries.get_mut(&key).unwrap();
            list.push(NoteEntry { n, lines: vec![(first, None)] });
            cur = Some((key, list.len() - 1));
        } else if let Some((key, i)) = &cur {
            entries.get_mut(key).unwrap()[*i].lines.push((l.to_string(), block.clone()));
        } else if !st.is_empty() {
            ctx.warn(format!("注の並びの初めに、どの注にも属さない行がある：{}", prefix_chars(st, 20)));
        }
    }
    entries
}

/// 注の並びを ctx.notes_blocks に登録する（番号だけ先に読む）
pub fn register(ctx: &mut Context, pos: usize, file: &str, text: &str) -> usize {
    let entries = parse_notes(ctx, text);
    let mut b = NotesBlock { pos, file: file.into(), text: text.into(), numbers: HashMap::new(),
                             scopes: HashMap::new(), cls: HashMap::new() };
    for s in &ctx.systems {
        b.numbers.insert(s.key.clone(), entries[&s.key].iter().map(|e| e.n.clone()).collect());
        b.cls.insert(s.key.clone(), s.cls.clone());
    }
    ctx.notes_blocks.push(b);
    ctx.notes_blocks.len() - 1
}

/// id の区別：番号が本の中で重なる系統だけ、注の並びの順番で区別する
pub fn assign_scopes(ctx: &mut Context) {
    let keys: Vec<String> = ctx.systems.iter().map(|s| s.key.clone()).collect();
    for key in keys {
        let idx: Vec<usize> = (0..ctx.notes_blocks.len())
            .filter(|&i| ctx.notes_blocks[i].numbers.get(&key).map(|v| !v.is_empty()).unwrap_or(false)).collect();
        let nums: Vec<&String> = idx.iter().flat_map(|&i| ctx.notes_blocks[i].numbers[&key].iter()).collect();
        let uniq: HashSet<&&String> = nums.iter().collect();
        if nums.len() == uniq.len() { continue; }
        for (k, &i) in idx.iter().enumerate() { ctx.notes_blocks[i].scopes.insert(key.clone(), format!("{}-", k + 1)); }
        for &i in &idx {
            let ns = ctx.notes_blocks[i].numbers[&key].clone();
            let mut dup: Vec<String> = ns.iter().filter(|n| ns.iter().filter(|x| x == n).count() > 1).cloned().collect();
            dup.sort();
            dup.dedup();
            if !dup.is_empty() {
                let f = ctx.notes_blocks[i].file.clone();
                ctx.warn(format!("{}：同じ番号の注が一つの並びに重なっている：{}", f, dup.join("、")));
            }
        }
    }
}

/// 注ごとに、本文で最初に参照したファイルを記録する
pub fn collect_backrefs(ctx: &mut Context, plan: &[(&str, &str, &str)]) {
    for (pos, (kind, text, file)) in plan.iter().enumerate() {
        if *kind == "notes" || *kind == "opener" { continue; }
        let mut found = Vec::new();
        for s in &ctx.systems {
            for m in s.ref_re.finditer(text) {
                found.push((s.key.clone(), num(gn(&m, &format!("{}_n", s.key)).unwrap()).to_string()));
            }
        }
        for (key, n) in found {
            if let Some(b) = ctx.resolve(&key, &n, pos) {
                ctx.backref.entry((key, b, n)).or_insert_with(|| file.to_string());
            }
        }
    }
}

fn cont(ctx: &mut Context, b: &str, blk: Option<&BlockSpec>, rp: RenderPos) -> String {
    if let Some(blk) = blk {
        let p = parse_para(ctx, b, Some(blk), "verse");
        return render_block(ctx, &p, rp);
    }
    if py_lstrip(b).starts_with("［＃") || py_rstrip(b).ends_with("［＃地付き］") {
        let p = parse_para(ctx, b, None, "nc");
        return render_block(ctx, &p, rp);
    }
    let nodes = parse_inline(ctx, b);
    format!("<p class=\"nc\">{}</p>", render_inline(ctx, &nodes, rp))
}

pub fn notes_html(ctx: &mut Context, block: usize, pos: usize) -> String {
    let rp = RenderPos { pos, mode: "note" };
    let text = ctx.notes_blocks[block].text.clone();
    let entries = parse_notes(ctx, &text);
    let mut out: Vec<String> = Vec::new();
    let systems: Vec<(String, String, String, String, String, String, bool)> = ctx.systems.iter()
        .map(|s| (s.key.clone(), s.section_cls.clone(), s.label.clone(), s.cls.clone(), s.prefix.clone(), s.suffix.clone(), s.popup))
        .collect();
    for (key, section_cls, label, cls, prefix, suffix, popup) in systems {
        let list = &entries[&key];
        if list.is_empty() { continue; }
        out.push(format!("<section class=\"notes {}\"><h2>{}</h2>", section_cls, esc(&label)));
        for e in list {
            let n = &e.n;
            let mut ps: Vec<String> = Vec::new();
            let anchor = ctx.notes_blocks[block].anchor(&key, n);
            let disp = ctx.system(&key).display(n);
            let back = match ctx.backref.get(&(key.clone(), block, n.clone())).cloned() {
                Some(r) => format!("<a class=\"back\" href=\"{}#{}ref{}{}\">{}</a>", r, cls, ctx.notes_blocks[block].scope(&key), n, disp),
                None => {
                    ctx.warn(format!("注 {}{}{} を参照している本文が見つからない", prefix, n, suffix));
                    format!("<span class=\"back\">{}</span>", disp)
                }
            };
            for (k, (b, blk)) in e.lines.iter().enumerate() {
                if k == 0 {
                    let nodes = parse_inline(ctx, b);
                    ps.push(format!("<p class=\"nh\">{}　{}</p>", back, render_inline(ctx, &nodes, rp)));
                } else if py_strip(b).is_empty() {
                    if blk.is_some() { ps.push(BLANK.into()); }
                } else {
                    ps.push(cont(ctx, b, blk.as_ref(), rp));
                }
            }
            if popup {
                out.push(format!("<aside epub:type=\"footnote\" id=\"{}\" class=\"note\">{}</aside>", anchor, ps.join("")));
            } else {
                out.push(format!("<div class=\"note\" id=\"{}\">{}</div>", anchor, ps.join("")));
            }
        }
        out.push("</section>".into());
    }
    out.join("\n")
}
