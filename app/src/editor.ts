// エディタ（CodeMirror）：行番号・色分け・頁の区切り・探す／置き換え
import { EditorState, RangeSetBuilder, StateEffect, StateField, Compartment } from '@codemirror/state';
import {
  Decoration, DecorationSet, EditorView, MatchDecorator, ViewPlugin, ViewUpdate, WidgetType,
  drawSelection, highlightActiveLine, highlightActiveLineGutter, highlightSpecialChars, keymap, lineNumbers,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search';

/** 探す・置き換えの欄などの日本語 */
const PHRASES = {
  'Find': '探す', 'Replace': '置き換え', 'next': '次', 'previous': '前', 'all': 'すべて',
  'match case': '大文字小文字を区別', 'by word': '語単位', 'regexp': '正規表現',
  'replace': '置き換え', 'replace all': 'すべて置き換え', 'close': '閉じる',
  'current match': '現在の一致', 'replaced $ matches': '$ 件を置き換えました', 'replaced match on line $': '$ 行目を置き換えました',
  'on line': '行目', 'Go to line': '行へ移動', 'go': '移動', 'Control character': '制御文字',
};

function matcher(re: RegExp, cls: string) {
  const deco = new MatchDecorator({ regexp: re, decoration: Decoration.mark({ class: cls }) });
  return ViewPlugin.fromClass(class {
    decorations: DecorationSet;
    constructor(view: EditorView) { this.decorations = deco.createDeco(view); }
    update(u: ViewUpdate) { this.decorations = deco.updateDeco(u, this.decorations); }
  }, { decorations: (v) => v.decorations });
}

/** 改ページ記号（\f）を「改頁」の印で見せる */
class FormFeed extends WidgetType {
  toDOM() {
    const s = document.createElement('span');
    s.className = 'cm-ff';
    s.textContent = '改頁';
    s.title = '改ページ記号';
    return s;
  }
  ignoreEvent() { return false; }
}

/** 頁の始まりの行（外から渡す）に線と頁の番号を付ける */
export const setPageStarts = StateEffect.define<number[]>();
const pageStarts = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    for (const e of tr.effects) {
      if (e.is(setPageStarts)) {
        const b = new RangeSetBuilder<Decoration>();
        const doc = tr.state.doc;
        e.value.forEach((line, i) => {
          if (i === 0 || line > doc.lines) return;
          b.add(doc.line(line).from, doc.line(line).from,
                Decoration.line({ class: 'cm-pagestart', attributes: { 'data-page': `${i + 1} 頁` } }));
        });
        return b.finish();
      }
    }
    return deco.map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});

export interface EditorHooks {
  onChange(): void;
  onCursor(): void;
}

export const fontSize = new Compartment();

export function fontTheme(px: number) {
  return EditorView.theme({ '&': { fontSize: `${px}px` } });
}

let EXTENSIONS: any[] = [];

export function createEditor(parent: HTMLElement, hooks: EditorHooks, extraKeys: any[]): EditorView {
  EXTENSIONS = [
      EditorState.phrases.of(PHRASES),
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightActiveLine(),
      history(),
      drawSelection(),
      highlightSpecialChars({
        render: (code, description, placeholder) => {
          if (code === 12) return new FormFeed().toDOM();
          const s = document.createElement('span');
          s.className = 'cm-specialChar';
          s.textContent = placeholder;
          if (description) s.title = description;
          return s;
        },
      }),
      EditorView.lineWrapping,
      search({ top: true }),
      highlightSelectionMatches(),
      matcher(/(?:※|＊|注)[0-9０-９]+|\[[0-9０-９]+\]/g, 'cm-note'),
      matcher(/《《[^《》\n]*》》|《[^《》\n]*》|｜/g, 'cm-ruby'),
      matcher(/［＃[^］\n]*］/g, 'cm-annot'),
      matcher(/^[ \t　]*［＃改(?:ページ|丁|段|見開き)］[ \t　]*$/gm, 'cm-pagebreak'),
      pageStarts,
      fontSize.of(fontTheme(16)),
      keymap.of([...extraKeys, ...searchKeymap, ...historyKeymap, ...defaultKeymap, indentWithTab]),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) hooks.onChange();
        if (u.docChanged || u.selectionSet) hooks.onCursor();
      }),
      EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off', lang: 'ja' }),
  ];
  return new EditorView({ state: EditorState.create({ doc: '', extensions: EXTENSIONS }), parent });
}

/** 文字列を丸ごと入れ替える（取り消しの記録も新しくする） */
export function resetDoc(view: EditorView, text: string, fontPx: number) {
  view.setState(EditorState.create({ doc: text, extensions: EXTENSIONS }));
  view.dispatch({ effects: fontSize.reconfigure(fontTheme(fontPx)) });
}
