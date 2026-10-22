//! 奥付（colophon.py）と、OPF・nav・表紙・画像をまとめた EPUB（package.py）

use crate::basics::*;
use crate::context::*;
use crate::py::*;
use crate::structure::{nav_html, NavItem, OutputFile};
use std::sync::LazyLock;

// ---------------------------------------------------------------- 奥付

fn role_label(r: &str) -> &'static str { match r { "aut" => "著", "trl" => "訳", "edt" => "編", "ill" => "絵", _ => "" } }
static JDATE_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("(\\d{4})(?:-(\\d{2})(?:-(\\d{2}))?)?"));

/// 2026-10-19 → 2026年10月19日（ほかの形はそのまま）
pub fn japanese_date(s: &str) -> String {
    let Some(m) = JDATE_RE.fullmatch(s) else { return s.to_string() };
    let mut out = format!("{}年", g(&m, 1).unwrap());
    if let Some(mo) = g(&m, 2) { out.push_str(&format!("{}月", mo.parse::<i64>().unwrap())); }
    if let Some(d) = g(&m, 3) { out.push_str(&format!("{}日", d.parse::<i64>().unwrap())); }
    out
}

pub fn creators(book: &Table) -> Vec<&Table> {
    match book.get("creators") {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_table()).collect(),
        _ => vec![],
    }
}

fn title_of(book: &Table, fallback: &str) -> String {
    book.get("title").filter(|v| truthy(v)).map(vstr).unwrap_or_else(|| fallback.to_string())
}

pub fn colophon_html(book: &Table, conf: &Table, fallback_title: &str) -> String {
    let mut rows = vec![format!("<p class=\"col-title\">{}</p>", esc_nq(&title_of(book, fallback_title)))];
    for c in creators(book) {
        let label = role_label(&get_str(c, "role").unwrap_or_else(|| "aut".into()));
        rows.push(format!("<p>{}{}</p>", esc_nq(&get_str(c, "name").unwrap_or_default()),
                          if label.is_empty() { String::new() } else { format!("　{}", label) }));
    }
    if let Some(d) = book.get("date").filter(|v| truthy(v)) {
        rows.push(format!("<p>{}　{}</p>", esc_nq(&get_str(conf, "date_label").unwrap_or_else(|| "発行".into())),
                          esc_nq(&japanese_date(&vstr(d)))));
    }
    if let Some(p) = book.get("publisher").filter(|v| truthy(v)) { rows.push(format!("<p>{}</p>", esc_nq(&vstr(p)))); }
    if let Some(Value::Array(lines)) = conf.get("lines") {
        for l in lines {
            rows.push(if truthy(l) { format!("<p>{}</p>", esc_nq(&vstr(l))) } else { "<p class=\"blank\">　</p>".into() });
        }
    }
    format!("<div class=\"colophon\">\n{}\n</div>", rows.join("\n"))
}

// ---------------------------------------------------------------- EPUB

pub const HORIZONTAL_CSS: &str = "
/* 横書き（writing_mode = \"horizontal\"） */
html { writing-mode: horizontal-tb; -webkit-writing-mode: horizontal-tb; -epub-writing-mode: horizontal-tb; }
.notes { margin-right: 0; margin-top: 3em; }
.notes h2 { margin-left: 0; margin-bottom: 1em; }
.note p.nh { text-indent: -2em; margin-top: 0; margin-left: 2em; }
.note p.nc { margin-top: 0; margin-left: 2em; }
.note { margin-left: 0; margin-bottom: 0.6em; }
h2.o-midashi, h3.naka-midashi, h4.ko-midashi { margin-left: 0; margin-top: 1em; margin-bottom: 1em; }
.tocpage p.toc2 { margin-top: 0; margin-left: 2em; }
.tocpage p.toc3 { margin-top: 0; margin-left: 4em; }
.yoko { margin: 1em 0; }
";

pub fn media_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.').map(|(a, e)| if a.is_empty() { "" } else { e }).unwrap_or("").to_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg", "png" => "image/png", "gif" => "image/gif",
        "webp" => "image/webp", "svg" => "image/svg+xml", _ => return None,
    })
}

static ISO_DATE_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("\\d{4}(-\\d{2}(-\\d{2})?)?"));
static IMG_ID_RE: LazyLock<Rx> = LazyLock::new(|| Rx::new("[^A-Za-z0-9]"));

fn img_id(name: &str) -> String { format!("img_{}", IMG_ID_RE.sub(name, "_")) }

fn metadata_xml(book: &Table, fallback_title: &str, ctx: &mut Context) -> String {
    let mut out = vec![format!("<dc:title id=\"title\">{}</dc:title>", esc(&title_of(book, fallback_title)))];
    if let Some(fa) = book.get("title_file_as").filter(|v| truthy(v)) {
        out.push(format!("<meta refines=\"#title\" property=\"file-as\">{}</meta>", esc(&vstr(fa))));
    }
    let mut used: Vec<(String, usize)> = Vec::new();
    for c in creators(book) {
        let role = get_str(c, "role").unwrap_or_else(|| "aut".into());
        let base = match role.as_str() { "aut" => "author", "trl" => "translator", "edt" => "editor", "ill" => "illustrator", _ => "creator" };
        let n = match used.iter_mut().find(|(b, _)| b == base) { Some(u) => { u.1 += 1; u.1 } None => { used.push((base.into(), 1)); 1 } };
        let cid = if n == 1 { base.to_string() } else { format!("{}{}", base, n) };
        out.push(format!("<dc:creator id=\"{}\">{}</dc:creator>", cid, esc(&get_str(c, "name").unwrap_or_default())));
        out.push(format!("<meta refines=\"#{}\" property=\"role\" scheme=\"marc:relators\">{}</meta>", cid, esc(&role)));
        if let Some(fa) = c.get("file_as").filter(|v| truthy(v)) {
            out.push(format!("<meta refines=\"#{}\" property=\"file-as\">{}</meta>", cid, esc(&vstr(fa))));
        }
    }
    out.push(format!("<dc:language>{}</dc:language>", esc(&get_str(book, "language").unwrap_or_else(|| "ja".into()))));
    for key in ["publisher", "date", "description"] {
        let Some(v) = book.get(key).filter(|v| truthy(v)) else { continue };
        if key == "date" && !ISO_DATE_RE.is_full(&vstr(v)) {
            ctx.warn(format!("[book] の date「{}」は 2026-10-19 の形で書いてください（書誌情報には入れなかった）", vstr(v)));
            continue;
        }
        out.push(format!("<dc:{k}>{}</dc:{k}>", esc(&vstr(v)), k = key));
    }
    out.join("\n")
}

fn cover_page(ctx: &Context) -> String {
    let cover = ctx.cover.clone().unwrap();
    let (cw, ch) = ctx.images_src.bytes(&cover).and_then(|b| image_size(&b)).unwrap_or((1600, 2560));
    let svg = format!("<section epub:type=\"cover\"><svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
version=\"1.1\" width=\"100%\" height=\"100%\" viewBox=\"0 0 {cw} {ch}\" preserveAspectRatio=\"xMidYMid meet\">\
<image width=\"{cw}\" height=\"{ch}\" xlink:href=\"images/{}\"/></svg></section>", cover, cw = cw, ch = ch);
    xhtml("表紙", &svg, "cover")
}

pub fn nav_xhtml(nav: &[NavItem]) -> String {
    xhtml("目次", &format!("<nav epub:type=\"toc\" id=\"toc\"><h1>目次</h1><ol>{}</ol></nav>", nav_html(nav)), "")
}

fn opf_xml(ctx: &Context, files: &[OutputFile], metadata: &str, identifier: &str, modified: &str) -> String {
    let mut manifest = Vec::new();
    let mut spine = Vec::new();
    for f in files {
        let props = if f.kind == "cover" { " properties=\"svg\"" } else { "" };
        let id = f.name.replace('.', "_");
        manifest.push(format!("<item id=\"{}\" href=\"{}\" media-type=\"application/xhtml+xml\"{}/>", id, f.name, props));
        spine.push(format!("<itemref idref=\"{}\"/>", id));
    }
    for im in &ctx.images {
        let mt = media_type(im).unwrap_or("application/octet-stream");
        let prop = if ctx.cover.as_deref() == Some(im.as_str()) { " properties=\"cover-image\"" } else { "" };
        manifest.push(format!("<item id=\"{}\" href=\"images/{}\" media-type=\"{}\"{}/>", img_id(im), im, mt, prop));
    }
    let cover_meta = ctx.cover.as_ref().map(|c| format!("<meta name=\"cover\" content=\"{}\"/>", img_id(c))).unwrap_or_default();
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" xml:lang=\"ja\">
<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">
<dc:identifier id=\"bookid\">{identifier}</dc:identifier>
{metadata}
<meta property=\"dcterms:modified\">{modified}</meta>
<meta property=\"rendition:spread\">none</meta>
<meta name=\"primary-writing-mode\" content=\"{mode}\"/>
{cover_meta}
</metadata>
<manifest>
<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>
<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>
{manifest}
</manifest>
<spine page-progression-direction=\"{dir}\">
{spine}
</spine>
</package>
",
        identifier = identifier, metadata = metadata, modified = modified,
        mode = if ctx.horizontal { "horizontal-lr" } else { "vertical-rl" }, cover_meta = cover_meta,
        manifest = manifest.join("\n"), dir = if ctx.horizontal { "ltr" } else { "rtl" }, spine = spine.join("\n"))
}

/// EPUB で使えない形式の画像と、小さすぎる・横長の表紙を警告する
fn check_images(ctx: &mut Context) {
    ctx.where_ = "images".into();
    for im in ctx.images.clone() {
        if media_type(&im).is_none() {
            ctx.warn(format!("EPUB で使えない形式の画像：{}（JPEG・PNG・GIF・WebP・SVG にしてください）", im));
        }
    }
    let Some(cover) = ctx.cover.clone() else { return };
    if media_type(&cover) == Some("image/svg+xml") { return; }
    let Some((w, h)) = ctx.images_src.bytes(&cover).and_then(|b| image_size(&b)) else { return };
    if w.max(h) < 1000 { ctx.warn(format!("表紙の画像が小さい（{}×{}）。長い辺 1600px 以上を勧めます", w, h)); }
    if w > h { ctx.warn(format!("表紙の画像が横長（{}×{}）", w, h)); }
}

/// EPUB の中身（書き出す前）
pub struct Book {
    pub files: Vec<OutputFile>,
    pub nav: Vec<NavItem>,
    pub opf: String,
    pub nav_xhtml: String,
    pub css: String,
    pub warnings: Vec<Warning>,
    pub images: Vec<String>,
    pub cover: Option<String>,
    pub src_pages: indexmap::IndexMap<String, usize>,
    pub encoding: String,
}

pub fn assemble(ctx: &mut Context, mut files: Vec<OutputFile>, nav: Vec<NavItem>, book: &Table, css: &str,
                fallback_title: &str, modified: &str) -> Book {
    check_images(ctx);
    if ctx.cover.is_some() {
        files.insert(0, OutputFile { name: "cover.xhtml".into(), xhtml: cover_page(ctx), kind: "cover".into() });
    }
    let identifier = get_str(book, "identifier").filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("urn:uuid:{}", uuid::Uuid::new_v4()));
    ctx.where_ = "content.opf".into();
    let meta = metadata_xml(book, fallback_title, ctx);
    let opf = opf_xml(ctx, &files, &meta, &identifier, modified);
    Book {
        nav_xhtml: nav_xhtml(&nav), files, nav, opf,
        css: format!("{}{}", css, if ctx.horizontal { HORIZONTAL_CSS } else { "" }),
        warnings: ctx.warnings.clone(), images: ctx.images.clone(), cover: ctx.cover.clone(),
        src_pages: ctx.src_pages.clone(), encoding: ctx.encoding.clone(),
    }
}

impl Book {
    /// EPUB（zip）のバイト列。images には（縮小などを済ませた）画像の中身を渡す
    pub fn to_epub(&self, images: &dyn ImageSource) -> Vec<u8> {
        let mut z = ZipWriter::default();
        z.add("mimetype", b"application/epub+zip", false);
        z.add("META-INF/container.xml", CONTAINER.as_bytes(), true);
        z.add("OEBPS/content.opf", self.opf.as_bytes(), true);
        z.add("OEBPS/nav.xhtml", self.nav_xhtml.as_bytes(), true);
        z.add("OEBPS/style.css", self.css.as_bytes(), true);
        for f in &self.files { z.add(&format!("OEBPS/{}", f.name), f.xhtml.as_bytes(), true); }
        for im in &self.images {
            let data = images.bytes(im).unwrap_or_default();
            z.add(&format!("OEBPS/images/{}", im), &data, false);
        }
        z.finish()
    }
}

// ---------------------------------------------------------------- zip

fn crc32(data: &[u8]) -> u32 {
    static TABLE: LazyLock<[u32; 256]> = LazyLock::new(|| {
        let mut t = [0u32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 { c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 }; }
            *v = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for &b in data { c = TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8); }
    c ^ 0xFFFF_FFFF
}

#[derive(Default)]
struct ZipWriter { body: Vec<u8>, central: Vec<u8>, count: u16 }

impl ZipWriter {
    fn add(&mut self, name: &str, data: &[u8], compress: bool) {
        let crc = crc32(data);
        let packed = if compress && !data.is_empty() {
            let d = miniz_oxide::deflate::compress_to_vec(data, 6);
            if d.len() < data.len() { Some(d) } else { None }
        } else { None };
        let method: u16 = if packed.is_some() { 8 } else { 0 };
        let stored: &[u8] = packed.as_deref().unwrap_or(data);
        let offset = self.body.len() as u32;
        let (time, date): (u16, u16) = (0, (0 << 9) | (1 << 5) | 1); // 1980-01-01
        let n = name.as_bytes();
        let mut h = Vec::new();
        h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        h.extend_from_slice(&20u16.to_le_bytes());
        h.extend_from_slice(&0x0800u16.to_le_bytes()); // 名前は UTF-8
        h.extend_from_slice(&method.to_le_bytes());
        h.extend_from_slice(&time.to_le_bytes());
        h.extend_from_slice(&date.to_le_bytes());
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(n.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        self.body.extend_from_slice(&h);
        self.body.extend_from_slice(n);
        self.body.extend_from_slice(stored);

        let c = &mut self.central;
        c.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&0x0800u16.to_le_bytes());
        c.extend_from_slice(&method.to_le_bytes());
        c.extend_from_slice(&time.to_le_bytes());
        c.extend_from_slice(&date.to_le_bytes());
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        c.extend_from_slice(&(data.len() as u32).to_le_bytes());
        c.extend_from_slice(&(n.len() as u16).to_le_bytes());
        c.extend_from_slice(&[0u8; 12]); // 拡張・注釈の長さ、ディスク番号、属性
        c.extend_from_slice(&offset.to_le_bytes());
        c.extend_from_slice(n);
        self.count += 1;
    }

    fn finish(mut self) -> Vec<u8> {
        let cd_offset = self.body.len() as u32;
        let cd_len = self.central.len() as u32;
        self.body.append(&mut self.central);
        let e = &mut self.body;
        e.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        e.extend_from_slice(&[0u8; 4]);
        e.extend_from_slice(&self.count.to_le_bytes());
        e.extend_from_slice(&self.count.to_le_bytes());
        e.extend_from_slice(&cd_len.to_le_bytes());
        e.extend_from_slice(&cd_offset.to_le_bytes());
        e.extend_from_slice(&0u16.to_le_bytes());
        self.body
    }
}

// ---------------------------------------------------------------- 画像の大きさ

/// 画像の幅と高さ（JPEG・PNG・GIF・WebP の頭を読む）
pub fn image_size(d: &[u8]) -> Option<(u32, u32)> {
    let be16 = |i: usize| -> u32 { (d[i] as u32) << 8 | d[i + 1] as u32 };
    let le16 = |i: usize| -> u32 { d[i] as u32 | (d[i + 1] as u32) << 8 };
    if d.len() >= 24 && d[..4] == [0x89, b'P', b'N', b'G'] {
        return Some((be16(16) << 16 | be16(18), be16(20) << 16 | be16(22)));
    }
    if d.len() >= 10 && &d[..3] == b"GIF" { return Some((le16(6), le16(8))); }
    if d.len() >= 30 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        return match &d[12..16] {
            b"VP8 " => Some((le16(26) & 0x3FFF, le16(28) & 0x3FFF)),
            b"VP8L" => {
                let b: Vec<u32> = d[21..25].iter().map(|x| *x as u32).collect();
                Some((1 + (b[0] | (b[1] & 0x3F) << 8), 1 + ((b[1] >> 6) | b[2] << 2 | (b[3] & 0x0F) << 10)))
            }
            b"VP8X" => Some((1 + (d[24] as u32 | (d[25] as u32) << 8 | (d[26] as u32) << 16),
                             1 + (d[27] as u32 | (d[28] as u32) << 8 | (d[29] as u32) << 16))),
            _ => None,
        };
    }
    if d.len() >= 4 && d[0] == 0xFF && d[1] == 0xD8 {
        let mut i = 2;
        while i + 9 < d.len() {
            if d[i] != 0xFF { i += 1; continue; }
            let marker = d[i + 1];
            if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) { i += 2; continue; }
            if marker == 0xFF { i += 1; continue; }
            let len = be16(i + 2) as usize;
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                return Some((be16(i + 7), be16(i + 5)));
            }
            i += 2 + len;
        }
    }
    None
}
