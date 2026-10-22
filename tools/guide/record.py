"""使い方の動画を撮る（ブラウザ版を Playwright で操作する）

    python3 record.py <dist の URL> <見本のフォルダ> <出力フォルダ>
"""
import os
import shutil
import sys
import time

from playwright.sync_api import sync_playwright

URL, SAMPLE, OUT = sys.argv[1], os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3])
W, H = 1280, 760
os.makedirs(OUT, exist_ok=True)

OVERLAY = r"""
(() => {
  const css = `
  #rec-ov{position:fixed;inset:0;width:100vw;height:100vh;max-width:none;max-height:none;margin:0;padding:0;border:0;
    background:transparent;pointer-events:none;overflow:visible}
  #rec-cur{position:absolute;left:-50px;top:-50px;width:22px;height:30px;transition:none;filter:drop-shadow(0 1px 2px rgba(0,0,0,.45))}
  #rec-cap{position:absolute;left:50%;bottom:26px;transform:translateX(-50%);max-width:86%;padding:10px 22px;border-radius:10px;
    background:rgba(20,26,34,.86);color:#fff;font:600 21px/1.45 "Noto Sans CJK JP","Hiragino Sans",sans-serif;text-align:center;
    opacity:0;transition:opacity .25s}
  #rec-cap.on{opacity:1}
  .rec-ring{position:absolute;width:34px;height:34px;margin:-17px 0 0 -17px;border-radius:50%;border:3px solid #f5a623;
    animation:recring .45s ease-out forwards}
  @keyframes recring{from{transform:scale(.3);opacity:1}to{transform:scale(1.4);opacity:0}}`;
  function setup() {
    if (document.getElementById('rec-ov')) return;
    const st = document.createElement('style'); st.textContent = css; document.head.appendChild(st);
    const ov = document.createElement('div'); ov.id = 'rec-ov'; ov.setAttribute('popover', 'manual');
    ov.innerHTML = '<svg id="rec-cur" viewBox="0 0 22 30"><path d="M2 2 L2 24 L8 18.5 L12 27.5 L15.5 26 L11.5 17 L19.5 17 Z" fill="#fff" stroke="#111" stroke-width="1.6" stroke-linejoin="round"/></svg><div id="rec-cap"></div>';
    document.body.appendChild(ov);
    ov.showPopover();
    const cur = ov.querySelector('#rec-cur');
    window.__recRaise = () => { try { ov.hidePopover(); ov.showPopover(); } catch (e) {} };
    window.__recMove = (x, y) => { cur.style.left = x + 'px'; cur.style.top = y + 'px'; };
    window.__recClick = (x, y) => {
      const r = document.createElement('div'); r.className = 'rec-ring'; r.style.left = x + 'px'; r.style.top = y + 'px';
      ov.appendChild(r); setTimeout(() => r.remove(), 600);
    };
    window.__recCaption = (t) => {
      const c = ov.querySelector('#rec-cap');
      if (!t) { c.classList.remove('on'); return; }
      c.textContent = t; c.classList.add('on');
    };
    document.addEventListener('mousemove', (e) => window.__recMove(e.clientX, e.clientY), true);
    const orig = HTMLDialogElement.prototype.showModal;
    HTMLDialogElement.prototype.showModal = function () { orig.call(this); window.__recRaise(); };
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', setup); else setup();
})();
"""


class Rec:
    def __init__(self, browser, name):
        self.name = name
        self.ctx = browser.new_context(viewport={'width': W, 'height': H}, record_video_dir=OUT + '/raw',
                                       record_video_size={'width': W, 'height': H}, accept_downloads=True,
                                       device_scale_factor=1)
        self.ctx.add_init_script("delete window.showOpenFilePicker; delete window.showSaveFilePicker; delete window.showDirectoryPicker;")
        self.ctx.add_init_script(OVERLAY)
        self.pg = self.ctx.new_page()
        self.x, self.y = W * 0.55, H * 0.6
        self.pg.goto(URL)
        self.pg.wait_for_selector('#version:not(:empty)')
        self.pg.evaluate(f'window.__recMove({self.x}, {self.y})')

    def cap(self, text, wait=0.0):
        self.pg.evaluate('(t) => window.__recCaption(t)', text)
        if wait:
            time.sleep(wait)

    def move(self, x, y, dur=0.55):
        steps = max(8, int(dur * 40))
        x0, y0 = self.x, self.y
        for i in range(1, steps + 1):
            t = i / steps
            t = t * t * (3 - 2 * t)
            self.pg.mouse.move(x0 + (x - x0) * t, y0 + (y - y0) * t)
            self.pg.evaluate(f'window.__recMove({x0 + (x - x0) * t}, {y0 + (y - y0) * t})')
            time.sleep(dur / steps)
        self.x, self.y = x, y

    def to(self, selector, dur=0.55):
        loc = self.pg.locator(selector).first
        loc.scroll_into_view_if_needed()
        b = loc.bounding_box()
        self.move(b['x'] + b['width'] / 2, b['y'] + b['height'] / 2, dur)
        return loc

    def click(self, selector, dur=0.55, pause=0.35):
        self.to(selector, dur)
        self.pg.evaluate(f'window.__recClick({self.x}, {self.y})')
        time.sleep(0.12)
        self.pg.mouse.click(self.x, self.y)
        time.sleep(pause)

    def choose_files(self, selector, files, dur=0.55):
        with self.pg.expect_file_chooser() as fc:
            self.click(selector, dur, pause=0)
        fc.value.set_files(files)
        time.sleep(0.8)

    def type(self, text, delay=0.09):
        for ch in text:
            self.pg.keyboard.insert_text(ch) if ord(ch) > 127 else self.pg.keyboard.type(ch)
            time.sleep(delay)

    def caret(self, needle, off=0, length=0, nth=0):
        """エディタで、needle の off 字目から length 字を選ぶ（0 ならカーソルを置く）。見た目のカーソルも動かす"""
        pos = self.pg.evaluate("""([needle, nth]) => {
          const v = document.querySelector('.cm-content').cmView.view;
          const s = v.state.doc.toString(); let i = -1;
          for (let k = 0; k <= nth; k++) i = s.indexOf(needle, i + 1);
          return i; }""", [needle, nth])
        if pos < 0:
            doc = self.pg.evaluate("() => document.querySelector('.cm-content').cmView.view.state.doc.toString()")
            raise RuntimeError(f'not found {needle!r} in {doc[:300]!r}')
        a, b = pos + off, pos + off + length
        self.pg.evaluate("""(a) => { const v = document.querySelector('.cm-content').cmView.view;
          const r = v.coordsAtPos(a), box = v.scrollDOM.getBoundingClientRect();
          if (!r || r.top < box.top + 30 || r.bottom > box.bottom - 30) {
            v.dispatch({ selection: { anchor: a }, scrollIntoView: true });
            const r2 = v.coordsAtPos(a); if (r2) v.scrollDOM.scrollTop += r2.top - box.top - box.height * 0.35;
          } }""", a)
        time.sleep(0.35)
        x0, top, bottom, x1 = self.pg.evaluate("""([a, b]) => { const v = document.querySelector('.cm-content').cmView.view;
          const p = v.coordsAtPos(a), q = v.coordsAtPos(b, -1) || p; return [p.left, p.top, p.bottom, q.right]; }""", [a, b])
        ymid = (top + bottom) / 2
        self.move(x0, ymid, 0.6)
        self.pg.evaluate(f'window.__recClick({x0}, {ymid})')
        if length:
            steps = 10
            for i in range(1, steps + 1):
                x = x0 + (x1 - x0) * i / steps
                self.pg.evaluate(f'window.__recMove({x}, {ymid})')
                time.sleep(0.03)
            self.x = x1
        self.pg.evaluate("""([a, b]) => { const v = document.querySelector('.cm-content').cmView.view;
          v.dispatch({ selection: { anchor: a, head: b } }); v.focus(); }""", [a, b])
        time.sleep(0.4)

    def editor_select(self, needle, nth=0):
        self.caret(needle, 0, len(needle), nth)

    def editor_cursor(self, needle, nth=0, off=0):
        self.caret(needle, off, 0, nth)

    def finish(self):
        time.sleep(0.6)
        video = self.pg.video
        self.ctx.close()
        dst = f'{OUT}/{self.name}.webm'
        shutil.move(video.path(), dst)
        print('video', dst)


def open_sample(r, captions=True, name='neko.txt'):
    if captions:
        r.cap('「開く」で、egword から書き出したテキストを開きます', 0.6)
    r.choose_files('#btn-open', [SAMPLE + '/' + name])
    time.sleep(1.4)
    if captions:
        r.cap('「画像」の「選ぶ…」で、表紙や挿絵の画像を選びます', 0.6)
    r.choose_files('#img-pick', [SAMPLE + '/cover.jpg', SAMPLE + '/ch01.jpg'])
    time.sleep(0.8)


def scene_open(b):
    r = Rec(b, '01-open')
    time.sleep(0.6)
    open_sample(r)
    r.cap('右に縦書きのプレビュー。カーソルのある頁を表示します', 0.4)
    r.editor_cursor('ようやくの思いで')
    time.sleep(2.0)
    r.cap('‹ › で頁を送れます', 0.3)
    r.click('#pv-next', pause=1.2)
    r.click('#pv-next', pause=1.2)
    r.cap('注の番号を押すと、注へ移ります', 0.3)
    r.editor_cursor('吾輩《わがはい》')
    time.sleep(2.0)
    fr = r.pg.frame_locator('#preview')
    a = fr.locator('a[href*="#"]').filter(has_text='1').first
    try:
        bb = r.pg.locator('#preview').bounding_box()
        ab = a.bounding_box()
        r.move(ab['x'] + ab['width'] / 2, ab['y'] + ab['height'] / 2, 0.8)
        r.pg.evaluate(f'window.__recClick({r.x}, {r.y})')
        a.click()
        time.sleep(2.0)
    except Exception as e:
        print('note link:', e)
    r.cap('')
    r.finish()


def scene_annotate(b):
    r = Rec(b, '02-annotate')
    open_sample(r, captions=False)
    r.pg.evaluate("document.querySelector('.cm-scroller').scrollTop = 0")
    r.cap('表紙：テキストの頭で「図…」→ 画像を選び、「改ページ」', 0.5)
    r.editor_cursor('吾輩は猫である\n')
    r.pg.keyboard.press('Enter')
    r.pg.keyboard.press('ArrowUp')
    time.sleep(0.4)
    r.click('summary:has-text("図版・区切り")', pause=0.3)
    r.click('button.pal:has-text("図…")')
    r.pg.wait_for_selector('dialog.dlg .choices button')
    r.click('dialog.dlg .choices button:has-text("cover.jpg")', pause=0.4)
    r.pg.wait_for_selector('dialog.dlg input')
    r.click('dialog.dlg button.primary', pause=0.5)
    r.click('button.pal:has-text("改ページ")', pause=1.6)
    time.sleep(1.2)
    r.cap('章の始まり：「章扉…」で画像と題名を入れます', 0.5)
    r.editor_cursor('\x0c\n\n一', off=2)
    r.click('button.pal:has-text("章扉…")')
    r.pg.wait_for_selector('dialog.dlg .choices button')
    r.click('dialog.dlg .choices button:has-text("ch01.jpg")', pause=0.4)
    r.pg.wait_for_selector('dialog.dlg input')
    r.to('dialog.dlg input')
    r.type('一')
    time.sleep(0.4)
    r.click('dialog.dlg button.primary', pause=2.4)
    r.cap('見出し：行を選んで「中見出し」', 0.4)
    r.caret('］\n一\n', 2, 1)
    r.click('summary:has-text("見出し")', pause=0.3)
    r.click('button.pal:has-text("中見出し")', pause=2.2)
    r.cap('ルビ：文字を選んで「ルビ…」（⌘R）。よみを入れるだけ', 0.4)
    r.editor_select('書生', 0)
    r.click('button.pal:has-text("ルビ…")')
    r.pg.wait_for_selector('dialog.dlg input')
    r.to('dialog.dlg input')
    r.type('しょせい')
    time.sleep(0.3)
    r.click('dialog.dlg button.primary', pause=0.8)
    r.cap('傍点：文字を選んで「傍点」', 0.4)
    r.editor_select('名前はまだ無い')
    r.click('button.pal:has-text("傍点")', pause=1.6)
    r.cap('本文の頁へ送ると、ルビと傍点が付いています', 0.3)
    r.click('#pv-next', pause=2.8)
    r.cap('入れた注記は ⌘Z で一回ずつ取り消せます', 1.8)
    r.cap('')
    r.finish()


def scene_problems(b):
    r = Rec(b, '03-problems')
    open_sample(r, captions=False, name='neko-done.txt')
    r.cap('書き損じがあると、下の「問題」に並びます', 0.4)
    r.editor_cursor('　ようやくの思いで')
    r.pg.keyboard.press('Enter')
    r.pg.keyboard.press('ArrowUp')
    r.type('［＃図（fig_01.jpg）入る］', 0.06)
    time.sleep(0.4)
    r.cap('例：まだない画像を図にしてしまった', 0.3)
    r.click('#btn-check', pause=2.0)
    r.cap('行を押すと、その場所へ移ります', 0.4)
    r.pg.evaluate("document.querySelector('.cm-scroller').scrollTop = 0")
    time.sleep(0.4)
    r.click('#problems-body tr', pause=1.8)
    r.cap('直して「調べる」（⌘K）。問題がなくなりました', 0.3)
    r.pg.keyboard.press('Home')
    r.pg.keyboard.press('Shift+End')
    time.sleep(0.5)
    r.pg.keyboard.press('Backspace')
    r.pg.keyboard.press('Backspace')
    time.sleep(0.4)
    r.click('#btn-check', pause=2.4)
    r.cap('')
    r.finish()


def scene_config(b):
    r = Rec(b, '04-config')
    open_sample(r, captions=False, name='neko-done.txt')
    r.cap('書名や作者は、設定の「新規…」で作ります', 0.4)
    r.click('#cfg-edit')
    r.pg.wait_for_selector('dialog.cfg')
    time.sleep(0.5)
    inp = 'dialog.cfg .cfg-page:not([hidden]) input'
    r.to(inp)
    r.pg.fill(inp + ' >> nth=0', '')
    r.type('吾輩は猫である')
    r.click(inp + ' >> nth=1', pause=0.2)
    r.type('わがはいはねこである')
    r.click('.cfg-creator input[aria-label="名前"]', pause=0.2)
    r.type('夏目漱石')
    r.click('.cfg-creator input[aria-label="よみ"]', pause=0.2)
    r.type('なつめ そうせき')
    time.sleep(0.4)
    r.cap('「組版と注」で、縦書き・横書きや注の書式を選べます', 0.3)
    r.click('.cfg-tabs button:has-text("組版と注")', pause=1.6)
    r.cap('「奥付」を付けると、最後に奥付の頁ができます', 0.3)
    r.click('.cfg-tabs button:has-text("奥付")', pause=0.4)
    r.click('dialog.cfg .cfg-page:not([hidden]) input[type=checkbox]', pause=0.3)
    r.click('dialog.cfg .cfg-page:not([hidden]) textarea', pause=0.2)
    r.type('底本：夏目漱石「吾輩は猫である」（青空文庫）', 0.05)
    time.sleep(0.4)
    r.cap('「テキスト」では設定ファイルそのものが見えます', 0.3)
    r.click('.cfg-tabs button:has-text("テキスト")', pause=1.8)
    r.cap('「確かめる」で書き損じを調べ、「保存」', 0.3)
    r.click('dialog.cfg button:has-text("確かめる")', pause=1.0)
    with r.pg.expect_download():
        r.click('dialog.cfg button.primary', pause=0.2)
    time.sleep(1.0)
    r.click('dialog.cfg button:has-text("閉じる")', pause=1.6)
    r.cap('')
    r.finish()


def scene_export(b):
    r = Rec(b, '05-export')
    open_sample(r, captions=False, name='neko-done.txt')
    r.cap('できたら「EPUB を書き出す」', 0.5)
    with r.pg.expect_download() as dl:
        r.click('#btn-export', pause=0.2)
    dl.value.save_as(OUT + '/neko.epub')
    time.sleep(1.4)
    r.cap('ルビ・傍点・注・図版の入った縦書きの EPUB ができます', 2.2)
    if r.pg.locator('dialog.dlg').count():
        r.click('dialog.dlg button.primary', pause=0.8)
    r.cap('')
    r.finish()


SCENES = {'open': scene_open, 'annotate': scene_annotate, 'problems': scene_problems,
          'config': scene_config, 'export': scene_export}

with sync_playwright() as p:
    b = p.chromium.launch()
    for name in (sys.argv[4:] or SCENES):
        SCENES[name](b)
    b.close()
