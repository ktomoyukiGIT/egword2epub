//! Python 版のテストで記録した変換（tools/record_fixtures.py）をやり直し、同じ結果になるか確かめる

use base64::Engine;
use egword_core::context::{build_systems, Context, Table, Value};
use egword_core::inline::{inline, RenderPos};
use egword_core::*;
use serde_json::Value as J;
use std::path::{Path, PathBuf};

fn fixtures() -> Vec<J> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fixtures.json");
    serde_json::from_str::<J>(&std::fs::read_to_string(p).unwrap()).unwrap().as_array().unwrap().clone()
}

fn to_toml(v: &J) -> Value {
    match v {
        J::String(s) => Value::String(s.clone()),
        J::Bool(b) => Value::Boolean(*b),
        J::Number(n) => n.as_i64().map(Value::Integer).unwrap_or_else(|| Value::Float(n.as_f64().unwrap())),
        J::Array(a) => Value::Array(a.iter().map(to_toml).collect()),
        J::Object(o) => Value::Table(o.iter().map(|(k, v)| (k.clone(), to_toml(v))).collect()),
        J::Null => Value::String(String::new()),
    }
}

fn to_table(v: &J) -> Table {
    match to_toml(v) { Value::Table(t) => t, _ => Table::new() }
}

fn warnings_json(ws: &[Warning]) -> Vec<Vec<String>> { ws.iter().map(|w| vec![w.file.clone(), w.message.clone()]).collect() }

fn expected_warnings(r: &J) -> Vec<Vec<String>> {
    r["warnings"].as_array().unwrap().iter()
        .map(|w| w.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()).collect()
}

fn make_ctx<'a>(r: &J, images: &'a dyn ImageSource) -> Context<'a> {
    let conf = to_table(&r["conf"]);
    let mut ctx = Context::new(images, build_systems(&conf).unwrap());
    let kw = r["kw"].as_object().unwrap();
    for (k, v) in kw {
        match k.as_str() {
            "periodicals" => ctx.periodicals = v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
            "autolink" => ctx.autolink = v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
            "horizontal" => ctx.horizontal = v.as_bool().unwrap(),
            "imgdir" => {}
            _ => panic!("記録にない引数：{}", k),
        }
    }
    ctx
}

#[test]
fn inline_same_as_python() {
    let mut n = 0;
    let imgs = DirImages(PathBuf::from("."));
    for r in fixtures().iter().filter(|r| r["type"] == "inline") {
        let mut ctx = make_ctx(r, &imgs);
        let out = inline(&mut ctx, r["text"].as_str().unwrap(), RenderPos { pos: r["pos"].as_u64().unwrap() as usize, mode: r["mode"].as_str().unwrap() });
        assert_eq!(out, r["out"].as_str().unwrap(), "行：{}", r["text"]);
        assert_eq!(warnings_json(&ctx.warnings), expected_warnings(r), "警告：{}", r["text"]);
        n += 1;
    }
    assert!(n > 20);
}

#[test]
fn lines_same_as_python() {
    let mut n = 0;
    let imgs = DirImages(PathBuf::from("."));
    for r in fixtures().iter().filter(|r| r["type"] == "lines") {
        let mut ctx = make_ctx(r, &imgs);
        let lines: Vec<String> = r["lines"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let out = egword_core::blocks::lines_html(&mut ctx, &lines, RenderPos { pos: r["pos"].as_u64().unwrap() as usize, mode: r["mode"].as_str().unwrap() });
        assert_eq!(out, r["out"].as_str().unwrap(), "頁：{:?}", lines);
        assert_eq!(warnings_json(&ctx.warnings), expected_warnings(r), "警告：{:?}", lines);
        let hs: Vec<Vec<String>> = ctx.headings.iter()
            .map(|h| vec![h.pos.to_string(), h.level.to_string(), h.title.clone(), h.file.clone(), h.anchor.clone()]).collect();
        let exp: Vec<Vec<String>> = r["headings"].as_array().unwrap().iter()
            .map(|h| h.as_array().unwrap().iter().map(|x| match x { J::String(s) => s.clone(), o => o.to_string() }).collect()).collect();
        assert_eq!(hs, exp, "見出し：{:?}", lines);
        n += 1;
    }
    assert!(n > 5);
}

fn norm_opf(s: &str) -> String {
    let id = fancy_regex::Regex::new("<dc:identifier id=\"bookid\">[^<]*</dc:identifier>").unwrap();
    let md = fancy_regex::Regex::new("<meta property=\"dcterms:modified\">[^<]*</meta>").unwrap();
    md.replace_all(&id.replace_all(s, "<id/>"), "<modified/>").into_owned()
}

/// コマンドと同じ手順で変換する（設定の css は設定ファイルの場所から読む）
fn convert_like_cli(dir: &Path, src: &Path, config: Option<&Path>, css: Option<&Path>) -> Result<Book, String> {
    let data = std::fs::read(src).unwrap();
    let (text, enc, _) = decode_text(&data).map_err(|e| e.0)?;
    let conf = match config {
        Some(p) => parse_config(&std::fs::read_to_string(p).unwrap()).map_err(|e| e.0)?,
        None => Table::new(),
    };
    let css_path = css.map(|p| p.to_path_buf()).or_else(|| {
        let c = conf.get("css").and_then(|v| v.as_str()).filter(|s| !s.is_empty())?;
        Some(config.unwrap().parent().unwrap().join(c))
    });
    let css_text = css_path.map(|p| std::fs::read_to_string(p).unwrap());
    let title = src.file_stem().unwrap().to_string_lossy().into_owned();
    let images = DirImages(dir.to_path_buf());
    let input = ConvertInput { text: &text, encoding: enc, config: &conf, css: css_text.as_deref(), fallback_title: &title,
                               images: &images, modified: "2026-01-01T00:00:00Z" };
    build_book(&input).map_err(|e| e.0)
}

#[test]
fn convert_same_as_python() {
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut n = 0;
    for (i, r) in fixtures().iter().enumerate().filter(|(_, r)| r["type"] == "convert") {
        let dir = std::env::temp_dir().join(format!("egword-parity-{}-{}", std::process::id(), i));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join(r["src_name"].as_str().unwrap());
        std::fs::write(&src, b64.decode(r["src"].as_str().unwrap()).unwrap()).unwrap();
        if let Some(imgs) = r["images"].as_object() {
            for (name, data) in imgs { std::fs::write(dir.join(name), b64.decode(data.as_str().unwrap()).unwrap()).unwrap(); }
        }
        let config = r["config"].as_str().map(|c| { let p = dir.join("book.toml"); std::fs::write(&p, c).unwrap(); p });
        let css = r["css"].as_str().map(|c| { let p = dir.join("given.css"); std::fs::write(&p, c).unwrap(); p });
        let label = format!("変換 {}：{}", i, String::from_utf8_lossy(&b64.decode(r["src"].as_str().unwrap()).unwrap()).chars().take(40).collect::<String>());
        let result = convert_like_cli(&dir, &src, config.as_deref(), css.as_deref());
        if let Some(err) = r["error"].as_str() {
            assert_eq!(result.err().as_deref(), Some(err), "{}", label);
            n += 1;
            continue;
        }
        let book = result.unwrap_or_else(|e| panic!("{}：{}", label, e));
        let files: Vec<Vec<String>> = book.files.iter().map(|f| vec![f.name.clone(), f.xhtml.clone(), f.kind.clone()]).collect();
        let exp: Vec<Vec<String>> = r["files"].as_array().unwrap().iter()
            .map(|f| f.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()).collect();
        assert_eq!(files.iter().map(|f| &f[0]).collect::<Vec<_>>(), exp.iter().map(|f| &f[0]).collect::<Vec<_>>(), "ファイル名：{}", label);
        for (a, b) in files.iter().zip(exp.iter()) { assert_eq!(a, b, "{}：{}", label, a[0]); }
        assert_eq!(book.nav_xhtml, r["nav"].as_str().unwrap(), "nav：{}", label);
        assert_eq!(norm_opf(&book.opf), norm_opf(r["opf"].as_str().unwrap()), "opf：{}", label);
        assert_eq!(book.css, r["css_out"].as_str().unwrap(), "css：{}", label);
        assert_eq!(warnings_json(&book.warnings), expected_warnings(r), "警告：{}", label);
        let imgs: Vec<String> = r["images_used"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
        assert_eq!(book.images, imgs, "画像：{}", label);
        assert_eq!(book.cover.as_deref(), r["cover"].as_str(), "表紙：{}", label);
        let sp: Vec<(String, usize)> = r["src_pages"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap() as usize)).collect();
        let mut mine: Vec<(String, usize)> = book.src_pages.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let mut sp2 = sp.clone();
        mine.sort(); sp2.sort();
        assert_eq!(mine, sp2, "頁：{}", label);
        assert_eq!(book.encoding, r["encoding_out"].as_str().unwrap(), "文字コード：{}", label);
        // 書き出した zip の中の名前
        let names = zip_names(&book.to_epub(&DirImages(dir.clone())));
        let exp_names: Vec<String> = r["zip_names"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
        assert_eq!(names, exp_names, "zip：{}", label);
        let _ = std::fs::remove_dir_all(&dir);
        n += 1;
    }
    assert!(n > 20);
}

/// zip の中央ディレクトリから名前を読む（並べて返す）
fn zip_names(z: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut i = 0;
    while i + 46 <= z.len() {
        if z[i..i + 4] == [0x50, 0x4b, 0x01, 0x02] {
            let n = u16::from_le_bytes([z[i + 28], z[i + 29]]) as usize;
            names.push(String::from_utf8(z[i + 46..i + 46 + n].to_vec()).unwrap());
            i += 46 + n;
        } else { i += 1; }
    }
    names.sort();
    names
}

// ---------------------------------------------------------------- 設定ファイル

fn same(v: &Value, e: &J, path: &str) -> Result<(), String> {
    let t = e["t"].as_str().unwrap();
    match (v, t) {
        (Value::String(s), "string") if J::String(s.clone()) == e["v"] => Ok(()),
        (Value::Integer(i), "int") if e["v"].as_i64() == Some(*i) => Ok(()),
        (Value::Float(f), "double") if e["v"].as_f64() == Some(*f) => Ok(()),
        (Value::Boolean(b), "bool") if e["v"].as_bool() == Some(*b) => Ok(()),
        (Value::Datetime(_), "date") => Ok(()),
        (Value::Array(a), "array") => {
            let ev = e["v"].as_array().unwrap();
            if a.len() != ev.len() { return Err(format!("{}：要素の数", path)); }
            for (i, (x, y)) in a.iter().zip(ev).enumerate() { same(x, y, &format!("{}[{}]", path, i))?; }
            Ok(())
        }
        (Value::Table(tb), "table") => {
            let keys: Vec<&str> = e["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap()).collect();
            let mine: Vec<&str> = tb.keys().map(|k| k.as_str()).collect();
            if mine != keys { return Err(format!("{}：項目 {:?} ≠ {:?}", path, mine, keys)); }
            for k in keys { same(&tb[k], &e["v"][k], &format!("{}.{}", path, k))?; }
            Ok(())
        }
        _ => Err(format!("{}：{:?} ≠ {}", path, v, e)),
    }
}

#[test]
fn toml_same_as_tomllib() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/toml.json");
    let j: J = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    for gd in j["good"].as_array().unwrap() {
        let t = parse_config(gd["text"].as_str().unwrap()).unwrap();
        same(&Value::Table(t), &gd["expected"], "").unwrap();
    }
    for b in j["bad"].as_array().unwrap() { assert!(parse_config(b.as_str().unwrap()).is_err(), "{}", b); }
}

// ---------------------------------------------------------------- エディタの中身・文字コード

#[test]
fn editops() {
    use egword_core::editops::*;
    assert_eq!(ruby("漢字", "かんじ", ""), "漢字《かんじ》");
    assert_eq!(ruby("漢字", "かんじ", "大"), "｜漢字《かんじ》");
    assert_eq!(ruby("にんにく", "ガーリック", ""), "｜にんにく《ガーリック》");
    assert_eq!(ruby("𠮟", "しつ", ""), "𠮟《しつ》");
    assert_eq!(decorate("傍点", "強調"), ("強調［＃「強調」に傍点］".into(), 12));
    assert_eq!(decorate("太字", ""), ("［＃太字］［＃太字終わり］".into(), 5));
    assert_eq!(decorate("１段階小さな文字", "x").0, "x［＃「x」は１段階小さな文字］");
    assert_eq!(block("［＃A］", "［＃B］", "a\nb").0, "［＃A］\na\nb\n［＃B］\n");
    assert_eq!(indent_block(12, Some(3)).0, "［＃ここから１２字下げ、折り返して３字下げ］");
    assert_eq!(gaiji("木＋世", "u+6318"), "※［＃「木＋世」、U+6318］");
    let text = "p1\n\u{0C}\np2 a\np2 b\n［＃改ページ］\np3 「探す」語\n";
    let starts = page_start_lines(text);
    assert_eq!(starts, vec![1, 2, 5]);
    let src: indexmap::IndexMap<String, usize> = [("p001.xhtml", 0), ("p002.xhtml", 1), ("p003.xhtml", 2)].iter().map(|(a, b)| (a.to_string(), *b)).collect();
    assert_eq!(locate_warning(text, &src, "p003.xhtml", "後置注記の対象「探す」が見つからない"), Some((6, 4, 6)));
    assert_eq!(locate_warning(text, &src, "images", "画像"), None);
    assert_eq!(file_for_page(&src, 2).as_deref(), Some("p003.xhtml"));
}

#[test]
fn shift_jis_round_trip() {
    let bytes = [0x8a, 0xbf, 0x8e, 0x9a, 0x81, 0x73, 0x82, 0xa9, 0x82, 0xf1, 0x82, 0xb6, 0x81, 0x74, 0x81, 0x60, 0x87, 0x40];
    let (s, enc, _) = decode_text(&bytes).unwrap();
    assert_eq!(enc, TextEncoding::Cp932);
    assert_eq!(s, "漢字《かんじ》～①");
    assert_eq!(encode_text(&s, enc, false).unwrap(), bytes.to_vec());
    assert_eq!(encode_text("𠮟", TextEncoding::Cp932, false), Err("𠮟".into()));
}
