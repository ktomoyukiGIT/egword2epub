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
    form.addEventListener('submit', (e) => { e.preventDefault(); finish(inputs.map((i) => i.value)); });
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

/** 数を尋ねる（全角の数字でもよい） */
export async function askNumber(title: string, label: string, value: number, min = 1): Promise<number | null> {
  while (true) {
    const s = await askText(title, label, String(value));
    if (s === null) return null;
    const n = parseInt(s.trim().replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)), 10);
    if (!isNaN(n) && n >= min && n <= 40) return n;
    await message('数を入れてください', `${min} から 40 までの数を入れてください。`);
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
