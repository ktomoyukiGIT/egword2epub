import time, sys
# 使い方：python3 mobile.py <dist の URL> <見本のフォルダ> <出力フォルダ>
URL, S, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
from playwright.sync_api import sync_playwright
from PIL import Image
with sync_playwright() as p:
    b=p.chromium.launch()
    pg=b.new_page(viewport={'width':390,'height':780}, device_scale_factor=2, is_mobile=True, has_touch=True)
    pg.add_init_script("delete window.showOpenFilePicker; delete window.showSaveFilePicker; delete window.showDirectoryPicker;")
    pg.goto(URL); pg.wait_for_selector('#version:not(:empty)')
    with pg.expect_file_chooser() as fc: pg.click('#btn-open')
    fc.value.set_files(S+'/neko-done.txt'); time.sleep(1.5)
    with pg.expect_file_chooser() as fc: pg.click('#img-pick')
    fc.value.set_files([S+'/cover.jpg',S+'/ch01.jpg']); time.sleep(1.5)
    pg.evaluate("()=>{const v=document.querySelector('.cm-content').cmView.view; v.dispatch({selection:{anchor:v.state.doc.toString().indexOf('　吾輩《')}, scrollIntoView:true});}")
    time.sleep(1.5)
    pg.screenshot(path=f'{OUT}/m1.png')
    pg.click('#tabs button[data-tab="preview"]'); time.sleep(0.6)
    pg.click('#pv-next'); time.sleep(1.5)
    pg.screenshot(path=f'{OUT}/m2.png')
    pg.click('#tabs button[data-tab="palette"]'); time.sleep(0.6)
    pg.screenshot(path=f'{OUT}/m3.png')
    b.close()
ims=[Image.open(ff'{OUT}/m{i}.png') for i in (3,1,2)]
w,h=ims[0].size; g=40
c=Image.new('RGB',(w*3+g*2,h),(255,255,255))
for i,im in enumerate(ims): c.paste(im,(i*(w+g),0))
c=c.resize((c.width//2,c.height//2), Image.LANCZOS)
c.save(f'{OUT}/06-mobile.png', optimize=True); print(c.size)
