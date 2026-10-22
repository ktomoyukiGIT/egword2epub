"""Python 版の設定エディタの中身（conftoml）で、欄の値・書き出し・確認の結果を記録する
（Rust 版・Swift 版の設定の画面が同じ結果を出すか確かめる）

    python3 tools/record_config_fixtures.py ../egword2epub core/tests/fixtures/config.json
"""
import copy
import json
import os
import sys
import tomllib

sys.path.insert(0, os.path.abspath(sys.argv[1]))
from egword2epub.conftoml import check_config, dump_toml, form_from_config, form_to_config  # noqa: E402

CASES = {
    'empty': '',
    'minimal': '[book]\ntitle = "短い本"\n',
    'full': '''# 書名などの設定
writing_mode = "vertical"
css = "book.css"
autolink = ["url", "mail", "x"]
guess_periodicals = true
chapter_image = 'ch(\\d+)\\.jpg'

[book]
title = "書名「引用」\\\\と\\"記号\\""
title_file_as = "しょめい"
identifier = "urn:uuid:12345678-1234-1234-1234-123456789abc"
language = "ja"
publisher = "出版社"
date = 2026-10-19
description = """二行の
説明"""

[[book.creators]]
name = "ヘンリー・サヴェージ"
role = "aut"
file_as = "サヴェージ, ヘンリー"

[[book.creators]]
name = "訳者"
role = "trl"
seq = 2

[notes.footnote]
format = "※1"
label = "原注"
heading_lines = ["※原注", "＊原注"]

[notes.endnote]
format = "[1]"
label = "訳注"
heading_lines = "＊訳注"

[colophon]
title = "奥付"
lines = ["底本：……", "", "二行目"]
''',
    'odd': '''squeeze_blank_lines = true
cover_image = "表紙.jpg"
autolink = []
"odd key" = { "x y" = 1.5, d = 2026-01-02, inline = [{ a = 1 }, { b = 2 }] }
bogus = 1
[book]
creators = ["名だけ"]
language = "en"
[notes.footnote]
format = "none"
[notes.endnote]
format = "※1"
''',
    'problems': '''writing_mode = "tate"
autolink = ["web"]
css = "nothing.css"
[book]
date = "2026年"
[[book.creators]]
name = " "
[notes.footnote]
format = "[1]"
''',
    'horizontal': 'writing_mode = "horizontal"\n[notes.endnote]\nformat = "(1)"\n[colophon]\n',
}

EDITS = [
    ('nothing', {}),
    ('book fields', {'title': ' 新しい書名 ', 'publisher': '', 'date': '2026-10-21', 'language': 'ja',
                     'description': '説明　'}),
    ('creators', {'creators': [{'name': '著者', 'role': 'aut', 'file_as': 'ちょしゃ', 'src': None},
                               {'name': '', 'role': 'trl', 'file_as': '', 'src': None},
                               {'name': '絵', 'role': 'ill', 'file_as': '', 'src': 1}]}),
    ('layout', {'writing_mode': 'horizontal', 'css': ' style.css ', 'autolink': ['mail', 'url'],
                'guess_periodicals': False, 'squeeze_blank_lines': True, 'chapter_image': '', 'cover_image': 'c.png'}),
    ('notes', {'footnote': {'format': 'none', 'label': '脚注', 'heading_lines': ''},
               'endnote': {'format': '注1', 'label': ' 後注 ', 'heading_lines': '＊訳注、注, 後注　他'}}),
    ('colophon on', {'colophon': True, 'colophon_title': '', 'colophon_date_label': '初版',
                     'colophon_lines': '底本：あ\n\n三行目\n\n'}),
    ('colophon off', {'colophon': False}),
]


def main():
    out = []
    for name, text in CASES.items():
        data = tomllib.loads(text)
        form = form_from_config(data)
        rec = {'name': name, 'text': text, 'form': form,
               'problems': check_config(data, css_exists=lambda n: n == 'book.css'),
               'edits': []}
        for ename, change in EDITS:
            f = copy.deepcopy(form)
            f.update(copy.deepcopy(change))
            d = form_to_config(data, f)
            rec['edits'].append({'name': ename, 'change': change, 'dump': dump_toml(d),
                                 'problems': check_config(d, css_exists=lambda n: n == 'book.css')})
        out.append(rec)
    with open(sys.argv[2], 'w', encoding='utf-8') as fp:
        json.dump(out, fp, ensure_ascii=False, indent=1)
    print(len(out), 'cases')


main()
