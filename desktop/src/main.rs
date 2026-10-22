//! Egword2EPUB のデスクトップ版：画面（app/dist）を窓に出し、ファイルの読み書きと選ぶ窓だけを受け持つ。
//! 変換は画面の中の WebAssembly で行うので、ブラウザ版と同じ結果になる。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

fn path_string(p: Option<FilePath>) -> Option<String> {
    p.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
}

/// ファイルを選ぶ（kind：text / config）
#[tauri::command]
async fn pick_file(app: tauri::AppHandle, kind: String) -> Option<String> {
    let d = app.dialog().file();
    let d = match kind.as_str() {
        "text" => d.add_filter("テキスト", &["txt"]).add_filter("すべて", &["*"]),
        "config" => d.add_filter("設定ファイル", &["toml"]),
        "css" => d.add_filter("CSS", &["css"]).add_filter("すべて", &["*"]),
        _ => d,
    };
    path_string(d.blocking_pick_file())
}

/// フォルダを選ぶ
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    path_string(app.dialog().file().blocking_pick_folder())
}

/// 保存先を尋ねる（kind：text / config / epub）
#[tauri::command]
async fn save_dialog(app: tauri::AppHandle, name: String, kind: String) -> Option<String> {
    let d = app.dialog().file().set_file_name(&name);
    let d = match kind.as_str() {
        "text" => d.add_filter("テキスト", &["txt"]),
        "config" => d.add_filter("設定ファイル", &["toml"]),
        "epub" => d.add_filter("EPUB", &["epub"]),
        _ => d,
    };
    path_string(d.blocking_save_file())
}

/// ファイルを読む（バイト列のまま返す）
#[tauri::command]
fn read_file(path: String) -> Result<tauri::ipc::Response, String> {
    std::fs::read(&path).map(tauri::ipc::Response::new).map_err(|e| format!("{}：{}", path, e))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// ファイルに書く（中身は本体で、場所は path の見出しで受け取る）
#[tauri::command]
fn write_file(request: tauri::ipc::Request) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else {
        return Err("書き込む中身がありません".into());
    };
    let path = request.headers().get("path").and_then(|v| v.to_str().ok()).map(percent_decode)
        .ok_or("書き込む場所がありません")?;
    // いったん別の名前で書いてから置き換える（途中で失敗しても元のファイルが残る）
    let tmp = format!("{}.egword-tmp", path);
    std::fs::write(&tmp, data).map_err(|e| format!("{}：{}", path, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("{}：{}", path, e) })
}

/// フォルダの中のファイル名
#[tauri::command]
fn list_dir(path: String) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = std::fs::read_dir(&path).map_err(|e| format!("{}：{}", path, e))?
        .filter_map(|e| e.ok())
        .filter(|e| std::fs::metadata(e.path()).map(|m| m.is_file()).unwrap_or(false))   // リンクの先も見る
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

/// 外のリンクを、ふだん使うブラウザなどで開く
#[tauri::command]
fn open_external(url: String) {
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")) { return; }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}

/// 起動のときに渡されたテキスト（Egword2EPUB 本文.txt）
#[tauri::command]
fn initial_file() -> Option<String> {
    std::env::args().skip(1).find(|a| !a.starts_with('-')).and_then(|a| {
        std::fs::canonicalize(&a).ok().map(|p| p.to_string_lossy().into_owned())
    })
}

/// 画面の側で保存の確認が済んだら呼ばれる（窓の大きさと位置を覚えてから終わる）
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    let _ = app.save_window_state(StateFlags::all());
    app.exit(0);
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // 窓の大きさと位置を覚えて、次に起動したときに戻す
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![pick_file, pick_folder, save_dialog, read_file, write_file, list_dir,
                                                 open_external, quit_app, initial_file])
        .on_window_event(|window, event| {
            // 窓を閉じる前に、画面の側で保存していない変更を確かめる
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .build(tauri::generate_context!())
        .expect("起動できませんでした");
    app.run(|handle, event| {
        // ⌘Q などで終わろうとしたときも、先に確かめる（quit_app からの終わりは code が付く）
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.emit("close-requested", ());
                }
            }
        }
    });
}
