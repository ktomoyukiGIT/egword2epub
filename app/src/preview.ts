// 縦書きのプレビュー：変換した XHTML を、CSS と画像を差し込んで iframe に出す（ファイルには書かない）

export interface PreviewBook {
  files: { name: string; kind: string; xhtml: string }[];
  css: string;
}

const MIME: Record<string, string> = { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml' };

export class Preview {
  private book: PreviewBook | null = null;
  private urls = new Map<string, string>();        // 画像の名前 → blob の URL
  private current = '';
  index = 0;
  onNavigate: () => void = () => {};

  constructor(private frame: HTMLIFrameElement) {}

  /** 画像を差し替える（フォルダを選び直したとき） */
  setImages(images: Map<string, Uint8Array>) {
    for (const u of this.urls.values()) URL.revokeObjectURL(u);
    this.urls.clear();
    for (const [name, bytes] of images) {
      const ext = name.split('.').pop()!.toLowerCase();
      this.urls.set(name, URL.createObjectURL(new Blob([bytes], { type: MIME[ext] || 'application/octet-stream' })));
    }
  }

  get pages(): string[] { return this.book ? this.book.files.map((f) => f.name) : []; }
  get name(): string { return this.pages[this.index] ?? ''; }

  setBook(book: PreviewBook | null) {
    this.book = book;
    if (!book) { this.index = 0; this.showMessage('テキストを開くと、ここに縦書きのプレビューが出ます。'); return; }
    this.index = Math.min(this.index, book.files.length - 1);
    this.render(true);
  }

  show(name: string, anchor = '') {
    const i = this.pages.indexOf(name);
    if (i < 0) return;
    const same = i === this.index && this.current === name;
    this.index = i;
    if (!same) this.render(false, anchor); else if (anchor) this.scrollTo(anchor);
  }

  step(d: number) {
    const i = this.index + d;
    if (i >= 0 && i < this.pages.length) { this.index = i; this.render(false); }
  }

  showMessage(text: string) {
    this.current = '';
    this.frame.srcdoc = `<html><body style="font:13px system-ui,sans-serif;color:#888;display:flex;align-items:center;
      justify-content:center;height:90vh;margin:0;text-align:center;padding:0 2em">${text}</body></html>`;
  }

  private html(x: string): string {
    const css = this.book!.css;
    let h = x.replace(/<\?xml[^>]*\?>\s*/, '').replace(/<!DOCTYPE[^>]*>\s*/, '<!DOCTYPE html>\n');
    h = h.replace(/<link rel="stylesheet"[^>]*href="style\.css"[^>]*\/>/, `<style>${css}\n/* プレビュー */ body{margin:1.5em 1em;}</style>`);
    h = h.replace(/((?:src|xlink:href)=")images\/([^"]+)"/g, (_m, attr, name) => {
      const u = this.urls.get(name.replace(/&amp;/g, '&'));
      return u ? `${attr}${u}"` : `${attr}data:,"`;
    });
    return h;
  }

  /** keepScroll：同じ頁の作り直しなら、読んでいた位置を保つ */
  private render(keepScroll: boolean, anchor = '') {
    if (!this.book) return;
    const f = this.book.files[this.index];
    if (!f) return;
    let pos: [number, number] | null = null;
    if (keepScroll && f.name === this.current) {
      try { const w = this.frame.contentWindow!; pos = [w.scrollX, w.scrollY]; } catch { pos = null; }
    }
    this.current = f.name;
    this.frame.onload = () => {
      const doc = this.frame.contentDocument;
      if (!doc) return;
      if (pos) this.frame.contentWindow!.scrollTo(pos[0], pos[1]);
      else if (anchor) this.scrollTo(anchor);
      // 頁の中のリンク：ほかの頁ならプレビューの中で移り、外のリンクはブラウザで開く
      doc.addEventListener('click', (e) => {
        const a = (e.target as Element).closest?.('a');
        if (!a) return;
        const href = a.getAttribute('href') || '';
        e.preventDefault();
        if (/^(https?:|mailto:)/.test(href)) { window.dispatchEvent(new CustomEvent('egword-open-external', { detail: href })); return; }
        const [file, frag] = href.split('#');
        if (!file || file === this.current) { if (frag) this.scrollTo(frag); return; }
        this.show(file, frag || '');
        this.onNavigate();
      });
    };
    this.frame.srcdoc = this.html(f.xhtml);
    this.onNavigate();
  }

  private scrollTo(id: string) {
    const doc = this.frame.contentDocument;
    const t = doc?.getElementById(id);
    if (t) t.scrollIntoView({ block: 'center', inline: 'center' });
  }

  /** いまの頁を、単独の HTML として（ブラウザで開く用） */
  currentHtml(): string | null {
    if (!this.book) return null;
    const f = this.book.files[this.index];
    return f ? this.html(f.xhtml) : null;
  }
}
