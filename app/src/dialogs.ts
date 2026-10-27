// 尋ねる窓（window.prompt はデスクトップ版で使えないことがあるので、自前の <dialog> で）

function el<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Record<string, string> = {}, text = ''): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (text) e.textContent = text;
  return e;
}

interface Field { label: string; value?: string; placeholder?: string; multiline?: boolean; hint?: string }

/** いくつかの欄を尋ねる。キャンセルなら null */
export function ask(title: string, fields: Field[], okLabel = 'OK'): Promise<string[] | null> {
  return new Promise((resolve) => {
    const d = el('dialog', { class: 'dlg' });
    const form = el('form', { method: 'dialog' });
    form.appendChild(el('h2', {}, title));
    const inputs: (HTMLInputElement | HTMLTextAreaElement)[] = [];
    for (const f of fields) {
      const lab = el('label', { class: 'field' });
      lab.appendChild(el('span', {}, f.label));
      const inp = f.multiline ? el('textarea', { rows: '4' }) : el('input', { type: 'text' });
      inp.value = f.value ?? '';
      if (f.placeholder) inp.placeholder = f.placeholder;
      lab.appendChild(inp);
      if (f.hint) lab.appendChild(el('small', {}, f.hint));
      inputs.push(inp);
      form.appendChild(lab);
    }
    const row = el('div', { class: 'buttons' });
    const cancel = el('button', { type: 'button', value: 'cancel' }, 'キャンセル');
    const ok = el('button', { type: 'submit', value: 'ok', class: 'primary' }, okLabel);
    row.append(cancel, ok);
    form.appendChild(row);
    d.appendChild(form);
    document.body.appendChild(d);
    let done = false;
    const finish = (v: string[] | null) => { if (done) return; done = true; d.close(); d.remove(); resolve(v); };
    cancel.addEventListener('click', () => finish(null));
    // 日本語入力の変換を確定する Enter で、窓が閉じてしまわないように（Safari・Mac のアプリでは、確定の直後に送信が来る）
    let composing = false;
    let compositionEnded = 0;
    for (const i of inputs) {
      i.addEventListener('compositionstart', () => { composing = true; });
      i.addEventListener('compositionend', () => { composing = false; compositionEnded = Date.now(); });
      i.addEventListener('keydown', (e) => {
        const ke = e as KeyboardEvent;
        if (ke.key === 'Enter' && (ke.isComposing || ke.keyCode === 229 || composing || Date.now() - compositionEnded < 120)) ke.preventDefault();
      });
    }
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      if (composing || Date.now() - compositionEnded < 120) return;
      finish(inputs.map((i) => i.value));
    });
    d.addEventListener('cancel', (e) => { e.preventDefault(); finish(null); });
    d.showModal();
    inputs[0]?.focus();
    (inputs[0] as HTMLInputElement)?.select?.();
  });
}

export async function askText(title: string, label: string, value = '', hint = ''): Promise<string | null> {
  const r = await ask(title, [{ label, value, hint }]);
  return r ? r[0] : null;
}

/** 全角の数字も読む。読めなければ NaN */
export function readNumber(s: string): number {
  const t = s.trim().replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0));
  return /^\d+$/.test(t) ? parseInt(t, 10) : NaN;
}

/** 数を尋ねる（全角の数字でもよい） */
export async function askNumber(title: string, label: string, value: number, min = 1): Promise<number | null> {
  while (true) {
    const s = await askText(title, label, String(value));
    if (s === null) return null;
    const n = readNumber(s);
    if (!isNaN(n) && n >= min && n <= 40) return n;
    await message('数を入れてください', `${min} から 40 までの数を入れてください。`);
  }
}

/** 範囲の字下げの数を、一つの窓で尋ねる。[一行目, 折り返し（空なら null）] */
export async function askIndentBlock(): Promise<[number, number | null] | null> {
  let first = '２', rest = '';
  while (true) {
    const r = await ask('範囲の字下げ', [
      { label: '何字下げますか？', value: first },
      { label: '折り返した行は何字下げますか？', value: rest, placeholder: '空なら書かない（上と同じ）', hint: '例：一行目は２字、折り返しは３字なら「２」と「３」。入れた数は、上と同じでもそのまま書きます。一行目を天付きにするなら上を「０」' },
    ]);
    if (!r) return null;
    [first, rest] = r;
    const n = readNumber(first);
    const empty = rest.trim() === '';
    const m = empty ? n : readNumber(rest);
    if (!isNaN(n) && !isNaN(m) && n >= 0 && m >= 0 && n <= 40 && m <= 40 && n + m > 0) return [n, empty ? null : m];
    await message('数を入れてください', '0 から 40 までの数を入れてください。');
  }
}

/** 一覧から選ぶ（画像など） */
export function choose(title: string, items: string[], note = ''): Promise<string | null> {
  return new Promise((resolve) => {
    const d = el('dialog', { class: 'dlg' });
    d.appendChild(el('h2', {}, title));
    if (note) d.appendChild(el('p', { class: 'note' }, note));
    const list = el('div', { class: 'choices' });
    let done = false;
    const finish = (v: string | null) => { if (done) return; done = true; d.close(); d.remove(); resolve(v); };
    for (const it of items) {
      const b = el('button', { type: 'button' }, it);
      b.addEventListener('click', () => finish(it));
      list.appendChild(b);
    }
    d.appendChild(list);
    const row = el('div', { class: 'buttons' });
    const cancel = el('button', { type: 'button' }, 'キャンセル');
    cancel.addEventListener('click', () => finish(null));
    row.appendChild(cancel);
    d.appendChild(row);
    d.addEventListener('cancel', (e) => { e.preventDefault(); finish(null); });
    document.body.appendChild(d);
    d.showModal();
  });
}

/** 知らせる。buttons の何番目を押したかを返す（Esc は -1） */
export function message(title: string, text: string, buttons: string[] = ['OK']): Promise<number> {
  return new Promise((resolve) => {
    const d = el('dialog', { class: 'dlg' });
    d.appendChild(el('h2', {}, title));
    const p = el('p', { class: 'msg' });
    p.textContent = text;
    d.appendChild(p);
    const row = el('div', { class: 'buttons' });
    let done = false;
    const finish = (i: number) => { if (done) return; done = true; d.close(); d.remove(); resolve(i); };
    buttons.forEach((b, i) => {
      const btn = el('button', { type: 'button', class: i === buttons.length - 1 ? 'primary' : '' }, b);
      btn.addEventListener('click', () => finish(i));
      row.appendChild(btn);
    });
    d.appendChild(row);
    d.addEventListener('cancel', (e) => { e.preventDefault(); finish(-1); });
    document.body.appendChild(d);
    d.showModal();
    (row.lastChild as HTMLButtonElement)?.focus();
  });
}
