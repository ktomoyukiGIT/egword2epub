// Egword2EPUB：原稿のエディタ・縦書きプレビュー・EPUB の書き出し（デスクトップとブラウザで同じ画面）
import { EditorView } from '@codemirror/view';
import { isolateHistory } from '@codemirror/commands';
import { openSearchPanel } from '@codemirror/search';
import init, * as core from '../pkg/egword_wasm.js';
import { ask, askIndentBlock, askNumber, askText, choose, message } from './dialogs';
import { openConfigEditor } from './configform';
import { createEditor, fontSize, fontTheme, resetDoc, setPageStarts } from './editor';
import { Action, PALETTE } from './palette';
import { ConfigFile, ImageSet, TextFile, basename, dirname, platform } from './platform';
import { Preview } from './preview';

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const store = {
  get(k: string) { try { return localStorage.getItem('egword:' + k); } catch { return null; } },
  set(k: string, v: string | null) { try { v === null ? localStorage.removeItem('egword:' + k) : localStorage.setItem('egword:' + k, v); } catch { /* なし */ } },
};

interface Problem { line: number | null; start: number; end: number; message: string }

// ================================================================ 状態

let view: EditorView;
let conv: core.Converter;
let preview: Preview;
let doc: TextFile | null = null;
let encoding = 'utf-8';
let encodingLabel = 'UTF-8';
let bom = false;
let crlf = false;
let dirty = false;
let images: ImageSet | null = null;
let config: ConfigFile | null = null;
let pageStarts: number[] = [1];
let problems: Problem[] = [];
let built = false;
let autoPreview = store.get('autoPreview') !== 'off';
let fontPx = parseInt(store.get('fontPx') || '16', 10) || 16;
let convertTimer: number | undefined;
let posTimer: number | undefined;
let pageTimer: number | undefined;
let draftTimer: number | undefined;

const text = () => view.state.doc.toString();

// ================================================================ 表示

function setStatus(msg: string, bad = false) {
  $('status-msg').textContent = msg;
  $('status-msg').classList.toggle('bad', bad);
}

function updateTitle() {
  const name = doc?.name ?? '新しいテキスト';
  document.title = `${dirty ? '● ' : ''}${name} — Egword2EPUB`;
  $('doc-name').textContent = name;
  $('doc-name').classList.toggle('dirty', dirty);
}

function setDirty(d: boolean) {
  if (dirty !== d) { dirty = d; updateTitle(); }
}

function updateSettingsBar() {
  $('img-label').textContent = images ? `${images.label}（${images.files.size}）` : '（未選択）';
  $('img-label').title = images?.path ?? images?.label ?? '';
  $('cfg-label').textContent = config ? config.name + (config.cssName ? `＋${config.cssName}${config.css ? '' : '（CSS が見つからない）'}` : '') : '（設定なし）';
  $('cfg-label').title = config?.path ?? config?.name ?? '';
  $('cfg-edit').textContent = config ? '編集…' : '新規…';
  $('cfg-clear').hidden = !config;
}

function updatePosition() {
  const st = view.state;
  const line = st.doc.lineAt(st.selection.main.head).number;
  let page = 0;
  pageStarts.forEach((s, i) => { if (s <= line) page = i; });
  $('pos').textContent = `${line} 行　${page + 1}／${pageStarts.length} 頁　${encodingLabel}・${crlf ? 'CRLF' : 'LF'}`;
  if (built) {
    const fn = conv.file_for_page(page) ?? preview.pages[0];
    if (fn && fn !== preview.name) preview.show(fn);
  }
}

function refreshPageStarts() {
  pageStarts = Array.from(core.page_start_lines(text()));
  view.dispatch({ effects: setPageStarts.of(pageStarts) });
}

function renderProblems() {
  const tbody = $('problems-body');
  tbody.textContent = '';
  for (const p of problems) {
    const tr = document.createElement('tr');
    const td1 = document.createElement('td');
    td1.textContent = p.line ? String(p.line) : '—';
    const td2 = document.createElement('td');
    td2.textContent = p.message;
    tr.append(td1, td2);
    tr.tabIndex = 0;
    const go = () => { jumpTo(p); if (isNarrow()) showTab('edit'); };
    tr.addEventListener('click', go);
    tr.addEventListener('keydown', (e) => { if (e.key === 'Enter') go(); });
    tbody.appendChild(tr);
  }
  $('problems-count').textContent = problems.length ? `問題 ${problems.length} 件` : '問題なし';
  $('tab-problems').dataset.count = problems.length ? String(problems.length) : '';
}

function jumpTo(p: Problem) {
  if (!p.line) return;
  const d = view.state.doc;
  const line = d.line(Math.min(p.line, d.lines));
  const from = p.end > p.start ? line.from + p.start : line.from;
  const to = p.end > p.start ? line.from + p.end : line.to;
  view.dispatch({ selection: { anchor: from, head: to }, scrollIntoView: true });
  view.focus();
}

// ================================================================ 変換

function nowIso() { return new Date().toISOString().replace(/\.\d+Z$/, 'Z'); }

/** 変換して、問題の一覧とプレビューを作り直す。できれば true */
function convert(): boolean {
  const t = text();
  const title = (doc?.name ?? 'book').replace(/\.[^.]*$/, '');
  try {
    conv.build(t, encoding, config?.text ?? undefined, config?.css ?? undefined, title, nowIso());
  } catch (e) {
    built = false;
    problems = [{ line: null, start: 0, end: 0, message: String(e) }];
    renderProblems();
    setStatus('変換できませんでした', true);
    return false;
  }
  built = true;
  const ws = conv.warnings() as { file: string; message: string }[];
  const rows = ws.map((w) => {
    const loc = conv.locate(t, w.file, w.message) as Uint32Array | undefined;
    return { line: loc ? loc[0] : null, start: loc ? loc[1] : 0, end: loc ? loc[2] : 0, message: w.message };
  });
  rows.sort((a, b) => (a.line ?? 1e9) - (b.line ?? 1e9));
  problems = rows;
  renderProblems();
  setStatus(problems.length ? `問題 ${problems.length} 件` : '問題は見つかりませんでした');
  preview.setBook({ files: conv.files(), css: conv.css() });
  updatePosition();
  return true;
}

function scheduleConvert(ms = 1200) {
  clearTimeout(convertTimer);
  convertTimer = window.setTimeout(convert, ms);
}

/** JPEG・PNG の大きな画像を縮小する（長い辺 2000px、表紙は 2560px）。縮小した数を返す */
async function shrinkImages(): Promise<number> {
  if (!images) return 0;
  const cover = conv.cover();
  let n = 0;
  for (const name of conv.used_images()) {
    const bytes = images.files.get(name);
    if (!bytes || !/\.(jpe?g|png)$/i.test(name)) continue;
    const limit = name === cover ? 2560 : 2000;
    const png = /\.png$/i.test(name);
    try {
      const bmp = await createImageBitmap(new Blob([bytes]));
      const long = Math.max(bmp.width, bmp.height);
      if (long <= limit) { bmp.close(); continue; }
      const k = limit / long;
      const c = document.createElement('canvas');
      c.width = Math.round(bmp.width * k);
      c.height = Math.round(bmp.height * k);
      const g = c.getContext('2d')!;
      g.imageSmoothingQuality = 'high';
      g.drawImage(bmp, 0, 0, c.width, c.height);
      bmp.close();
      const blob: Blob | null = await new Promise((r) => c.toBlob(r, png ? 'image/png' : 'image/jpeg', 0.85));
      if (blob) { conv.set_export_image(name, new Uint8Array(await blob.arrayBuffer())); n++; }
    } catch { /* 読めない画像はそのまま入れる */ }
  }
  return n;
}

async function exportEpub() {
  if (!convert()) { await message('変換できませんでした', problems[0]?.message ?? ''); return; }
  setStatus('EPUB を書き出しています…');
  const shrunk = await shrinkImages();
  const bytes = conv.epub();
  const name = (doc?.name ?? 'book').replace(/\.[^.]*$/, '') + '.epub';
  const saved = await platform.saveEpub(name, bytes, doc);
  if (!saved) { setStatus('書き出しをやめました'); return; }
  const n = problems.length;
  setStatus(`書き出しました：${basename(saved)}（${conv.files().length} ファイル、画像 ${conv.used_images().length} 点${shrunk ? `、うち ${shrunk} 点を縮小` : ''}）`);
  await message('EPUB を書き出しました', `${basename(saved)}\n` + (n ? `警告が ${n} 件あります。問題の一覧を見てください。` : '警告はありません。'));
}

// ================================================================ ファイル

function remember() {
  if (!doc?.path) return;
  store.set('doc:' + doc.path, JSON.stringify({ images: images?.path ?? null, config: config?.path ?? null }));
  store.set('lastFile', doc.path);
}

async function confirmDiscard(): Promise<boolean> {
  if (!dirty) return true;
  const i = await message(`「${doc?.name ?? '新しいテキスト'}」の変更を保存しますか？`, '保存しないと、変更は失われます。',
                          ['保存しない', 'キャンセル', '保存']);
  if (i === 2) return save();
  return i === 0;
}

async function loadText(f: TextFile) {
  let d: { text: string; encoding: string; label: string; bom: boolean };
  try { d = core.decode_text(f.bytes); } catch (e) { await message('開けませんでした', String(e)); return; }
  crlf = d.text.includes('\r\n');
  const previousDir = doc?.path ? dirname(doc.path) : null;
  doc = f;
  encoding = d.encoding; encodingLabel = d.label; bom = d.bom;
  resetDoc(view, d.text.replace(/\r\n?/g, '\n'), fontPx);
  setDirty(false);
  updateTitle();
  // 画像フォルダと設定：このテキストで前に使ったもの → 同じフォルダのテキストで使っていたもの → テキストと同じフォルダ
  if (platform.kind === 'desktop' && f.path) {
    const raw = store.get('doc:' + f.path);
    let saved: { images?: string | null; config?: string | null } = {};
    try { saved = JSON.parse(raw || '{}'); } catch { /* なし */ }
    let lost = '';
    if (raw === null && previousDir === dirname(f.path)) {
      // 覚えたものがなく、前のテキストと同じフォルダ：いま使っている画像と設定をそのまま使う
    } else {
      const imgs = saved.images && platform.imagesAt ? await platform.imagesAt(saved.images).catch(() => null)
        : await platform.imagesNear?.(f).catch(() => null);
      if (imgs) setImages(imgs);
      config = saved.config && platform.configAt ? await platform.configAt(saved.config).catch(() => null) : null;
      if (saved.config && !config) lost = saved.config;
    }
    if (lost) {
      // 読めなかっただけかもしれないので、覚えた場所は消さない
      await message('設定ファイルが読めません', `このテキストで使っていた設定ファイルが読めませんでした。\n${lost}\n\n` +
        'いまは設定なしで変換します。ファイルが戻れば、次に開いたときにまた使います。');
    } else {
      remember();
    }
  }
  updateSettingsBar();
  refreshPageStarts();
  problems = []; renderProblems();
  convert();
  view.focus();
}

async function openFile() {
  if (!(await confirmDiscard())) return;
  const f = await platform.openText();
  if (f) await loadText(f);
}

function encodeCurrent(): Uint8Array | null {
  let t = text();
  if (crlf) t = t.replace(/\n/g, '\r\n');
  try { return core.encode_text(t, encoding, bom); } catch (bad) {
    return null;
  }
}

async function save(as = false): Promise<boolean> {
  let bytes = encodeCurrent();
  if (!bytes) {
    const i = await message(`${encodingLabel} で保存できない文字があります`, 'UTF-8 で保存しますか？', ['キャンセル', 'UTF-8 で保存']);
    if (i !== 1) return false;
    encoding = 'utf-8'; encodingLabel = 'UTF-8'; bom = false;
    bytes = encodeCurrent()!;
  }
  const f = !as && doc && platform.canSaveInPlace(doc)
    ? await platform.saveText(doc, bytes)
    : await platform.saveTextAs(doc?.name ?? 'book.txt', bytes);
  if (!f) return false;
  doc = f;
  setDirty(false);
  updateTitle();
  remember();
  store.set('draft', null);
  setStatus(`保存しました：${f.name}`);
  return true;
}

async function newFile() {
  if (!(await confirmDiscard())) return;
  doc = null; encoding = 'utf-8'; encodingLabel = 'UTF-8'; bom = false; crlf = false;
  resetDoc(view, '', fontPx);
  setDirty(false); updateTitle(); refreshPageStarts();
  problems = []; renderProblems(); built = false; preview.setBook(null);
}

function setImages(set: ImageSet) {
  images = set;
  conv.clear_images();
  for (const [n, b] of set.files) conv.add_image(n, b);
  preview.setImages(set.files);
  updateSettingsBar();
}

async function pickImages() {
  const set = await platform.pickImages(doc);
  if (!set) return;
  setImages(set);
  remember();
  convert();
}

async function pickConfig() {
  const c = await platform.pickConfig(doc);
  if (!c) return;
  config = c;
  remember();
  updateSettingsBar();
  if (c.cssName && !c.css) {
    await message('CSS が見つかりません', `設定の css にある「${c.cssName}」が読めませんでした。` +
      (platform.kind === 'browser' ? '設定ファイルといっしょに選んでください。' : '設定ファイルの場所から見た位置で書いてください。'));
  }
  convert();
}

async function editConfigFile() {
  const stem = (doc?.name ?? '').replace(/\.[^.]*$/, '');
  let cur: ConfigFile = config ?? { name: (stem || 'book') + '.toml', text: '' };
  const isNew = !config;
  const result = await openConfigEditor({
    name: cur.name, path: cur.path, text: isNew ? null : cur.text, newTitle: stem,
    cssExists: (name) => platform.cssExists(cur, name),
    pickCss: async () => {
      const r = await platform.pickCss(cur);
      if (!r) return null;
      cur = r.config;
      return r.name;
    },
    save: async (t) => {
      const saved = await platform.saveConfig(cur, t);
      if (!saved) return null;
      cur = saved;
      config = saved;
      remember();
      updateSettingsBar();
      convert();
      return { name: saved.name, path: saved.path };
    },
  });
  if (result && config?.cssName && !config.css) {
    await message('CSS が見つかりません', `設定の css にある「${config.cssName}」が読めませんでした。` +
      (platform.kind === 'browser' ? '設定の画面の「選ぶ…」で CSS を選んでください。' : '設定ファイルの場所から見た位置で書いてください。'));
  }
}

// ================================================================ 注記を入れる

function replace(from: number, to: number, insert: string, cursor?: number) {
  view.dispatch({
    changes: { from, to, insert },
    selection: { anchor: from + (cursor ?? insert.length) },
    scrollIntoView: true,
    annotations: isolateHistory.of('full'),
    userEvent: 'input.annotation',
  });
  view.focus();
}

function selectedLines(): { from: number; to: number } {
  const d = view.state.doc;
  const sel = view.state.selection.main;
  const a = d.lineAt(sel.from);
  let b = d.lineAt(sel.to);
  if (sel.to > sel.from && sel.to === b.from && b.number > a.number) b = d.line(b.number - 1);
  return { from: a.from, to: b.to };
}

function insertLine(s: string) {
  const line = view.state.doc.lineAt(view.state.selection.main.head);
  if (line.text.trim() === '') replace(line.from, line.to, s);
  else replace(line.to, line.to, '\n' + s);
}

function addPrefix(s: string) {
  const r = selectedLines();
  const body = view.state.doc.sliceString(r.from, r.to);
  const out = body.split('\n').map((l) => s + l).join('\n');
  replace(r.from, r.to, out);
}

function wrapBlock(open: string, close: string) {
  const sel = view.state.selection.main;
  if (sel.empty) {
    const start = view.state.doc.lineAt(sel.head).from;
    const [t, cur] = core.block(open, close, '') as [string, number];
    replace(start, start, t, cur);
    return;
  }
  const r = selectedLines();
  const d = view.state.doc;
  const to = r.to < d.length ? r.to + 1 : r.to;      // 行末の改行も含めて囲む
  const [t, cur] = core.block(open, close, d.sliceString(r.from, to)) as [string, number];
  replace(r.from, to, t, cur);
}

function imageNames(): string[] { return images ? Array.from(images.files.keys()).sort() : []; }

async function pickImageName(title: string): Promise<string | null> {
  const names = imageNames();
  if (!names.length) {
    await message('画像がありません', '先に、上の欄で画像フォルダ（画像）を選んでください。');
    return null;
  }
  return choose(title, names, `画像フォルダ：${images!.label}`);
}

async function perform(a: Action) {
  const st = view.state;
  const sel = st.selection.main;
  const selected = st.doc.sliceString(sel.from, sel.to);
  switch (a.t) {
    case 'line': insertLine(a.s); break;
    case 'prefix': addPrefix(a.s); break;
    case 'decorate': {
      if (selected.includes('\n')) { await message('一行の中で選んでください', '文字の飾りは、行をまたいで付けられません。'); return; }
      const [t, cur] = core.decorate(a.name, selected) as [string, number];
      replace(sel.from, sel.to, t, cur);
      break;
    }
    case 'block': wrapBlock(a.open, a.close); break;
    case 'indentBlock': {
      const v = await askIndentBlock(); if (!v) return;
      const [open, close] = core.indent_block(v[0], v[1] ?? undefined) as [string, string];
      wrapBlock(open, close);
      break;
    }
    case 'indent': {
      const n = await askNumber('字下げ', '何字下げますか？', 2); if (n === null) return;
      addPrefix(`［＃${core.fullwidth(n)}字下げ］`);
      break;
    }
    case 'raise': {
      const n = await askNumber('地から字上げ', '地から何字上げますか？', 2); if (n === null) return;
      addPrefix(`［＃地から${core.fullwidth(n)}字上げ］`);
      break;
    }
    case 'ruby': {
      if (selected.includes('\n')) { await message('一行の中で選んでください', 'ルビを付ける文字は、一行の中で選んでください。'); return; }
      let base = selected;
      // すでにルビの付いた文字（見当《けんとう》・｜X《よみ》）を選んだときは、ルビを付け直す
      const had = /^｜?([^《》｜\n]+)《[^《》]*》$/.exec(base);
      if (had) base = had[1];
      else if (/[《》｜]/.test(base)) { await message('ルビを付けられません', '《》や｜を含む文字には、ルビを付けられません。ルビの付いた語なら、その語だけ（よみまで）を選んでください。'); return; }
      let reading: string | null;
      if (!base) {
        const r = await ask('ルビ', [{ label: '親文字' }, { label: 'よみ' }]);
        if (!r || !r[0] || !r[1]) return;
        [base, reading] = r;
      } else {
        reading = await askText('ルビ', `「${base}」のよみ`);
        if (!reading) return;
      }
      const before = Array.from(st.doc.sliceString(Math.max(0, sel.from - 2), sel.from));
      const prev = before.length && before[before.length - 1] !== '\n' ? before[before.length - 1] : '';
      replace(sel.from, sel.to, core.ruby(base, reading!, prev));
      break;
    }
    case 'periodical':
      if (selected.includes('\n')) { await message('一行の中で選んでください', '誌名は、行をまたいで付けられません。'); return; }
      replace(sel.from, sel.to, `《《${selected}》》`, 2 + selected.length); break;
    case 'gaiji': {
      const r = await ask('外字', [{ label: '字の説明', placeholder: '木＋世' },
                                  { label: 'Unicode の符号位置', placeholder: 'U+6318', hint: '分からなければ空のまま' }]);
      if (!r || !r[0]) return;
      replace(sel.from, sel.to, core.gaiji(r[0], r[1]));
      break;
    }
    case 'figure': {
      const img = await pickImageName('図版の画像を選ぶ'); if (!img) return;
      const cap = await askText('図', '説明（なくてもよい）', '', '「／」で改行'); if (cap === null) return;
      insertLine(core.figure(img, cap));
      break;
    }
    case 'opener': {
      const img = await pickImageName('章扉の画像を選ぶ'); if (!img) return;
      const t = await askText('章扉', '章の題名（目次に使う）', '', '「補遺／書誌」で階層。空なら直後の見出しから'); if (t === null) return;
      insertLine(core.opener(img, t));
      break;
    }
    case 'chapter': {
      const t = await askText('章', '章の題名（目次に使う）', '', '「補遺／書誌」で階層'); if (!t) return;
      insertLine(`［＃章：${t}］`);
      break;
    }
  }
}

// ================================================================ 画面の組み立て

function buildPalette() {
  const box = $('palette-list');
  for (const sec of PALETTE) {
    const det = document.createElement('details');
    det.open = !!sec.open;
    const sum = document.createElement('summary');
    sum.textContent = sec.title;
    det.appendChild(sum);
    for (const it of sec.items) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'pal';
      b.textContent = it.label;
      if (it.key) { const k = document.createElement('kbd'); k.textContent = isMac ? '⌘R' : 'Ctrl+R'; b.appendChild(k); }
      b.title = it.hint;
      b.addEventListener('mouseenter', () => { $('palette-hint').textContent = it.hint; });
      b.addEventListener('focus', () => { $('palette-hint').textContent = it.hint; });
      b.addEventListener('click', async () => { await perform(it.action); if (isNarrow()) showTab('edit'); });
      det.appendChild(b);
    }
    box.appendChild(det);
  }
}

const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
const isNarrow = () => window.matchMedia('(max-width: 820px)').matches;

function showTab(name: string) {
  document.body.dataset.tab = name;
  document.querySelectorAll<HTMLButtonElement>('#tabs button').forEach((b) => b.classList.toggle('on', b.dataset.tab === name));
  if (name === 'edit') view.requestMeasure();
}

function zoom(d: number) {
  fontPx = Math.max(11, Math.min(32, fontPx + d));
  store.set('fontPx', String(fontPx));
  view.dispatch({ effects: fontSize.reconfigure(fontTheme(fontPx)) });
}

function wire() {
  const on = (id: string, f: () => void) => $(id).addEventListener('click', f);
  on('btn-new', newFile);
  on('btn-open', openFile);
  on('btn-save', () => save());
  on('btn-check', () => { convert(); if (isNarrow() && problems.length) showTab('problems'); });
  on('btn-export', exportEpub);
  on('img-pick', pickImages);
  on('cfg-pick', pickConfig);
  on('cfg-edit', editConfigFile);
  on('cfg-clear', () => { config = null; remember(); updateSettingsBar(); convert(); });
  on('pv-next', () => preview.step(1));
  on('pv-prev', () => preview.step(-1));
  on('pv-refresh', () => convert());
  on('btn-find', () => { openSearchPanel(view); });
  const auto = $<HTMLInputElement>('pv-auto');
  auto.checked = autoPreview;
  auto.addEventListener('change', () => { autoPreview = auto.checked; store.set('autoPreview', autoPreview ? 'on' : 'off'); });
  document.querySelectorAll<HTMLButtonElement>('#tabs button').forEach((b) => b.addEventListener('click', () => showTab(b.dataset.tab!)));
  window.addEventListener('egword-open-external', (e) => platform.openExternal((e as CustomEvent).detail));

  // ⌘S などは、エディタの外にいても効くように
  window.addEventListener('keydown', (e) => {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (!mod) return;
    const k = e.key.toLowerCase();
    const run = (f: () => void) => { e.preventDefault(); f(); };
    if (k === 's' && e.shiftKey) run(() => save(true));
    else if (k === 's') run(() => save());
    else if (k === 'o') run(openFile);
    else if (k === 'k') run(() => convert());
    else if (k === 'e' && e.shiftKey) run(exportEpub);
    else if (k === 'p' && e.shiftKey) run(() => convert());
    else if (k === '=' || k === '+' || k === ';') run(() => zoom(1));
    else if (k === '-') run(() => zoom(-1));
    else if (e.altKey && k === 'arrowleft') run(() => preview.step(1));
    else if (e.altKey && k === 'arrowright') run(() => preview.step(-1));
  });
  window.addEventListener('beforeunload', (e) => { if (dirty && platform.kind === 'browser') { e.preventDefault(); e.returnValue = ''; } });
}

/** ブラウザ版：書きかけを保存しておき、開き直したときに戻せるようにする */
function saveDraft() {
  if (platform.kind !== 'browser') return;
  clearTimeout(draftTimer);
  draftTimer = window.setTimeout(() => {
    store.set('draft', JSON.stringify({ name: doc?.name ?? '新しいテキスト.txt', encoding, text: text(), crlf }));
  }, 1500);
}

async function restore() {
  if (platform.kind === 'desktop') {
    const given: string | null = await (window as any).__TAURI__.core.invoke('initial_file');
    const last = given ?? store.get('lastFile');
    if (last && platform.openTextAt) {
      const f = await platform.openTextAt(last);
      if (f) await loadText(f);
    }
    return;
  }
  const raw = store.get('draft');
  if (!raw) return;
  try {
    const d = JSON.parse(raw);
    const i = await message('書きかけのテキストがあります', `「${d.name}」の書きかけを開きますか？`, ['捨てる', '開く']);
    if (i === 1) {
      doc = { name: d.name, bytes: new Uint8Array() };
      encoding = d.encoding; encodingLabel = d.encoding === 'cp932' ? 'Shift_JIS' : 'UTF-8'; crlf = !!d.crlf;
      resetDoc(view, d.text, fontPx);
      setDirty(true); updateTitle(); refreshPageStarts(); convert();
    } else if (i === 0) {
      store.set('draft', null);
    }
  } catch { store.set('draft', null); }
}

async function main() {
  await init({ module_or_path: 'egword_wasm_bg.wasm' });
  conv = new core.Converter();
  document.body.dataset.platform = platform.kind;
  preview = new Preview($<HTMLIFrameElement>('preview'));
  preview.onNavigate = () => {
    $('pv-name').textContent = preview.pages.length ? `${preview.name}（${preview.index + 1}／${preview.pages.length}）` : 'プレビュー';
    $<HTMLButtonElement>('pv-next').disabled = preview.index + 1 >= preview.pages.length;
    $<HTMLButtonElement>('pv-prev').disabled = preview.index <= 0;
  };
  preview.setBook(null);
  const mod = 'Mod';
  view = createEditor($('editor'), {
    onChange() {
      setDirty(true);
      saveDraft();
      // 頁の区切りは、カーソルの動きで取り消されないよう、別のタイマーで作り直す
      clearTimeout(pageTimer);
      pageTimer = window.setTimeout(() => { refreshPageStarts(); updatePosition(); }, 300);
      if (autoPreview) scheduleConvert();
    },
    onCursor() {
      clearTimeout(posTimer);
      posTimer = window.setTimeout(updatePosition, 150);
    },
  }, [
    { key: `${mod}-r`, run: () => { perform({ t: 'ruby' }); return true; }, preventDefault: true },
  ]);
  resetDoc(view, '', fontPx);
  buildPalette();
  wire();
  updateTitle();
  updateSettingsBar();
  renderProblems();
  showTab('edit');
  $('version').textContent = `egword2epub ${core.version()}（${platform.kind === 'desktop' ? 'アプリ' : 'ブラウザ'}版）`;
  setStatus('テキストを開くか、ここに書き始めてください。注記は左の一覧から入れられます。');
  await restore();
  // デスクトップ版：窓を閉じる前に確かめる
  if (platform.kind === 'desktop') {
    const ev = (window as any).__TAURI__.event;
    ev.listen('close-requested', async () => {
      if (await confirmDiscard()) (window as any).__TAURI__.core.invoke('quit_app');
    });
  }
}

main().catch((e) => {
  document.body.innerHTML = `<p style="padding:2em;font:14px system-ui">起動できませんでした：${String(e)}</p>`;
});
