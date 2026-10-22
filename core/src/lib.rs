//! egword2epub の変換の本体（Rust 版）
//!
//! Python 版（egword2epub）と同じ規則で、青空文庫形式の注記のテキストを EPUB3 にする。
//! 同じ入力からは Python 版と同じ XHTML・目次・OPF・警告を出す（tests/parity.rs で確かめる）。
//! ファイルを読み書きしないので、コマンド・アプリ・ブラウザ（WebAssembly）のどれからでも使える。

pub mod basics;
pub mod blocks;
pub mod conftoml;
pub mod context;
pub mod convert;
pub mod editops;
pub mod epub;
pub mod inline;
pub mod ir;
pub mod notes;
pub mod py;
pub mod structure;

pub use context::{ConvertError, ImageSource, Table, Warning};
pub use convert::{build_book, decode_text, encode_text, normalize, parse_config, ConvertInput, TextEncoding, DEFAULT_CSS};
pub use epub::{image_size, Book};
pub use structure::OutputFile;

/// 画像がどこにもないとき
pub struct NoImages;
impl ImageSource for NoImages {
    fn exists(&self, _: &str) -> bool { false }
    fn bytes(&self, _: &str) -> Option<Vec<u8>> { None }
}

/// フォルダの中の画像
pub struct DirImages(pub std::path::PathBuf);
impl ImageSource for DirImages {
    fn exists(&self, name: &str) -> bool { self.0.join(name).exists() }
    fn bytes(&self, name: &str) -> Option<Vec<u8>> { std::fs::read(self.0.join(name)).ok() }
}
