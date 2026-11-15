//! Mentor Intelligence desktop shell.
//!
//! A thin window around https://www.mentorintell.com. The site gets no Tauri
//! commands (see capabilities/local-shell.json); this file only decides which
//! pages stay in the window, where downloads go, and which user agent is sent.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

const MAIN_LABEL: &str = "main";
const PROBE_LABEL: &str = "ua-probe";
/// The probe page navigates here with the default user agent in the query.
/// `.invalid` is a reserved name, so this never reaches the network.
const PROBE_HOST: &str = "ua-probe.invalid";
const UA_SUFFIX: &str = concat!(" MentorIntelligenceDesktop/", env!("CARGO_PKG_VERSION"));

/// Sign-in providers whose pages must stay inside the window so the session
/// cookie ends up in this webview (not in the system browser).
const SIGN_IN_HOSTS: &[&str] = &[
    "accounts.google.com",
    "appleid.apple.com",
    "login.microsoftonline.com",
    "login.live.com",
];

/// True when a page may load inside the app window.
pub fn stays_in_window(url: &Url) -> bool {
    match url.scheme() {
        // The bundled offline/start page (macOS/Linux use tauri://localhost).
        "tauri" => url.host_str() == Some("localhost"),
        // Blank frames, generated files and inline data.
        "about" | "blob" | "data" => true,
        // Windows serves bundled pages from http(s)://tauri.localhost.
        "http" => url.host_str() == Some("tauri.localhost"),
        "https" => match url.host_str() {
            Some(host) => {
                let host = host.to_ascii_lowercase();
                host == "tauri.localhost"
                    || host == "mentorintell.com"
                    || host.ends_with(".mentorintell.com")
                    || SIGN_IN_HOSTS.contains(&host.as_str())
            }
            None => false,
        },
        _ => false,
    }
}

/// True for links the system can open (browser, mail app, phone app).
fn opens_in_system(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "mailto" | "tel")
}

/// Opens a link in the system browser, at most once per URL every 2 seconds
/// so a page that retries a blocked load cannot open a stream of tabs.
#[derive(Default)]
struct ExternalOpener {
    recent: Mutex<HashMap<String, Instant>>,
}

impl ExternalOpener {
    fn open(&self, app: &AppHandle, url: &Url) {
        if !opens_in_system(url) {
            return;
        }
        let now = Instant::now();
        {
            let mut recent = self.recent.lock().unwrap();
            recent.retain(|_, at| now.duration_since(*at) < Duration::from_secs(2));
            if recent.contains_key(url.as_str()) {
                return;
            }
            recent.insert(url.to_string(), now);
        }
        if let Err(err) = app.opener().open_url(url.as_str(), None::<&str>) {
            eprintln!("could not open {url} in the system browser: {err}");
        }
    }
}

fn build_main_window(app: &AppHandle, user_agent: Option<String>) -> tauri::Result<()> {
    if app.get_webview_window(MAIN_LABEL).is_some() {
        return Ok(());
    }
    let opener = Arc::new(ExternalOpener::default());
    let downloads: Arc<Mutex<HashMap<String, PathBuf>>> = Arc::default();

    let nav_app = app.clone();
    let nav_opener = opener.clone();
    let popup_app = app.clone();
    let popup_opener = opener.clone();
    let download_app = app.clone();

    // index.html checks the connection, then goes to the sign-in page or shows
    // the offline message with a Retry button.
    let mut builder = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App("index.html".into()))
        .title("Mentor Intelligence")
        .inner_size(1366.0, 900.0)
        .min_inner_size(1024.0, 700.0)
        .center()
        .on_navigation(move |url| {
            if stays_in_window(url) {
                true
            } else {
                nav_opener.open(&nav_app, url);
                false
            }
        })
        .on_new_window(move |url, _features| {
            if stays_in_window(&url) {
                // Our own pop-ups (a PDF in a new tab, a sign-in pop-up).
                NewWindowResponse::Allow
            } else {
                popup_opener.open(&popup_app, &url);
                NewWindowResponse::Deny
            }
        })
        .on_page_load(|_window, payload| {
            // Debug builds only: which pages finished loading (no page content).
            if cfg!(debug_assertions)
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                eprintln!("loaded: {}", payload.url());
            }
        })
        .on_download(move |_webview, event| {
            match event {
                DownloadEvent::Requested { url, destination } => {
                    // The webview proposes the Downloads folder; keep that,
                    // but never write somewhere relative or unknown.
                    if !destination.is_absolute() {
                        let name = destination
                            .file_name()
                            .map(|n| n.to_owned())
                            .unwrap_or_else(|| "download".into());
                        let dir = download_app
                            .path()
                            .download_dir()
                            .unwrap_or_else(|_| std::env::temp_dir());
                        *destination = dir.join(name);
                    }
                    downloads
                        .lock()
                        .unwrap()
                        .insert(url.to_string(), destination.clone());
                }
                DownloadEvent::Finished { url, path, success } => {
                    let saved = path.or_else(|| downloads.lock().unwrap().remove(url.as_str()));
                    if success {
                        if let Some(saved) = saved {
                            let _ = download_app.opener().reveal_item_in_dir(saved);
                        }
                    }
                }
                _ => {}
            }
            true
        });

    if let Some(ua) = user_agent {
        eprintln!("user agent: {ua}");
        builder = builder.user_agent(&ua);
    }
    builder.build()?;
    Ok(())
}

/// Learns the webview's own default user agent from a hidden window, then
/// opens the main window with " MentorIntelligenceDesktop/x.y.z" appended.
/// Tauri can only replace the user agent, so this keeps the real browser part
/// (which sign-in providers check) and adds ours. If the probe does not answer
/// in 3 seconds the main window opens with the default user agent.
fn start(app: &AppHandle) -> tauri::Result<()> {
    let opened = Arc::new(AtomicBool::new(false));

    let probe_app = app.clone();
    let probe_opened = opened.clone();
    let probe = WebviewWindowBuilder::new(app, PROBE_LABEL, WebviewUrl::App("probe.html".into()))
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .on_navigation(move |url| {
            if url.host_str() == Some(PROBE_HOST) {
                let ua = url
                    .query_pairs()
                    .find(|(k, _)| k == "ua")
                    .map(|(_, v)| v.into_owned())
                    .filter(|ua| !ua.is_empty() && ua.len() < 1024);
                if !probe_opened.swap(true, Ordering::SeqCst) {
                    let app = probe_app.clone();
                    // Window creation from inside a webview callback can
                    // deadlock on Windows; hand it to another thread.
                    std::thread::spawn(move || {
                        let ua = ua.map(|ua| format!("{ua}{UA_SUFFIX}"));
                        if let Err(err) = build_main_window(&app, ua) {
                            eprintln!("could not open the main window: {err}");
                        }
                        if let Some(probe) = app.get_webview_window(PROBE_LABEL) {
                            let _ = probe.destroy();
                        }
                    });
                }
                return false;
            }
            url.scheme() == "tauri"
                || url.host_str() == Some("tauri.localhost")
                || url.scheme() == "about"
        })
        .build();

    if let Err(err) = probe {
        eprintln!("user agent probe failed to start: {err}");
        opened.store(true, Ordering::SeqCst);
        return build_main_window(app, None);
    }

    let fallback_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        if !opened.swap(true, Ordering::SeqCst) {
            eprintln!("user agent probe timed out; using the default user agent");
            if let Err(err) = build_main_window(&fallback_app, None) {
                eprintln!("could not open the main window: {err}");
            }
            if let Some(probe) = fallback_app.get_webview_window(PROBE_LABEL) {
                let _ = probe.destroy();
            }
        }
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch focuses the running window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window(MAIN_LABEL) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // Only the main window's size and place are remembered.
                .with_denylist(&[PROBE_LABEL])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            start(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Mentor Intelligence");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(s: &str) -> bool {
        stays_in_window(&Url::parse(s).unwrap())
    }

    #[test]
    fn own_site_and_subdomains_stay() {
        assert!(allowed("https://www.mentorintell.com/login"));
        assert!(allowed("https://mentorintell.com/"));
        assert!(allowed("https://auth.mentorintell.com/auth/v1/callback"));
        assert!(allowed("https://edmonton.mentorintell.com/dashboard"));
    }

    #[test]
    fn sign_in_providers_stay() {
        assert!(allowed("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(allowed("https://appleid.apple.com/auth/authorize"));
        assert!(allowed("https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize"));
        assert!(allowed("https://login.live.com/oauth20_authorize.srf"));
    }

    #[test]
    fn other_sites_leave() {
        assert!(!allowed("https://example.com/"));
        assert!(!allowed("https://mentorintell.com.evil.example/"));
        assert!(!allowed("https://evilmentorintell.com/"));
        assert!(!allowed("http://www.mentorintell.com/"));
        assert!(!allowed("https://www.linkedin.com/in/someone"));
        assert!(!allowed("mailto:someone@example.com"));
        assert!(!allowed("file:///etc/passwd"));
    }

    #[test]
    fn bundled_pages_stay() {
        assert!(allowed("tauri://localhost/index.html"));
        assert!(allowed("http://tauri.localhost/index.html"));
        assert!(allowed("https://tauri.localhost/index.html"));
        assert!(allowed("about:blank"));
        assert!(!allowed("tauri://elsewhere/index.html"));
    }
}
