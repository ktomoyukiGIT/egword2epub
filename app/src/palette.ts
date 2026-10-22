// 注記の一覧（左の欄とメニューの「挿入」）

export type Action =
  | { t: 'line'; s: string }                 // 一行として入れる
  | { t: 'prefix'; s: string }               // 選んだ行の頭に付ける
  | { t: 'decorate'; name: string }          // 文字の飾り
  | { t: 'block'; open: string; close: string }
  | { t: 'opener' } | { t: 'chapter' } | { t: 'indent' } | { t: 'raise' } | { t: 'indentBlock' }
  | { t: 'ruby' } | { t: 'periodical' } | { t: 'gaiji' } | { t: 'figure' };

export interface Item { label: string; hint: string; action: Action; key?: string }
export interface Section { title: string; items: Item[]; open?: boolean }

const deco = (names: string[], hint: string): Item[] => names.map((n) => ({ label: n, hint, action: { t: 'decorate', name: n } }));
const blk = (label: string, open: string, close: string, hint: string): Item => ({ label, hint, action: { t: 'block', open, close } });

export const PALETTE: Section[] = [
  { title: '頁と章', open: true, items: [
    { label: '改ページ', hint: 'ここで頁を改める。', action: { t: 'line', s: '［＃改ページ］' } },
    { label: '章扉…', hint: '画像を章扉として置き、ここから新しい章。題名は目次にだけ使う。', action: { t: 'opener' } },
    { label: '章…', hint: 'この頁を目次に載せ、ここから新しい章（本文には何も出ない）。「補遺／書誌」で階層。', action: { t: 'chapter' } },
    { label: '目次を置く', hint: 'ここに目次を作る。', action: { t: 'line', s: '［＃目次］' } },
    { label: '頁の左右中央', hint: 'この頁を中央に組む（献辞など）。', action: { t: 'line', s: '［＃ページの左右中央］' } },
  ] },
  { title: '見出し', items: deco(['大見出し', '中見出し', '小見出し'], '選んだ文字を見出しに。選ばずに押すと始めと終わりを入れる。') },
  { title: 'ルビ・文字の飾り', open: true, items: [
    { label: 'ルビ…', hint: '選んだ文字にルビ。漢字以外や途中からの語には「｜」を自動で付ける。', action: { t: 'ruby' }, key: 'Mod-r' },
    { label: '誌名《《 》》', hint: '誌名・紙名。ルビと区別するために二重の《《 》》にする。', action: { t: 'periodical' } },
    { label: '外字…', hint: '※［＃「説明」、U+XXXX］ の形の外字注記。', action: { t: 'gaiji' } },
    ...deco(['傍点', '白ゴマ傍点', '丸傍点', '白丸傍点', '傍線', '二重傍線', '鎖線', '破線', '波線', '太字', '斜体', '縦中横'],
            '選んだ文字に付ける。選ばずに押すと範囲の始めと終わりを入れる。'),
  ] },
  { title: '文字の大小', items: deco(['１段階小さな文字', '２段階小さな文字', '１段階大きな文字', '２段階大きな文字'], '選んだ文字の大きさを変える。') },
  { title: '一行の字下げ・字上げ', items: [
    { label: '字下げ…', hint: '行頭に ［＃N字下げ］。選んだ行すべてに付く。', action: { t: 'indent' } },
    { label: '地付き', hint: '行を地付きに。', action: { t: 'prefix', s: '［＃地付き］' } },
    { label: '地から字上げ…', hint: '地付きにして N 字上げる。', action: { t: 'raise' } },
    { label: '中央揃え', hint: '行を中央揃えに（縦書きでは天地の中央）。', action: { t: 'prefix', s: '［＃中央揃え］' } },
  ] },
  { title: '範囲', items: [
    { label: '範囲の字下げ…', hint: '選んだ行を ［＃ここからN字下げ］…［＃ここで字下げ終わり］ で囲む。', action: { t: 'indentBlock' } },
    blk('範囲の地付き', '［＃ここから地付き］', '［＃ここで地付き終わり］', '選んだ行を地付きに。'),
    blk('範囲の中央揃え', '［＃ここから中央揃え］', '［＃ここで中央揃え終わり］', '選んだ行を中央揃えに。'),
    blk('範囲の１段階小さな文字', '［＃ここから１段階小さな文字］', '［＃ここで小さな文字終わり］', '選んだ行の文字を小さく。'),
    blk('範囲の１段階大きな文字', '［＃ここから１段階大きな文字］', '［＃ここで大きな文字終わり］', '選んだ行の文字を大きく。'),
    blk('横組み', '［＃ここから横組み］', '［＃ここで横組み終わり］', '縦書きの本の中で、選んだ行を横組みに（原詩など）。'),
  ] },
  { title: '図版・区切り', items: [
    { label: '図…', hint: '画像を図版として置く。説明を付けられる（「／」で改行）。', action: { t: 'figure' } },
    ...['実線', '破線', '点線', '二重線'].map((s) => ({ label: `区切り線（${s}）`, hint: '一行の区切り線。',
      action: { t: 'line' as const, s: s === '実線' ? '［＃区切り線］' : `［＃区切り線（${s}）］` } })),
  ] },
];
