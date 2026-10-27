//! 変換の本体を、ブラウザ（JavaScript）から使えるようにする

use egword_core::{editops, Book, ImageSource};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

fn js_err(s: impl Into<String>) -> JsValue { JsValue::from_str(&s.into()) }
fn to_js<T: Serialize>(v: &T) -> JsValue {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible()).unwrap()
}

#[derive(Serialize)]
struct Decoded { text: String, encoding: String, label: String, bom: bool }

/// テキストのバイト列を読む（UTF-8・UTF-16・Shift_JIS を見分ける）
#[wasm_bindgen]
pub fn decode_text(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let (text, enc, bom) = egword_core::decode_text(bytes).map_err(|e| js_err(e.0))?;
    Ok(to_js(&Decoded { text, encoding: enc.name().into(), label: enc.label().into(), bom }))
}

/// テキストをバイト列に。書けない字があれば、その字を投げる
#[wasm_bindgen]
pub fn encode_text(text: &str, encoding: &str, bom: bool) -> Result<Vec<u8>, JsValue> {
    egword_core::encode_text(text, egword_core::TextEncoding::from_name(encoding), bom).map_err(js_err)
}

/// 設定ファイルの css の値（なければ undefined）。書き方の誤りは投げる
#[wasm_bindgen]
pub fn config_css(config: &str) -> Result<Option<String>, JsValue> {
    let t = egword_core::parse_config(config).map_err(|e| js_err(e.0))?;
    Ok(t.get("css").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()))
}

struct Images {
    data: HashMap<String, Vec<u8>>,
    export: HashMap<String, Vec<u8>>,   // 書き出し用（縮小したもの）。なければ data を使う
    for_export: bool,
}

impl ImageSource for Images {
    fn exists(&self, name: &str) -> bool { self.data.contains_key(name) }
    fn bytes(&self, name: &str) -> Option<Vec<u8>> {
        if self.for_export { if let Some(b) = self.export.get(name) { return Some(b.clone()); } }
        self.data.get(name).cloned()
    }
}

#[derive(Serialize)]
struct FileOut<'a> { name: &'a str, kind: &'a str, xhtml: &'a str }
#[derive(Serialize)]
struct WarnOut<'a> { file: &'a str, message: &'a str }

/// 変換するもの。画像を登録しておき、テキストが変わるたびに build する
#[wasm_bindgen]
pub struct Converter {
    images: RefCell<Images>,
    book: Option<Book>,
}

#[wasm_bindgen]
impl Converter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Converter {
        Converter { images: RefCell::new(Images { data: HashMap::new(), export: HashMap::new(), for_export: false }), book: None }
    }

    pub fn add_image(&self, name: &str, bytes: Vec<u8>) {
        let mut im = self.images.borrow_mut();
        im.export.remove(name);
        im.data.insert(name.to_string(), bytes);
    }
    /// 書き出しのときに使う画像（縮小したもの）
    pub fn set_export_image(&self, name: &str, bytes: Vec<u8>) { self.images.borrow_mut().export.insert(name.to_string(), bytes); }
    pub fn clear_images(&self) { let mut im = self.images.borrow_mut(); im.data.clear(); im.export.clear(); }
    pub fn image_names(&self) -> Vec<String> { let mut v: Vec<String> = self.images.borrow().data.keys().cloned().collect(); v.sort(); v }
    pub fn has_image(&self, name: &str) -> bool { self.images.borrow().data.contains_key(name) }

    /// 変換する。設定の誤りなどは投げる
    pub fn build(&mut self, text: &str, encoding: &str, config: Option<String>, css: Option<String>,
                 title: &str, modified: &str) -> Result<(), JsValue> {
        let conf = match config.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(c) => egword_core::parse_config(c).map_err(|e| js_err(e.0))?,
            None => egword_core::Table::new(),
        };
        let images = self.images.borrow();
        let input = egword_core::ConvertInput {
            text, encoding: egword_core::TextEncoding::from_name(encoding), config: &conf, css: css.as_deref(),
            fallback_title: title, images: &*images, modified,
        };
        let book = egword_core::build_book(&input).map_err(|e| js_err(e.0))?;
        drop(images);
        self.book = Some(book);
        Ok(())
    }

    fn book(&self) -> Result<&Book, JsValue> { self.book.as_ref().ok_or_else(|| js_err("まだ変換していません")) }

    pub fn files(&self) -> Result<JsValue, JsValue> {
        let b = self.book()?;
        Ok(to_js(&b.files.iter().map(|f| FileOut { name: &f.name, kind: &f.kind, xhtml: &f.xhtml }).collect::<Vec<_>>()))
    }
    pub fn warnings(&self) -> Result<JsValue, JsValue> {
        let b = self.book()?;
        Ok(to_js(&b.warnings.iter().map(|w| WarnOut { file: &w.file, message: &w.message }).collect::<Vec<_>>()))
    }
    pub fn css(&self) -> Result<String, JsValue> { Ok(self.book()?.css.clone()) }
    pub fn nav_xhtml(&self) -> Result<String, JsValue> { Ok(self.book()?.nav_xhtml.clone()) }
    pub fn used_images(&self) -> Result<Vec<String>, JsValue> { Ok(self.book()?.images.clone()) }
    pub fn cover(&self) -> Result<Option<String>, JsValue> { Ok(self.book()?.cover.clone()) }

    /// 出力ファイル → 元のテキストの何頁目（0 から）
    pub fn src_pages(&self) -> Result<JsValue, JsValue> {
        let b = self.book()?;
        let m: std::collections::BTreeMap<&str, usize> = b.src_pages.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        Ok(to_js(&m))
    }

    /// 警告の場所（行, 列の始め, 列の終わり。列は UTF-16）。分からなければ undefined
    pub fn locate(&self, text: &str, file: &str, message: &str) -> Result<Option<Vec<u32>>, JsValue> {
        let b = self.book()?;
        Ok(editops::locate_warning(text, &b.src_pages, file, message).map(|(l, a, e)| vec![l as u32, a as u32, e as u32]))
    }

    /// その頁から作った出力ファイル
    pub fn file_for_page(&self, page: usize) -> Result<Option<String>, JsValue> {
        Ok(editops::file_for_page(&self.book()?.src_pages, page))
    }

    /// EPUB のバイト列
    pub fn epub(&self) -> Result<Vec<u8>, JsValue> {
        let b = self.book()?;
        self.images.borrow_mut().for_export = true;
        let out = b.to_epub(&*self.images.borrow());
        self.images.borrow_mut().for_export = false;
        Ok(out)
    }
}

impl Default for Converter { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------- エディタの中身

#[wasm_bindgen] pub fn ruby(base: &str, reading: &str, prev: &str) -> String { editops::ruby(base, reading, prev) }

/// 飾りを付けた文字列とカーソルの位置（[文字列, 位置]）
#[wasm_bindgen]
pub fn decorate(name: &str, selection: &str) -> JsValue { to_js(&editops::decorate(name, selection)) }
#[wasm_bindgen]
pub fn block(open: &str, close: &str, selection: &str) -> JsValue { to_js(&editops::block(open, close, selection)) }
#[wasm_bindgen]
pub fn indent_block(first: i32, rest: Option<i32>) -> JsValue { to_js(&editops::indent_block(first as i64, rest.map(|r| r as i64))) }
#[wasm_bindgen] pub fn gaiji(description: &str, code: &str) -> String { editops::gaiji(description, code) }
#[wasm_bindgen] pub fn figure(img: &str, caption: &str) -> String { editops::figure(img, caption) }
#[wasm_bindgen] pub fn opener(img: &str, title: &str) -> String { editops::opener(img, title) }
#[wasm_bindgen] pub fn separator(style: &str) -> String { editops::separator(style) }
#[wasm_bindgen] pub fn fullwidth(n: i32) -> String { editops::fullwidth(n as i64) }
#[wasm_bindgen]
pub fn page_start_lines(text: &str) -> Vec<u32> { editops::page_start_lines(text).into_iter().map(|x| x as u32).collect() }
#[wasm_bindgen] pub fn version() -> String { env!("CARGO_PKG_VERSION").into() }

/// 設定ファイルを確かめる。問題があればその説明、なければ undefined
#[wasm_bindgen]
pub fn check_config(text: &str) -> Option<String> {
    let conf = match egword_core::parse_config(text) { Ok(c) => c, Err(e) => return Some(e.0) };
    let input = egword_core::ConvertInput {
        text: "", encoding: egword_core::TextEncoding::Utf8, config: &conf, css: Some(""), fallback_title: "t",
        images: &egword_core::NoImages, modified: "2026-01-01T00:00:00Z",
    };
    match egword_core::build_book(&input) {
        Err(e) => Some(e.0),
        Ok(b) => b.warnings.iter().find(|w| w.file == "content.opf").map(|w| w.message.clone()),
    }
}

// ---------------------------------------------------------------- 設定の画面（中身は core の conftoml）
// 設定そのものは JSON の文字列で画面に持たせる（日付の形を失わないため）

use egword_core::conftoml;

fn data_of(json: &str) -> Result<egword_core::Table, JsValue> { conftoml::table_from_json(json).map_err(|e| JsValue::from_str(&e)) }

/// 設定ファイルを読む：{ data: 設定の JSON, form: 欄の値 }。書き方の誤りは例外（説明の文）
#[wasm_bindgen]
pub fn config_load(text: &str) -> Result<JsValue, JsValue> {
    let conf = egword_core::parse_config(text).map_err(|e| JsValue::from_str(&e.0))?;
    #[derive(serde::Serialize)]
    struct Loaded { data: String, form: conftoml::ConfigForm }
    Ok(to_js(&Loaded { data: conftoml::table_to_json(&conf), form: conftoml::form_from_config(&conf) }))
}

/// 設定の欄の値
#[wasm_bindgen]
pub fn config_form(data: &str) -> Result<JsValue, JsValue> { Ok(to_js(&conftoml::form_from_config(&data_of(data)?))) }

/// 元の設定に欄の値を重ねたもの（JSON）
#[wasm_bindgen]
pub fn config_apply(data: &str, form: JsValue) -> Result<String, JsValue> {
    let form: conftoml::ConfigForm = serde_wasm_bindgen::from_value(form).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(conftoml::table_to_json(&conftoml::form_to_config(&data_of(data)?, &form)))
}

/// 二つの設定が同じか（並びは問わない）
#[wasm_bindgen]
pub fn config_same(a: &str, b: &str) -> Result<bool, JsValue> { Ok(data_of(a)? == data_of(b)?) }

/// 設定を TOML のテキストにする
#[wasm_bindgen]
pub fn config_dump(data: &str) -> Result<String, JsValue> { Ok(conftoml::dump_toml(&data_of(data)?)) }

/// 直したほうがよいところ（説明の文の一覧）。css_exists：設定の CSS が見つかるか（分からなければ true）
#[wasm_bindgen]
pub fn config_problems(data: &str, css_exists: bool) -> Result<Vec<String>, JsValue> {
    Ok(conftoml::check_config(&data_of(data)?, &|_| css_exists))
}

/// 新しい設定の雛形（JSON）
#[wasm_bindgen]
pub fn config_new(title: &str, identifier: &str, date: &str) -> String {
    conftoml::table_to_json(&conftoml::new_config(title, identifier, date))
}
