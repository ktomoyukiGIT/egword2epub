//! 割注（［＃割り注］…［＃改行］…［＃割り注終わり］）。tomBinder の段階7で追加
use egword_core::context::{build_systems, Context, ImageSource, Table};
use egword_core::inline::{inline, RenderPos};

struct NoImages;
impl ImageSource for NoImages {
    fn exists(&self, _: &str) -> bool { false }
    fn bytes(&self, _: &str) -> Option<Vec<u8>> { None }
}

fn run(text: &str) -> (String, Vec<String>) {
    let imgs = NoImages;
    let mut ctx = Context::new(&imgs, build_systems(&Table::new()).unwrap());
    let out = inline(&mut ctx, text, RenderPos { pos: 0, mode: "body" });
    (out, ctx.warnings.iter().map(|w| w.message.clone()).collect())
}

#[test]
fn warichu() {
    let (o, w) = run("SFWA(［＃割り注］アメリカSFファ［＃改行］ンタジー作家協会［＃割り注終わり］)主催");
    assert_eq!(o, "SFWA(<span class=\"warichu\"><span class=\"wr\">アメリカSFファ</span><span class=\"wr\">ンタジー作家協会</span></span>)主催");
    assert!(w.is_empty(), "{:?}", w);
    // ［＃改行］がなければ字数の半分で分ける。ルビは途中で割らない
    let (o, _) = run("あ［＃割り注］いうえお漢字《かんじ》［＃割り注終わり］");
    assert_eq!(o, "あ<span class=\"warichu\"><span class=\"wr\">いうえ</span><span class=\"wr\">お<ruby>漢字<rt>かんじ</rt></ruby></span></span>");
    // 割注の外の［＃改行］は取り除いて知らせる
    let (o, w) = run("あ［＃改行］い");
    assert_eq!(o, "あい");
    assert_eq!(w.len(), 1);
}
