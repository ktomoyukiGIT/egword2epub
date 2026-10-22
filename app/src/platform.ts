// ファイルの読み書き。デスクトップ（Tauri）とブラウザの違いをここに閉じ込める

export interface TextFile {
  name: string;           // ファイル名
  path?: string;          // デスクトップ版：場所
  handle?: any;           // ブラウザ版：書き戻せるときの印（File System Access API）
  bytes: Uint8Array;
}

export interface ImageSet {
  label: string;                       // 表示用の名前（フォルダ名など）
  path?: string;                       // デスクトップ版：フォルダの場所
  files: Map<string, Uint8Array>;      // 画像の名前 → 中身
}

export interface ConfigFile {
  name: string;
  path?: string;
  handle?: any;
  text: string;
  css?: string;          // 設定の css を、設定ファイルの場所から読んだもの
  cssName?: string;
}

export interface Platform {
  kind: 'desktop' | 'browser';
  /** 開いたテキストを、同じ場所に書き戻せるか */
  canSaveInPlace(f: TextFile | null): boolean;
  openText(): Promise<TextFile | null>;
  openTextAt?(path: string): Promise<TextFile | null>;
  saveText(f: TextFile, bytes: Uint8Array): Promise<TextFile | null>;   // 書き戻せなければ「名前を付けて保存」
  saveTextAs(name: string, bytes: Uint8Array): Promise<TextFile | null>;
  pickImages(near?: TextFile | null): Promise<ImageSet | null>;
  imagesNear?(f: TextFile): Promise<ImageSet | null>;     // テキストと同じフォルダの画像（デスクトップ版）
  imagesAt?(path: string): Promise<ImageSet | null>;
  pickConfig(near?: TextFile | null): Promise<ConfigFile | null>;
  configAt?(path: string): Promise<ConfigFile | null>;
  reloadConfig(c: ConfigFile): Promise<ConfigFile>;
  saveConfig(c: ConfigFile, text: string): Promise<ConfigFile | null>;
  /** 設定に書いた CSS が見つかるか（分からなければ null） */
  cssExists(c: ConfigFile, name: string): Promise<boolean | null>;
  /** CSS を選ぶ。設定に書く名前と、それを覚えた設定 */
  pickCss(c: ConfigFile): Promise<{ name: string; config: ConfigFile } | null>;
  saveEpub(name: string, bytes: Uint8Array, near?: TextFile | null): Promise<string | null>;
  openExternal(url: string): void;
}

const IMAGE_EXT = /\.(jpe?g|png|gif|webp|svg)$/i;

function dirname(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(0, i) : '';
}
function basename(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
}
function join(dir: string, name: string): string {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}
function resolveRelative(base: string, rel: string): string {
  if (rel.startsWith('/') || /^[A-Za-z]:[\\/]/.test(rel)) return rel;
  const parts = base.split(/[\\/]/);
  for (const seg of rel.split(/[\\/]/)) {
    if (seg === '..') parts.pop(); else if (seg !== '.' && seg !== '') parts.push(seg);
  }
  return parts.join('/');
}
/** dir から見た target の位置（二つより上に出るときは、そのままの場所） */
function relativePath(dir: string, target: string): string {
  const a = dir.split(/[\\/]/).filter(Boolean);
  const b = target.split(/[\\/]/).filter(Boolean);
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  const up = a.length - i;
  if (up >= 2 || i === 0) return target.replace(/\\/g, '/');
  return [...Array(up).fill('..'), ...b.slice(i)].join('/');
}
const CSS_RE = /^\s*css\s*=\s*["']([^"']+)["']/m;

// ================================================================ デスクトップ版（Tauri）

function tauriInvoke(cmd: string, args?: any, options?: any): Promise<any> {
  return (window as any).__TAURI__.core.invoke(cmd, args, options);
}

async function readFile(path: string): Promise<Uint8Array> {
  const buf: ArrayBuffer = await tauriInvoke('read_file', { path });
  return new Uint8Array(buf);
}
async function writeFile(path: string, bytes: Uint8Array): Promise<void> {
  await tauriInvoke('write_file', bytes, { headers: { path: encodeURIComponent(path) } });
}

class DesktopPlatform implements Platform {
  kind = 'desktop' as const;
  canSaveInPlace(f: TextFile | null) { return !!f?.path; }

  async openText() {
    const path: string | null = await tauriInvoke('pick_file', { kind: 'text' });
    return path ? this.openTextAt(path) : null;
  }
  async openTextAt(path: string) {
    try { return { name: basename(path), path, bytes: await readFile(path) }; } catch { return null; }
  }
  async saveText(f: TextFile, bytes: Uint8Array) {
    if (!f.path) return this.saveTextAs(f.name, bytes);
    await writeFile(f.path, bytes);
    return { ...f, bytes };
  }
  async saveTextAs(name: string, bytes: Uint8Array) {
    const path: string | null = await tauriInvoke('save_dialog', { name, kind: 'text' });
    if (!path) return null;
    await writeFile(path, bytes);
    return { name: basename(path), path, bytes };
  }
  async imagesAt(dir: string) {
    const names: string[] = await tauriInvoke('list_dir', { path: dir });
    const files = new Map<string, Uint8Array>();
    for (const n of names.filter((n) => IMAGE_EXT.test(n))) files.set(n, await readFile(join(dir, n)));
    return { label: basename(dir) || dir, path: dir, files };
  }
  async imagesNear(f: TextFile) { return f.path ? this.imagesAt(dirname(f.path)) : null; }
  async pickImages() {
    const dir: string | null = await tauriInvoke('pick_folder', {});
    return dir ? this.imagesAt(dir) : null;
  }
  async configAt(path: string) {
    const text = new TextDecoder().decode(await readFile(path));
    return this.withCss({ name: basename(path), path, text });
  }
  private async withCss(c: ConfigFile): Promise<ConfigFile> {
    const m = CSS_RE.exec(c.text);
    if (m && c.path) {
      try {
        const p = resolveRelative(dirname(c.path), m[1]);
        return { ...c, css: new TextDecoder().decode(await readFile(p)), cssName: basename(p) };
      } catch { return { ...c, css: undefined, cssName: m[1] }; }
    }
    return { ...c, css: undefined, cssName: undefined };
  }
  async pickConfig() {
    const path: string | null = await tauriInvoke('pick_file', { kind: 'config' });
    return path ? this.configAt(path) : null;
  }
  async reloadConfig(c: ConfigFile) { return c.path ? this.configAt(c.path) : c; }
  async saveConfig(c: ConfigFile, text: string) {
    let path = c.path;
    if (!path) {
      path = await tauriInvoke('save_dialog', { name: c.name || 'book.toml', kind: 'config' });
      if (!path) return null;
    }
    await writeFile(path, new TextEncoder().encode(text));
    return this.configAt(path);
  }
  async cssExists(c: ConfigFile, name: string) {
    const abs = name.startsWith('/') || /^[A-Za-z]:[\\/]/.test(name);
    if (!abs && !c.path) return null;
    try { await readFile(abs ? name : resolveRelative(dirname(c.path!), name)); return true; } catch { return false; }
  }
  async pickCss(c: ConfigFile) {
    const path: string | null = await tauriInvoke('pick_file', { kind: 'css' });
    if (!path) return null;
    return { name: c.path ? relativePath(dirname(c.path), path) : path, config: c };
  }
  async saveEpub(name: string, bytes: Uint8Array) {
    const path: string | null = await tauriInvoke('save_dialog', { name, kind: 'epub' });
    if (!path) return null;
    await writeFile(path, bytes);
    return path;
  }
  openExternal(url: string) { tauriInvoke('open_external', { url }); }
}

// ================================================================ ブラウザ版

/** <input type="file"> でファイルを選ぶ（File System Access API のないブラウザ用） */
function pickWithInput(accept: string, multiple: boolean, directory = false): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.multiple = multiple;
    if (directory) (input as any).webkitdirectory = true;
    input.style.display = 'none';
    document.body.appendChild(input);
    input.addEventListener('change', () => { resolve(Array.from(input.files || [])); input.remove(); });
    input.addEventListener('cancel', () => { resolve([]); input.remove(); });
    input.click();
  });
}

function download(name: string, bytes: Uint8Array, type: string) {
  const blob = new Blob([bytes], { type });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
}

const hasFS = typeof (window as any).showOpenFilePicker === 'function';
const hasDirPicker = typeof (window as any).showDirectoryPicker === 'function';

class BrowserPlatform implements Platform {
  kind = 'browser' as const;
  canSaveInPlace(f: TextFile | null) { return !!f?.handle; }

  async openText() {
    if (hasFS) {
      try {
        const [h] = await (window as any).showOpenFilePicker({
          types: [{ description: 'テキスト', accept: { 'text/plain': ['.txt'] } }],
        });
        const file: File = await h.getFile();
        return { name: file.name, handle: h, bytes: new Uint8Array(await file.arrayBuffer()) };
      } catch { return null; }
    }
    const [file] = await pickWithInput('.txt,text/plain', false);
    return file ? { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) } : null;
  }

  async saveText(f: TextFile, bytes: Uint8Array) {
    if (f.handle) {
      try {
        const w = await f.handle.createWritable();
        await w.write(bytes);
        await w.close();
        return { ...f, bytes };
      } catch { /* 書き戻せなければ下へ */ }
    }
    return this.saveTextAs(f.name, bytes);
  }

  async saveTextAs(name: string, bytes: Uint8Array) {
    if (typeof (window as any).showSaveFilePicker === 'function') {
      try {
        const h = await (window as any).showSaveFilePicker({
          suggestedName: name, types: [{ description: 'テキスト', accept: { 'text/plain': ['.txt'] } }],
        });
        const w = await h.createWritable();
        await w.write(bytes);
        await w.close();
        return { name: h.name, handle: h, bytes };
      } catch { return null; }
    }
    download(name, bytes, 'text/plain');
    return { name, bytes };
  }

  async pickImages() {
    const files = new Map<string, Uint8Array>();
    if (hasDirPicker) {
      try {
        const dir = await (window as any).showDirectoryPicker();
        for await (const [name, h] of dir.entries()) {
          if (h.kind === 'file' && IMAGE_EXT.test(name)) {
            files.set(name, new Uint8Array(await (await h.getFile()).arrayBuffer()));
          }
        }
        return { label: dir.name, files };
      } catch { return null; }
    }
    // スマホなど：画像をまとめて選ぶ
    const picked = await pickWithInput('image/*', true);
    if (!picked.length) return null;
    for (const f of picked) files.set(f.name, new Uint8Array(await f.arrayBuffer()));
    return { label: `${picked.length} 点の画像`, files };
  }

  async pickConfig() {
    // 設定ファイルと、その中で指定した CSS をまとめて選ぶ
    const picked = await pickWithInput('.toml,.css,text/css', true);
    const toml = picked.find((f) => f.name.toLowerCase().endsWith('.toml'));
    if (!toml) return null;
    const c: ConfigFile = { name: toml.name, text: await toml.text() };
    const m = /^\s*css\s*=\s*["']([^"']+)["']/m.exec(c.text);
    if (m) {
      const want = basename(m[1]);
      const css = picked.find((f) => f.name === want);
      c.cssName = want;
      if (css) c.css = await css.text();
    }
    return c;
  }
  async reloadConfig(c: ConfigFile) { return c; }
  async saveConfig(c: ConfigFile, text: string) {
    download(c.name || 'book.toml', new TextEncoder().encode(text), 'text/plain');
    // 設定の css が変わったら、選んであった CSS の中身は使わない
    const m = CSS_RE.exec(text);
    const want = m ? basename(m[1]) : undefined;
    return { ...c, text, cssName: want, css: want && want === c.cssName ? c.css : undefined };
  }
  async cssExists(c: ConfigFile, name: string) { return basename(name) === c.cssName && c.css !== undefined; }
  async pickCss(c: ConfigFile) {
    const [f] = await pickWithInput('.css,text/css', false);
    if (!f) return null;
    return { name: f.name, config: { ...c, cssName: f.name, css: await f.text() } };
  }

  async saveEpub(name: string, bytes: Uint8Array) {
    if (typeof (window as any).showSaveFilePicker === 'function') {
      try {
        const h = await (window as any).showSaveFilePicker({
          suggestedName: name, types: [{ description: 'EPUB', accept: { 'application/epub+zip': ['.epub'] } }],
        });
        const w = await h.createWritable();
        await w.write(bytes);
        await w.close();
        return h.name;
      } catch { return null; }
    }
    download(name, bytes, 'application/epub+zip');
    return name;
  }

  openExternal(url: string) { window.open(url, '_blank', 'noopener'); }
}

export const platform: Platform = (window as any).__TAURI__ ? new DesktopPlatform() : new BrowserPlatform();
export { basename, dirname };
