// 設定ファイルを画面で作る・直す窓（Python 版の confedit.py と同じ作り）
//
// タブ「書誌」「組版と注」「奥付」で、よく使う項目を欄で直す。
// タブ「テキスト」では、設定ファイルをそのまま直せる（欄にない項目もここで）。
// 欄を変えずに保存したときは、テキストをそのまま保存するので、元のコメントは残る。
// 欄を変えて保存すると、ファイルは書き直される（コメントは消える）。
// 欄 ⇔ 設定の中身は Rust の conftoml（Python 版と同じ結果になることを確かめてある）。
import * as core from '../pkg/egword_wasm.js';
import { message } from './dialogs';

export interface Creator { name: string; role: string; file_as: string; src: number | null }
export interface NoteForm { format: string; label: string; heading_lines: string }
export interface ConfigForm {
  title: string; title_file_as: string; publisher: string; date: string; description: string; identifier: string;
  language: string; creators: Creator[]; writing_mode: string; css: string; autolink: string[];
  footnote: NoteForm; endnote: NoteForm; guess_periodicals: boolean; squeeze_blank_lines: boolean;
  chapter_image: string; cover_image: string; colophon: boolean; colophon_title: string;
  colophon_date_label: string; colophon_lines: string;
}

export interface ConfigEditorHost {
  name: string;
  path?: string;
  /** 新しく作るときは null（雛形から始める） */
  text: string | null;
  newTitle: string;
  /** 設定の css が見つかるか（分からなければ null） */
  cssExists(name: string): Promise<boolean | null>;
  /** CSS を選ぶ。設定に書く名前を返す */
  pickCss?(): Promise<string | null>;
  /** 保存する。保存できたら、そのときの名前と場所 */
  save(text: string): Promise<{ name: string; path?: string } | null>;
}

const ROLES: [string, string][] = [['aut', '著'], ['trl', '訳'], ['edt', '編'], ['ill', '絵']];
const NOTE_FORMATS = ['※1', '＊1', '注1', '[1]', '1]', 'none'];
const LINKS: [string, string][] = [['url', 'URL'], ['mail', 'メールアドレス'], ['x', 'X のアカウント（@名前）']];

type Attrs = Record<string, string>;
function el<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...kids: (Node | string)[]): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  for (const k of kids) e.append(k);
  return e;
}
const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};
const newIdentifier = () => 'urn:uuid:' + (crypto.randomUUID ? crypto.randomUUID() :
  '10000000-1000-4000-8000-100000000000'.replace(/[018]/g, (c) =>
    (+c ^ (crypto.getRandomValues(new Uint8Array(1))[0] & (15 >> (+c / 4)))).toString(16)));

/** 設定ファイルの窓を開く。閉じたら、最後に保存したときの {name, path}（保存しなければ null） */
export function openConfigEditor(host: ConfigEditorHost): Promise<{ name: string; path?: string } | null> {
  return new Promise((resolve) => {
    let data: string;           // いまの設定（JSON）
    let savedText: string | null = host.text;
    let lastSaved: { name: string; path?: string } | null = null;
    let initialText: string;
    if (host.text === null) {
      data = core.config_new(host.newTitle, newIdentifier(), today());
      initialText = core.config_dump(data);
    } else {
      try {
        data = core.config_load(host.text).data;
      } catch (e) {
        data = core.config_new(host.newTitle, newIdentifier(), today());
        // 読めないときは、テキストのタブで直してもらう
        setTimeout(() => { selectTab(3, true); showStatus([String(e)], true); }, 0);
      }
      initialText = host.text;
    }

    const d = el('dialog', { class: 'dlg wide cfg' });
    const title = el('h2', {}, `設定ファイル：${host.name}`);
    const where = el('p', { class: 'note cfg-path' }, host.path ?? (host.text === null ? '新しい設定ファイル（保存するときに場所を決めます）' : ''));
    const tabs = el('div', { class: 'cfg-tabs', role: 'tablist' });
    const pages: HTMLElement[] = [];
    const TAB_NAMES = ['書誌', '組版と注', '奥付', 'テキスト'];
    TAB_NAMES.forEach((name, i) => {
      const b = el('button', { type: 'button', role: 'tab', 'data-i': String(i) }, name);
      b.addEventListener('click', () => selectTab(i));
      tabs.append(b);
      pages.push(el('div', { class: 'cfg-page', role: 'tabpanel' }));
    });
    const status = el('div', { class: 'note cfg-status' });

    // ---------------- 欄を作る ----------------
    const inputs: Record<string, HTMLInputElement | HTMLTextAreaElement> = {};
    function row(page: HTMLElement, label: string, input: HTMLElement, hint = '', button?: [string, () => void]) {
      const r = el('div', { class: 'cfg-row' });
      r.append(el('label', {}, label));
      const cell = el('div', { class: 'cfg-cell' });
      const line = el('div', { class: 'cfg-line' }, input);
      if (button) {
        const b = el('button', { type: 'button', class: 'small' }, button[0]);
        b.addEventListener('click', button[1]);
        line.append(b);
      }
      cell.append(line);
      if (hint) cell.append(el('small', {}, hint));
      r.append(cell);
      page.append(r);
    }
    function text(key: string, page: HTMLElement, label: string, hint = '', button?: [string, () => void], multi = false) {
      const i = multi ? el('textarea', { rows: '2' }) : el('input', { type: 'text' });
      inputs[key] = i;
      row(page, label, i, hint, button);
      return i;
    }

    // 書誌
    const pb = pages[0];
    text('title', pb, '書名');
    text('title_file_as', pb, '書名のよみ');
    text('publisher', pb, '発行者');
    text('date', pb, '発行日', '2026-10-19 の形で', ['今日', () => { inputs.date.value = today(); }]);
    text('description', pb, '説明文', '', undefined, true);
    text('language', pb, '言語', '日本語なら ja');
    text('identifier', pb, '識別子', '同じ本の改訂版では変えない。空なら変換のたびに新しく作る',
      ['作る', () => { inputs.identifier.value = newIdentifier(); }]);
    const creatorsBox = el('div', { class: 'cfg-creators' });
    const addCreatorBtn = el('button', { type: 'button', class: 'small' }, '＋ 作者を足す');
    const creatorsCell = el('div', { class: 'cfg-cell' }, creatorsBox, el('div', {}, addCreatorBtn));
    pb.append(el('div', { class: 'cfg-row' }, el('label', {}, '作者'), creatorsCell));
    interface CreatorRow { name: HTMLInputElement; role: HTMLSelectElement; file_as: HTMLInputElement; src: number | null; el: HTMLElement }
    let creatorRows: CreatorRow[] = [];
    function addCreator(c: Creator = { name: '', role: 'aut', file_as: '', src: null }) {
      const name = el('input', { type: 'text', placeholder: '名前', 'aria-label': '名前' });
      name.value = c.name;
      const role = el('select', { 'aria-label': '役割' });
      const roles = [...ROLES];
      if (!roles.some(([k]) => k === c.role)) roles.push([c.role, c.role]);
      for (const [k, v] of roles) role.append(el('option', { value: k }, v === k ? k : `${v}（${k}）`));
      role.value = c.role;
      const fileAs = el('input', { type: 'text', placeholder: 'よみ', 'aria-label': 'よみ' });
      fileAs.value = c.file_as;
      const del = el('button', { type: 'button', class: 'small', title: 'この作者を消す' }, '消す');
      const line = el('div', { class: 'cfg-creator' }, name, role, fileAs, del);
      const r: CreatorRow = { name, role, file_as: fileAs, src: c.src, el: line };
      del.addEventListener('click', () => { line.remove(); creatorRows = creatorRows.filter((x) => x !== r); });
      creatorsBox.append(line);
      creatorRows.push(r);
    }
    addCreatorBtn.addEventListener('click', () => { addCreator(); creatorRows[creatorRows.length - 1].name.focus(); });

    // 組版と注
    const pl = pages[1];
    const modeBox = el('div', { class: 'cfg-inline' });
    const modes: Record<string, HTMLInputElement> = {};
    for (const [v, label] of [['vertical', '縦書き'], ['horizontal', '横書き']]) {
      const r = el('input', { type: 'radio', name: 'cfg-mode', value: v });
      modes[v] = r;
      modeBox.append(el('label', { class: 'check' }, r, label));
    }
    row(pl, '組み方向', modeBox);
    text('css', pl, 'CSS', '空なら同梱の標準の CSS。設定ファイルの場所から見た位置で書く',
      host.pickCss ? ['選ぶ…', async () => { const n = await host.pickCss!(); if (n) inputs.css.value = n; }] : undefined);
    const linkBox = el('div', { class: 'cfg-stack' });
    const links: Record<string, HTMLInputElement> = {};
    for (const [k, label] of LINKS) {
      const c = el('input', { type: 'checkbox' });
      links[k] = c;
      linkBox.append(el('label', { class: 'check' }, c, label));
    }
    row(pl, '自動リンク', linkBox);
    const notes: Record<string, { format: HTMLSelectElement; label: HTMLInputElement; heading_lines: HTMLInputElement }> = {};
    for (const [key, name] of [['footnote', '脚注'], ['endnote', '文末注']]) {
      const fs = el('fieldset', { class: 'cfg-box' }, el('legend', {}, name));
      const format = el('select', { 'aria-label': `${name}の書式` });
      const label = el('input', { type: 'text' });
      const hl = el('input', { type: 'text' });
      notes[key] = { format, label, heading_lines: hl };
      row(fs, '書式', format);
      row(fs, '見出し', label);
      row(fs, '取り除く行', hl, '注の並びの前の見出し行（例 ＊訳注）。いくつかあれば空白で区切る');
      pl.append(fs);
    }
    const old = el('fieldset', { class: 'cfg-box' }, el('legend', {}, '以前の原稿のための設定'));
    const flags: Record<string, HTMLInputElement> = {};
    for (const [k, label] of [['guess_periodicals', '漢字以外の直後の《…》を誌名とみなす'],
                              ['squeeze_blank_lines', '範囲注記のある頁で、続く空行を一つにまとめる']]) {
      const c = el('input', { type: 'checkbox' });
      flags[k] = c;
      old.append(el('label', { class: 'check cfg-flag' }, c, label));
    }
    text('chapter_image', old, '章扉の画像名');
    text('cover_image', old, '表紙の画像名', '正規表現（例 ch(\\d+)\\.jpg）。空なら使わない／既定のまま');
    pl.append(old);

    // 奥付
    const pc = pages[2];
    const colOn = el('input', { type: 'checkbox' });
    pc.append(el('label', { class: 'check cfg-flag' }, colOn, '最後に奥付の頁を作る（書名・作者・発行日・発行者は書誌から取ります）'));
    text('colophon_title', pc, '目次での題名', '空なら「奥付」');
    text('colophon_date_label', pc, '日付の前の語', '空なら「発行」');
    const colLines = el('textarea', { rows: '7' });
    inputs.colophon_lines = colLines;
    row(pc, '足す行', colLines, '一行ずつ（空の行は一行あけ）。例：底本：……');

    // テキスト
    const pt = pages[3];
    pt.append(el('p', { class: 'note' }, '設定ファイルそのもの。欄にない項目もここで直せます（README の「設定ファイル」）。'));
    const ta = el('textarea', { class: 'config', spellcheck: 'false' });
    pt.append(ta);

    // ---------------- 欄 ⇔ 設定 ----------------
    let otherMode = 'vertical';
    function fill(f: ConfigForm) {
      for (const k of ['title', 'title_file_as', 'publisher', 'date', 'description', 'identifier', 'language', 'css',
                       'chapter_image', 'cover_image', 'colophon_title', 'colophon_date_label', 'colophon_lines'] as const) {
        inputs[k].value = f[k];
      }
      creatorRows.forEach((r) => r.el.remove());
      creatorRows = [];
      f.creators.forEach((c) => addCreator(c));
      for (const [v, r] of Object.entries(modes)) r.checked = f.writing_mode === v;
      otherMode = f.writing_mode;   // 縦・横のどちらでもないときは、そのまま残す（「確かめる」で知らせる）
      for (const [k, c] of Object.entries(links)) c.checked = f.autolink.includes(k);
      for (const key of ['footnote', 'endnote'] as const) {
        const n = notes[key];
        n.format.replaceChildren();
        const opts = NOTE_FORMATS.includes(f[key].format) ? NOTE_FORMATS : [...NOTE_FORMATS, f[key].format];
        for (const o of opts) n.format.append(el('option', { value: o }, o === 'none' ? 'なし（none）' : o));
        n.format.value = f[key].format;
        n.label.value = f[key].label;
        n.heading_lines.value = f[key].heading_lines;
      }
      for (const [k, c] of Object.entries(flags)) c.checked = (f as any)[k];
      colOn.checked = f.colophon;
    }
    function read(): ConfigForm {
      const v = (k: string) => inputs[k].value;
      const note = (key: string): NoteForm => ({ format: notes[key].format.value, label: notes[key].label.value, heading_lines: notes[key].heading_lines.value });
      return {
        title: v('title'), title_file_as: v('title_file_as'), publisher: v('publisher'), date: v('date'),
        description: v('description'), identifier: v('identifier'), language: v('language'),
        creators: creatorRows.map((r) => ({ name: r.name.value, role: r.role.value, file_as: r.file_as.value, src: r.src })),
        writing_mode: Object.entries(modes).find(([, r]) => r.checked)?.[0] ?? otherMode,
        css: v('css'), autolink: LINKS.map(([k]) => k).filter((k) => links[k].checked),
        footnote: note('footnote'), endnote: note('endnote'),
        guess_periodicals: flags.guess_periodicals.checked, squeeze_blank_lines: flags.squeeze_blank_lines.checked,
        chapter_image: v('chapter_image'), cover_image: v('cover_image'),
        colophon: colOn.checked, colophon_title: v('colophon_title'), colophon_date_label: v('colophon_date_label'),
        colophon_lines: v('colophon_lines'),
      };
    }
    const formToData = () => core.config_apply(data, read());

    fill(core.config_form(data));
    ta.value = initialText;
    let baseline = formToData();

    // ---------------- タブの行き来 ----------------
    let current = 0;
    function selectTab(i: number, force = false) {
      if (i === current && !force) return;
      if (current === 3 && i !== 3) {
        // テキストで直した内容を欄へ
        try {
          data = core.config_load(ta.value).data;
        } catch (e) {
          showStatus([String(e)], true);
          return;
        }
        fill(core.config_form(data));
        baseline = formToData();
        showStatus([]);
      } else if (i === 3 && current !== 3) {
        const nd = formToData();
        if (!core.config_same(nd, baseline)) {   // 欄を変えたときだけ書き直す（変えていなければコメントも残る）
          data = nd;
          ta.value = core.config_dump(nd);
          baseline = nd;
        }
      }
      current = i;
      tabs.querySelectorAll('button').forEach((b, j) => {
        b.classList.toggle('on', j === i);
        b.setAttribute('aria-selected', String(j === i));
      });
      pages.forEach((p, j) => { p.hidden = j !== i; });
    }

    /** いまの内容：[テキスト, 設定の JSON]。テキストの誤りは例外 */
    function currentContent(): [string, string] {
      if (current === 3) return [ta.value, core.config_load(ta.value).data];
      const nd = formToData();
      if (core.config_same(nd, baseline)) return [ta.value, core.config_load(ta.value).data];
      return [core.config_dump(nd), nd];
    }

    function showStatus(lines: string[], bad = false, ok = '') {
      status.replaceChildren();
      status.classList.toggle('bad', bad);
      if (!lines.length) { status.textContent = ok; return; }
      if (lines.length === 1) { status.textContent = lines[0]; return; }
      const ul = el('ul');
      for (const l of lines) ul.append(el('li', {}, l));
      status.append(ul);
    }

    async function problemsOf(json: string): Promise<string[]> {
      const css = (core.config_form(json) as ConfigForm).css.trim();
      const exists = css ? await host.cssExists(css) : true;
      return core.config_problems(json, exists !== false);
    }

    async function verify() {
      let json: string;
      try { json = currentContent()[1]; } catch (e) { showStatus([String(e)], true); return; }
      const probs = await problemsOf(json);
      if (probs.length) showStatus(['直したほうがよいところがあります：', ...probs], true);
      else showStatus([], false, '問題は見つかりませんでした。');
    }

    async function save(): Promise<boolean> {
      let content: [string, string];
      try { content = currentContent(); } catch (e) {
        showStatus([`保存できません。${e}`], true);
        return false;
      }
      const [t, json] = content;
      const probs = await problemsOf(json);
      if (probs.length) {
        const a = await message('設定ファイル', '直したほうがよいところがあります：\n\n・' + probs.join('\n・') + '\n\nこのまま保存しますか？',
          ['やめる', '保存する']);
        if (a !== 1) return false;
      }
      const out = t.endsWith('\n') ? t : t + '\n';
      const r = await host.save(out);
      if (!r) return false;
      lastSaved = r;
      host.name = r.name;
      host.path = r.path;
      title.textContent = `設定ファイル：${r.name}`;
      where.textContent = r.path ?? '';
      data = json;
      savedText = out;
      ta.value = out;
      fill(core.config_form(data));
      baseline = formToData();
      showStatus([], false, '保存しました。');
      return true;
    }

    function changed(): boolean {
      try {
        const [t] = currentContent();
        return savedText === null || t.replace(/\n+$/, '') !== savedText.replace(/\n+$/, '');
      } catch { return true; }
    }

    let done = false;
    async function close() {
      if (changed()) {
        const a = await message('設定ファイル', '変更を保存しますか？', ['キャンセル', '保存しない', '保存する']);
        if (a === 0 || a === -1) return;
        if (a === 2 && !(await save())) return;
      }
      if (done) return;
      done = true;
      d.close();
      d.remove();
      resolve(lastSaved);
    }

    const buttons = el('div', { class: 'buttons' });
    const bVerify = el('button', { type: 'button' }, '確かめる');
    const bClose = el('button', { type: 'button' }, '閉じる');
    const bSave = el('button', { type: 'button', class: 'primary' }, '保存');
    bVerify.addEventListener('click', verify);
    bClose.addEventListener('click', close);
    bSave.addEventListener('click', () => { save(); });
    buttons.append(bVerify, el('span', { class: 'spacer' }), bClose, bSave);
    d.addEventListener('cancel', (e) => { e.preventDefault(); close(); });

    const body = el('div', { class: 'cfg-body' }, ...pages);
    d.append(title, where, tabs, body, status, buttons);
    document.body.append(d);
    pages.forEach((p, j) => { p.hidden = j !== 0; });
    tabs.querySelector('button')!.classList.add('on');
    d.showModal();
    inputs.title.focus();
  });
}
