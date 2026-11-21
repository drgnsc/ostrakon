mod files;
mod render;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use files::{ArchiveEntry, Config, Sibling};
use serde::Serialize;
use tauri::http::{header, Response, StatusCode};
use tauri::plugin::{Builder as PluginBuilder, TauriPlugin};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Url, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

const WATCH_INTERVAL: Duration = Duration::from_millis(800);

#[derive(Default)]
struct AppState {
    pending_path: Mutex<Option<String>>,
    /// The open document and the modification time it was rendered from.
    current: Mutex<Option<(PathBuf, u64)>>,
    scan_lock: Mutex<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    path: String,
    name: String,
    crumbs: Vec<String>,
    html: String,
    toc: Vec<render::TocEntry>,
    meta: Vec<(String, String)>,
    words: usize,
    modified: u64,
    siblings: Vec<Sibling>,
}

fn load_document(path: PathBuf) -> Result<Document, String> {
    let text = files::read_text(&path)?;
    let rendered = render::render(&text);
    Ok(Document {
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        crumbs: files::crumbs(&path),
        siblings: files::siblings(&path),
        modified: files::modified_ms(&path),
        html: rendered.html,
        toc: rendered.toc,
        meta: rendered.meta,
        words: rendered.words,
        path: files::display_path(&path),
    })
}

/// Renders off the main thread, then makes the result the current document.
async fn show(window: &WebviewWindow, path: PathBuf) -> Result<Document, String> {
    let document = {
        let path = path.clone();
        tauri::async_runtime::spawn_blocking(move || load_document(path))
            .await
            .map_err(|e| e.to_string())??
    };
    let title = document.toc.iter().find(|t| t.level == 1).map(|t| t.text.as_str()).unwrap_or(&document.name);
    let _ = window.set_title(&format!("{title} · Ostrakon"));
    *window.state::<AppState>().current.lock().unwrap() = Some((path, document.modified));
    Ok(document)
}

#[tauri::command]
fn initial_path(state: State<'_, AppState>) -> Option<String> {
    state.pending_path.lock().unwrap().take()
}

#[tauri::command]
async fn open_document(window: WebviewWindow, path: String) -> Result<Document, String> {
    let resolved = files::resolve_markdown(&path)?;
    show(&window, resolved).await
}

#[tauri::command]
async fn open_link(window: WebviewWindow, href: String) -> Result<Document, String> {
    let current = window.state::<AppState>().current.lock().unwrap().clone();
    let (document, _) = current.ok_or("Brak otwartego dokumentu.")?;
    let resolved = files::resolve_relative_markdown(&document, &href)?;
    show(&window, resolved).await
}

#[tauri::command]
async fn pick_document(window: WebviewWindow) -> Result<Option<Document>, String> {
    let picked = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Otwórz dokument")
        .add_filter("Markdown", &files::MARKDOWN_EXTENSIONS)
        .blocking_pick_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let resolved = files::resolve_markdown(&path.to_string_lossy())?;
    show(&window, resolved).await.map(Some)
}

/// The only way out of the app: a browser, for web and mail links, after a user click.
#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|_| "Nieprawidłowy adres.")?;
    if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
        return Err("Ten typ odnośnika jest zablokowany.".into());
    }
    app.opener().open_url(parsed.as_str(), None::<&str>).map_err(|e| e.to_string())
}

fn data_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(name))
}

/// Without a saved config the archive starts empty: the user picks folders with "Dodaj folder".
fn load_config(app: &AppHandle) -> Config {
    data_file(app, "config.json")
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Config>(&t).ok())
        .unwrap_or_default()
}

fn save_json<T: Serialize>(app: &AppHandle, name: &str, value: &T) -> Result<(), String> {
    let path = data_file(app, name)?;
    let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, text).map_err(|e| e.to_string())?;
    fs::rename(&temp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn archive_cached(app: AppHandle) -> Vec<ArchiveEntry> {
    data_file(&app, "archive.json")
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

#[tauri::command]
async fn archive_scan(app: AppHandle) -> Result<Vec<ArchiveEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _guard = state.scan_lock.lock().unwrap();
        let entries = files::scan_roots(&load_config(&app).roots);
        save_json(&app, "archive.json", &entries)?;
        Ok(entries)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn archive_roots(app: AppHandle) -> Vec<String> {
    load_config(&app).roots
}

#[tauri::command]
async fn add_root(window: WebviewWindow) -> Result<Vec<String>, String> {
    let picked = window.dialog().file().set_parent(&window).set_title("Dodaj folder do archiwum").blocking_pick_folder();
    let app = window.app_handle();
    let mut config = load_config(app);
    if let Some(path) = picked.and_then(|p| p.into_path().ok()) {
        let path = path.to_string_lossy().into_owned();
        if !config.roots.contains(&path) {
            config.roots.push(path);
            save_json(app, "config.json", &config)?;
        }
    }
    Ok(config.roots)
}

#[tauri::command]
fn remove_root(app: AppHandle, path: String) -> Result<Vec<String>, String> {
    let mut config = load_config(&app);
    config.roots.retain(|r| r != &path);
    save_json(&app, "config.json", &config)?;
    Ok(config.roots)
}

/// Serves images referenced by the open document as https://art.localhost/?p=<relative path>.
fn art_response(app: &AppHandle, uri: &str) -> Response<Vec<u8>> {
    let not_found = || Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap();
    let current = app.state::<AppState>().current.lock().unwrap().clone();
    let Some((document, _)) = current else { return not_found() };
    let Some(relative) = Url::parse(uri).ok().and_then(|u| {
        u.query_pairs().find(|(key, _)| key == "p").map(|(_, value)| value.into_owned())
    }) else {
        return not_found();
    };
    match files::read_project_image(&document, &relative) {
        Ok((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(header::CACHE_CONTROL, "no-store")
            .body(bytes)
            .unwrap(),
        Err(_) => not_found(),
    }
}

/// Stat-polling instead of fs events: one file, and it survives the atomic renames editors do.
fn watch_current_document(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(WATCH_INTERVAL);
        let state = app.state::<AppState>();
        let mut current = state.current.lock().unwrap();
        if let Some((path, seen)) = current.as_mut() {
            let modified = files::modified_ms(path);
            if modified != 0 && modified != *seen {
                *seen = modified;
                let _ = app.emit("doc-changed", ());
            }
        }
    });
}

/// The webview may only ever show the bundled app; anything else is refused at navigation.
fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    PluginBuilder::new("navigation-guard")
        .on_navigation(|_webview, url| {
            let host = url.host_str().unwrap_or("");
            if host == "tauri.localhost" || url.scheme() == "tauri" {
                return true;
            }
            #[cfg(debug_assertions)]
            {
                // `pnpm tauri dev` serves src/ from the CLI's own server (http://127.0.0.1:<port>)
                // and hands that address to the app as devUrl. Only that exact origin is allowed.
                let dev_server = _webview.config().build.dev_url.as_ref().is_some_and(|dev| dev.origin() == url.origin());
                if host == "localhost" || dev_server {
                    return true;
                }
            }
            false
        })
        .build()
}

fn path_from_args(args: &[String], cwd: Option<&Path>) -> Option<String> {
    let arg = args.iter().skip(1).find(|a| !a.starts_with('-'))?;
    let path = Path::new(arg);
    Some(match cwd {
        Some(cwd) if path.is_relative() => cwd.join(path).to_string_lossy().into_owned(),
        _ => arg.clone(),
    })
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let state = AppState {
        pending_path: Mutex::new(path_from_args(&args, std::env::current_dir().ok().as_deref())),
        ..Default::default()
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            if let Some(path) = path_from_args(&argv, Some(Path::new(&cwd))) {
                let _ = app.emit("open-path", path);
            }
        }))
        .plugin(navigation_guard())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .register_asynchronous_uri_scheme_protocol("art", |context, request, responder| {
            let app = context.app_handle().clone();
            let uri = request.uri().to_string();
            std::thread::spawn(move || responder.respond(art_response(&app, &uri)));
        })
        .setup(|app| {
            watch_current_document(app.handle().clone());
            // Safety net: never stay invisible if the frontend fails before its first frame.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial_path,
            open_document,
            open_link,
            pick_document,
            open_external,
            archive_cached,
            archive_scan,
            archive_roots,
            add_root,
            remove_root,
            show_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ostrakon");
}

/// The window starts hidden and is revealed once the first frame is ready, so no blank flash.
#[tauri::command]
fn show_window(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}
