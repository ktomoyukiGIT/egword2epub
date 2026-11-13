//! 地付き・地寄せ（字上げ）の「文字列の末尾」の形と、範囲の字上げ。tomBinder の段階7で追加
use egword_core::blocks::lines_html;
use egword_core::context::{build_systems, Context, ImageSource, Table};
use egword_core::inline::RenderPos;

struct NoImages;
impl ImageSource for NoImages {
    fn exists(&self, _: &str) -> bool { false }
    fn bytes(&self, _: &str) -> Option<Vec<u8>> { None }
}

fn run(lines: &[&str]) -> (String, Vec<String>) {
    let imgs = NoImages;
    let mut ctx = Context::new(&imgs, build_systems(&Table::new()).unwrap());
    let ls: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let out = lines_html(&mut ctx, &ls, RenderPos { pos: 0, mode: "body" });
    (out, ctx.warnings.iter().map(|w| w.message.clone()).collect())
}

#[test]
fn end_of_string() {
    // 行の途中の［＃地付き］：後ろの部分だけを地に寄せる
    let (o, w) = run(&["　右のとおり報告いたします。［＃地付き］山田太郎"]);
    assert_eq!(o, "<p class=\"chi-line\" style=\"text-indent:1em;\">右のとおり報告いたします。<span class=\"chi-end\">山田太郎</span></p>");
    assert!(w.is_empty(), "{:?}", w);
    // ［＃地から２字上げ］：二字上げる
    let (o, _) = run(&["本文［＃地から２字上げ］一九二二年"]);
    assert_eq!(o, "<p class=\"chi-line\" style=\"text-indent:0em;\">本文<span class=\"chi-end\" style=\"margin-bottom:2em;\">一九二二年</span></p>");
    // 行頭・行末の［＃地付き］は、これまでどおり行全体を地付きに
    let (o, _) = run(&["［＃地付き］山田太郎"]);
    assert_eq!(o, "<p style=\"text-align:end;text-indent:0em;\">山田太郎</p>");
    let (o, _) = run(&["山田太郎［＃地付き］"]);
    assert_eq!(o, "<p style=\"text-align:end;text-indent:0em;\">山田太郎</p>");
}

#[test]
fn block_raise() {
    // ［＃ここから地から２字上げ］だけなら地寄せ（地に寄せて二字上げる）
    let (o, _) = run(&["［＃ここから地から２字上げ］", "あ", "［＃ここで字上げ終わり］"]);
    assert_eq!(o, "<p style=\"text-align:end;margin-bottom:2em;text-indent:0em;\">あ</p>");
    // 字下げと組み合わせたものは字詰め（地には寄せない）
    let (o, _) = run(&["［＃ここから２字下げ、地から２字上げ］", "あ", "［＃ここで字下げ終わり］"]);
    assert_eq!(o, "<p style=\"margin-top:2em;margin-bottom:2em;text-indent:0em;\">あ</p>");
}
