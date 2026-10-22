"""Python 版のテストを走らせて、変換の入力と結果を記録する（Swift 版がそれと同じ結果を出すか確かめる）

    python3 tools/record_fixtures.py ../egword2epub core/tests/fixtures/fixtures.json
"""
import base64
import json
import os
import sys
import unittest
import zipfile

repo = os.path.abspath(sys.argv[1])
out_path = os.path.abspath(sys.argv[2])
sys.path.insert(0, repo)
os.chdir(repo)

import egword2epub.cli as cli
import egword2epub.blocks as blocks
from tests import helpers

records = []


def ctx_state(ctx):
    return {'warnings': [list(w) for w in ctx.warnings]}


# ---- 一行（helpers.run）----
orig_make_ctx = helpers.make_ctx


def make_ctx(conf=None, **kw):
    ctx = orig_make_ctx(conf, **kw)
    ctx._rec = {'conf': conf or {}, 'kw': {k: (sorted(v) if isinstance(v, (set, frozenset)) else v)
                                           for k, v in kw.items()}}
    return ctx


helpers.make_ctx = make_ctx
orig_run = helpers.run


def run(text, ctx=None, pos=0, mode='body', **kw):
    fresh = ctx is None
    if fresh:
        ctx = make_ctx(**kw)
    before = len(ctx.warnings)
    res = orig_run(text, ctx, pos, mode)
    if fresh and not ctx.notes_blocks:
        records.append({'type': 'inline', 'text': text, 'pos': pos, 'mode': mode, **ctx._rec,
                        'out': res, 'warnings': [list(w) for w in ctx.warnings[before:]]})
    return res


helpers.run = run

# ---- 一頁（blocks.lines_html）----
orig_lines_html = blocks.lines_html


def lines_html(ctx, lines, rp):
    rec = getattr(ctx, '_rec', None)
    fresh = rec is not None and not ctx.notes_blocks and not ctx.warnings and not ctx.headings
    res = orig_lines_html(ctx, lines, rp)
    if fresh and rec['kw'].get('imgdir', '.') == '.':
        records.append({'type': 'lines', 'lines': list(lines), 'pos': rp.pos, 'mode': rp.mode, **rec,
                        'out': res, 'warnings': [list(w) for w in ctx.warnings],
                        'headings': [list(h) for h in ctx.headings]})
    return res


blocks.lines_html = lines_html

# ---- 全体（cli.convert）----
orig_convert = cli.convert


def convert(src, imgdir, out, config=None, css=None, encoding=None):
    rec = {'type': 'convert', 'src_name': os.path.basename(src),
           'src': base64.b64encode(open(src, 'rb').read()).decode(),
           'config': open(config, encoding='utf-8').read() if config else None,
           'css': open(css, encoding='utf-8').read() if css else None,
           'encoding': encoding,
           'images': {n: base64.b64encode(open(os.path.join(imgdir, n), 'rb').read()).decode()
                      for n in sorted(os.listdir(imgdir))
                      if os.path.isfile(os.path.join(imgdir, n)) and n.lower().endswith(
                          ('.jpg', '.jpeg', '.png', '.gif', '.webp', '.svg', '.bmp', '.tif', '.tiff'))}}
    try:
        files, ctx = orig_convert(src, imgdir, out, config, css, encoding)
    except SystemExit as ex:
        rec['error'] = str(ex)
        records.append(rec)
        raise
    z = zipfile.ZipFile(out)
    rec['files'] = [[fn, x, k] for fn, x, k in files]
    rec['nav'] = z.read('OEBPS/nav.xhtml').decode()
    rec['opf'] = z.read('OEBPS/content.opf').decode()
    rec['css_out'] = z.read('OEBPS/style.css').decode()
    rec['zip_names'] = sorted(z.namelist())
    rec['warnings'] = [list(w) for w in ctx.warnings]
    rec['images_used'] = list(ctx.images)
    rec['cover'] = ctx.cover
    rec['src_pages'] = ctx.src_pages
    rec['encoding_out'] = ctx.encoding
    records.append(rec)
    return files, ctx


cli.convert = convert

suite = unittest.defaultTestLoader.discover('tests', top_level_dir='.')
# テストのモジュールは「from egword2epub.cli import convert」で読むので、読み込んだ後にも差し替える
for mod in list(sys.modules.values()):
    if getattr(mod, '__name__', '').startswith('tests.'):
        if getattr(mod, 'convert', None) is orig_convert:
            mod.convert = convert
        if getattr(mod, 'run', None) is orig_run:
            mod.run = run
        if getattr(mod, 'make_ctx', None) is orig_make_ctx:
            mod.make_ctx = make_ctx
        if getattr(mod, 'lines_html', None) is orig_lines_html:
            mod.lines_html = lines_html
result = unittest.TextTestRunner(verbosity=0).run(suite)
os.makedirs(os.path.dirname(out_path), exist_ok=True)
with open(out_path, 'w', encoding='utf-8') as f:
    json.dump(records, f, ensure_ascii=False)
kinds = {}
for r in records:
    kinds[r['type']] = kinds.get(r['type'], 0) + 1
print('記録しました：', kinds, '→', out_path)
