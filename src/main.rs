#![windows_subsystem = "windows"]
// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Pomme : lecteur Apple Music et fichiers locaux, léger et fluide.
//!
//! Une seule vue WebView2 affiche l'interface (servie depuis l'exécutable à
//! l'adresse https://music.apple.com/pomme/, seule origine où le lecteur
//! officiel d'Apple accepte de fonctionner) et fait tourner MusicKit. Le code
//! natif s'occupe de la fenêtre en verre, des fichiers locaux (lecture sans
//! perte, égaliseur), des paroles et des contrôles multimédia de Windows.

mod assets;
mod audio;
mod auth;
mod config;
mod discord;
mod glass;
mod install;
mod langue;
mod library;
mod media;
mod net;
mod setup;
mod sys;

use audio::{AudioEvent, Cmd as AudioCmd, Engine};
use library::Library;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::WindowExtWindows;
use tao::window::WindowBuilder;
use wry::{MemoryUsageLevel, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

pub const APP_URL: &str = "https://music.apple.com/pomme/";

#[derive(Debug)]
pub enum UserEvent {
    /// message de l'interface (JSON)
    Ipc(String),
    /// message à transmettre à l'interface (JSON)
    ToUi(Value),
    /// action de la zone de notification ou d'une seconde instance
    Tray(&'static str),
    /// la page de connexion Apple s'est fermée
    AuthClosed,
    /// fichiers ou dossiers glissés dans la fenêtre
    Drop(Vec<std::path::PathBuf>),
    /// script de test (POMME_TEST uniquement)
    Eval(String),
    /// pochette d'un fichier local lue par le fil des pochettes
    Art(u64, Option<(Vec<u8>, &'static str)>),
}

static PROXY: std::sync::OnceLock<Mutex<EventLoopProxy<UserEvent>>> = std::sync::OnceLock::new();

pub fn post(e: UserEvent) {
    if let Some(p) = PROXY.get() {
        let _ = p.lock().unwrap().send_event(e);
    }
}

/// Options du moteur web : services d'Edge inutiles ici désactivés.
/// Sans carte graphique, le moteur web consomme environ deux fois moins de
/// mémoire (≈ 120 Mo au lieu de 220) ; l'interface reste fluide.
fn browser_args(gpu: bool) -> String {
    // carte graphique, réseau et audio dans le processus principal du moteur :
    // 5 processus au lieu de 7 et ≈ 60 Mo de moins, défilement fluide à 144 Hz
    let mut a = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,msEdgeTranslate,AutofillServerCommunication,OptimizationHints,msEdgeShoppingAssistant,AudioServiceOutOfProcess --enable-features=NetworkServiceInProcess2 --in-process-gpu --autoplay-policy=no-user-gesture-required".to_string();
    if !gpu {
        a.push_str(" --disable-gpu");
    }
    if let Ok(extra) = std::env::var("POMME_ARGS") {
        a.push(' ');
        a.push_str(&extra);
    }
    a
}

/// Instance unique : une seconde ouverture réveille la première.
fn single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject};
    // instance de test séparée (POMME_TEST) : ne gêne pas un Pomme déjà ouvert
    let tag = if std::env::var_os("POMME_TEST").is_some() { "PommeTest" } else { "Pomme" };
    let m = sys::w(&format!("Local\\{tag}.instance"));
    let e = sys::w(&format!("Local\\{tag}.show"));
    let q = sys::w(&format!("Local\\{tag}.quit"));
    unsafe {
        CreateMutexW(std::ptr::null(), 0, m.as_ptr());
        let first = GetLastError() != ERROR_ALREADY_EXISTS;
        let ev = CreateEventW(std::ptr::null(), 0, 0, e.as_ptr());
        if !first {
            windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX);
            SetEvent(ev);
            return false;
        }
        // réveil par une seconde ouverture, fermeture demandée par l'installateur
        for (ev, action) in [(ev as usize, "open"), (CreateEventW(std::ptr::null(), 0, 0, q.as_ptr()) as usize, "quit")] {
            std::thread::spawn(move || loop {
                if WaitForSingleObject(ev as _, u32::MAX) == 0 {
                    post(UserEvent::Tray(action));
                }
            });
        }
        true
    }
}

fn make_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            post(UserEvent::Tray("open"));
        }
    }));
    MenuEvent::set_event_handler(Some(|e: MenuEvent| {
        let a = match e.id.0.as_str() {
            "open" => "open",
            "toggle" => "toggle",
            "next" => "next",
            "prev" => "previous",
            _ => "quit",
        };
        post(UserEvent::Tray(a));
    }));
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &MenuItem::with_id("open", langue::tr("Ouvrir Pomme"), true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("toggle", langue::tr("Lecture / Pause"), true, None),
        &MenuItem::with_id("next", langue::tr("Suivant"), true, None),
        &MenuItem::with_id("prev", langue::tr("Précédent"), true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", langue::tr("Quitter"), true, None),
    ]);
    let icon = tray_icon::Icon::from_rgba(assets::icon_rgba(32), 32, 32).ok();
    let mut b = TrayIconBuilder::new().with_tooltip("Pomme").with_menu(Box::new(menu)).with_menu_on_left_click(false);
    if let Some(i) = icon {
        b = b.with_icon(i);
    }
    b.build().ok()
}

fn data_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| ".".into())
        .join(if std::env::var_os("POMME_TEST").is_some() { "PommeTest" } else { "Pomme" })
}

fn main() {
    // DLL chargées à la demande : uniquement depuis System32
    unsafe {
        windows_sys::Win32::System::LibraryLoader::SetDefaultDllDirectories(
            windows_sys::Win32::System::LibraryLoader::LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
    }
    let args: Vec<String> = std::env::args().collect();
    let stem = std::env::current_exe()
        .ok()
        .and_then(|e| e.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if args.iter().any(|a| a == "--uninstall") {
        std::process::exit(install::quiet_uninstall(args.iter().any(|a| a == "--all")));
    }
    if args.iter().any(|a| a == "--maintenance") || stem == "uninstall" {
        return setup::run(setup::Mode::Maintenance, &args);
    }
    if args.iter().any(|a| a == "--setup") || stem.contains("setup") || stem.contains("install") {
        return setup::run(setup::Mode::Setup, &args);
    }
    // autotest du moteur audio (silencieux) : Pomme.exe --self-test <fichier>
    if let Some(i) = args.iter().position(|a| a == "--self-test") {
        let file = args.get(i + 1).cloned().unwrap_or_default();
        let engine = Engine::start(|e| {
            test_log(&match e {
                AudioEvent::Loaded { duration, rate, bits, channels, codec } => format!("chargé {codec} {rate} Hz {bits} bits {channels} canaux, {duration:.2} s"),
                AudioEvent::Position(p) => format!("position {p:.2}"),
                AudioEvent::Playing(p) => format!("lecture {p}"),
                AudioEvent::Ended => "fin".into(),
                AudioEvent::Error(e) => format!("erreur {e}"),
            })
        });
        engine.send(AudioCmd::Volume(0.0));
        engine.send(AudioCmd::Eq(audio::EqSettings { enabled: true, preamp: -3.0, gains: [6.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 6.0] }));
        engine.send(AudioCmd::Load { path: file, start: 0.0, play: true, gain: 0.0 });
        std::thread::sleep(std::time::Duration::from_secs(4));
        let mut lib = Library { folders: vec![std::env::var("SystemRoot").unwrap_or_default() + "\\Media"], ..Default::default() };
        lib.scan(&|_| {});
        test_log(&format!("analyse : {} fichiers, ex. {:?}", lib.tracks.len(), lib.tracks.first().map(|t| (&t.title, &t.format, t.duration))));
        return;
    }
    if !single_instance() {
        return;
    }
    let minimized = args.iter().any(|a| a == "--minimized");
    let mut cfg = config::Config::load();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let _ = PROXY.set(Mutex::new(proxy.clone()));
    let mut tray = if cfg.close_to_tray { make_tray() } else { None };
    let window = WindowBuilder::new()
        .with_title("Pomme")
        .with_inner_size(tao::dpi::LogicalSize::new(1240.0, 800.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(320.0, 300.0))
        .with_window_icon(assets::window_icon())
        .with_visible(false)
        .build(&event_loop)
        .expect("fenêtre");
    let hwnd = window.hwnd() as isize;
    glass::apply(hwnd);

    let mut ctx = wry::WebContext::new(Some(data_dir().join("WebView2")));
    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new_with_web_context(&mut ctx)
        .with_url("about:blank")
        .with_background_color((13, 14, 22, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_additional_browser_args(&browser_args(cfg.gpu))
        // la vue principale ne quitte jamais l'interface de Pomme
        .with_navigation_handler(|url| url.starts_with(APP_URL) || url == "about:blank")
        .with_ipc_handler(move |m| {
            // seuls les messages de l'interface de Pomme sont acceptés
            if m.uri().to_string().starts_with(APP_URL) {
                let _ = ipc_proxy.send_event(UserEvent::Ipc(m.body().clone()));
            }
        })
        .with_new_window_req_handler(move |uri, features| {
            // la connexion Apple s'affiche dans la fenêtre de Pomme ; tout
            // autre lien part dans le navigateur de l'utilisateur
            if auth::is_apple(&uri) {
                match auth::open(hwnd, features.opener.environment.clone()) {
                    Some(webview) => wry::NewWindowResponse::Create { webview },
                    None => wry::NewWindowResponse::Deny,
                }
            } else {
                if uri.starts_with("https://") {
                    glass::open_url(&uri);
                }
                wry::NewWindowResponse::Deny
            }
        })
        .with_drag_drop_handler(|e| {
            if let wry::DragDropEvent::Drop { paths, .. } = e {
                post(UserEvent::Drop(paths));
            }
            // les fichiers ne doivent jamais remplacer l'interface
            true
        })
        .build(&window)
        .expect("moteur web");
    let library = Arc::new(Mutex::new(Library::load()));
    assets::serve(&webview, library.clone());
    assets::guard_permissions(&webview);
    // l'interface n'est demandée qu'une fois le serveur intégré branché
    let _ = webview.load_url(APP_URL);
    let to_ui = {
        let p = proxy.clone();
        move |v: Value| {
            let _ = p.send_event(UserEvent::ToUi(v));
        }
    };
    let engine = {
        let send = to_ui.clone();
        Engine::start(move |e| send(audio_event_json(e)))
    };
    let mut controls = media::Controls::new(hwnd, {
        let send = to_ui.clone();
        move |action: &str| send(json!({ "type": "mediaKey", "action": action }))
    });

    let presence = discord::Presence::new(cfg.discord, cfg.discord_id.clone());

    // instance de test : exécute %TEMP%\pomme_cmd.js quand il apparaît (pilotage des tests)
    if std::env::var_os("POMME_TEST").is_some() {
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let p = std::env::temp_dir().join("pomme_cmd.js");
            if let Ok(js) = std::fs::read_to_string(&p) {
                let _ = std::fs::remove_file(&p);
                post(UserEvent::Eval(js));
            }
        });
    }

    let mut shown = false;
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::UserEvent(UserEvent::Ipc(msg)) => {
                if std::env::var_os("POMME_TEST").is_some() && !msg.contains("\"log\"") {
                    test_log(&format!("ipc {}", msg.chars().take(120).collect::<String>()));
                }
                let Ok(v) = serde_json::from_str::<Value>(&msg) else { return };
                // langue de l'interface : menu de la zone de notification et boîtes de Windows dans la même langue
                if v["cmd"] == "ready" && langue::definir(v["lang"].as_str().unwrap_or("")) && tray.is_some() {
                    tray = make_tray();
                }
                if v["cmd"] == "ready" && !shown {
                    // instance de test : fenêtre hors écran pour les captures
                    if let Ok(geo) = std::env::var("POMME_SHOW_AT") {
                        let n: Vec<f64> = geo.split(',').filter_map(|x| x.parse().ok()).collect();
                        if n.len() == 4 {
                            window.set_outer_position(tao::dpi::PhysicalPosition::new(n[0], n[1]));
                            window.set_inner_size(tao::dpi::PhysicalSize::new(n[2], n[3]));
                            window.set_visible(true);
                            shown = true;
                        }
                    }
                }
                if v["cmd"] == "ready" && !shown && !minimized && std::env::var_os("POMME_TEST").is_none() {
                    // la fenêtre n'apparaît qu'une fois l'interface dessinée
                    window.set_visible(true);
                    shown = true;
                }
                if v["cmd"] == "config" {
                    // réglages natifs : appliqués tout de suite, sauf la carte graphique (au prochain lancement)
                    if let Some(b) = v["autostart"].as_bool() {
                        cfg.autostart = b;
                        config::set_autostart(b);
                    }
                    if let Some(b) = v["closeToTray"].as_bool() {
                        cfg.close_to_tray = b;
                        if b && tray.is_none() {
                            tray = make_tray();
                        } else if !b {
                            tray = None;
                        }
                    }
                    if let Some(b) = v["gpu"].as_bool() {
                        cfg.gpu = b;
                    }
                    if let Some(b) = v["discord"].as_bool() {
                        cfg.discord = b;
                    }
                    if let Some(id) = v["discordId"].as_str() {
                        // un identifiant d'application Discord est un nombre
                        let id = id.trim();
                        if id.is_empty() {
                            cfg.discord_id = config::DISCORD_ID.into();
                        } else if id.len() <= 24 && id.bytes().all(|b| b.is_ascii_digit()) {
                            cfg.discord_id = id.to_string();
                        }
                    }
                    presence.configure(cfg.discord, cfg.discord_id.clone());
                    cfg.save();
                }
                if v["cmd"] == "media" {
                    presence.update(&v);
                }
                if v["cmd"] == "ready" {
                    auto_zoom(&window, &webview);
                    let c = &cfg;
                    let _ = webview.evaluate_script(&format!(
                        "window.pomme&&pomme.recv({})",
                        json!({ "type": "config", "gpu": c.gpu, "closeToTray": c.close_to_tray, "autostart": c.autostart, "discord": c.discord, "discordId": c.discord_id, "version": env!("CARGO_PKG_VERSION") })
                    ));
                }
                handle(&v, &webview, &engine, &library, &mut controls, &proxy, &window);
            }
            Event::UserEvent(UserEvent::ToUi(v)) => {
                let _ = webview.evaluate_script(&format!("window.pomme&&pomme.recv({v})"));
            }
            Event::UserEvent(UserEvent::AuthClosed) => auth::close(),
            Event::UserEvent(UserEvent::Art(n, art)) => assets::art_ready(n, art),
            Event::UserEvent(UserEvent::Eval(js)) => {
                let js = js.trim_start_matches('\u{feff}');
                if js.starts_with("//resize ") {
                    let n: Vec<f64> = js[9..].trim().split(',').filter_map(|x| x.parse().ok()).collect();
                    if n.len() == 2 {
                        window.set_inner_size(tao::dpi::PhysicalSize::new(n[0], n[1]));
                    }
                } else {
                    let _ = webview.evaluate_script(&js);
                }
            }
            Event::UserEvent(UserEvent::Drop(paths)) => dropped(paths, &library, &proxy),
            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                fit(&webview, size);
                auto_zoom(&window, &webview);
                auth::relayout(hwnd);
                // réduite : mémoire réduite, la musique continue
                let hidden = size.width == 0 || size.height == 0 || window.is_minimized();
                let _ = webview.set_memory_usage_level(if hidden { MemoryUsageLevel::Low } else { MemoryUsageLevel::Normal });
            }
            // passage sur un autre écran (taille, orientation ou échelle différentes) : la vue suit la fenêtre
            Event::WindowEvent { event: WindowEvent::ScaleFactorChanged { new_inner_size, .. }, .. } => {
                fit(&webview, *new_inner_size);
                auth::relayout(hwnd);
                auto_zoom(&window, &webview);
            }
            Event::WindowEvent { event: WindowEvent::Moved(_), .. } => {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    fit(&webview, size);
                }
                auto_zoom(&window, &webview);
            }
            Event::UserEvent(UserEvent::Tray(action)) => match action {
                "open" => {
                    window.set_visible(true);
                    window.set_minimized(false);
                    window.set_focus();
                    shown = true;
                    let _ = webview.set_visible(true);
                    let _ = webview.set_memory_usage_level(MemoryUsageLevel::Normal);
                }
                "quit" => {
                    engine.send(AudioCmd::Stop);
                    tray = None;
                    *flow = ControlFlow::Exit;
                }
                a => {
                    let _ = webview.evaluate_script(&format!("window.pomme&&pomme.recv({})", json!({ "type": "mediaKey", "action": a })));
                }
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if tray.is_some() {
                    // la musique continue : rendu suspendu, mémoire minimale
                    window.set_visible(false);
                    let _ = webview.set_visible(false);
                    let _ = webview.set_memory_usage_level(MemoryUsageLevel::Low);
                } else {
                    engine.send(AudioCmd::Stop);
                    *flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// Journal de test (POMME_TEST) : %TEMP%\pomme_test.log
fn test_log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("pomme_test.log")) {
        let _ = writeln!(f, "{msg}");
    }
}

/// Échelle de l'interface selon l'écran où se trouve la fenêtre : agrandie
/// sur les écrans 1440p et 4K à 100 %, pour rester lisible. Zoom du moteur
/// web (et non CSS) : mise en page et coordonnées de la souris restent justes.
fn auto_zoom(window: &tao::window::Window, webview: &wry::WebView) {
    static ZOOM: Mutex<f64> = Mutex::new(1.0);
    let Some(m) = window.current_monitor() else { return };
    let s = m.size().to_logical::<f64>(m.scale_factor());
    let short = s.width.min(s.height);
    let z = if short >= 2000.0 { 1.4 } else if short >= 1400.0 { 1.15 } else { 1.0 };
    let mut cur = ZOOM.lock().unwrap();
    if (*cur - z).abs() > 0.001 {
        *cur = z;
        let _ = webview.zoom(z);
    }
}

/// La vue occupe toute la zone cliente de la fenêtre.
fn fit(webview: &wry::WebView, size: tao::dpi::PhysicalSize<u32>) {
    if size.width == 0 || size.height == 0 {
        return;
    }
    let _ = webview.set_bounds(wry::Rect {
        position: wry::dpi::PhysicalPosition::new(0, 0).into(),
        size: wry::dpi::PhysicalSize::new(size.width, size.height).into(),
    });
}

/// Mini-lecteur : petite fenêtre toujours visible ; la taille précédente (ou
/// l'état agrandi) est rétablie en sortant.
fn mini(window: &tao::window::Window, on: bool) {
    static PREV: Mutex<Option<(tao::dpi::PhysicalSize<u32>, bool)>> = Mutex::new(None);
    let mut prev = PREV.lock().unwrap();
    if on {
        if prev.is_none() {
            *prev = Some((window.inner_size(), window.is_maximized()));
        }
        window.set_maximized(false);
        window.set_inner_size(tao::dpi::LogicalSize::new(340.0, 380.0));
        window.set_always_on_top(true);
    } else {
        window.set_always_on_top(false);
        if let Some((s, maximized)) = prev.take() {
            window.set_inner_size(s);
            window.set_maximized(maximized);
        }
    }
}

/// Fichiers glissés : les dossiers rejoignent la bibliothèque, les morceaux
/// sont lus tout de suite sans être ajoutés.
fn dropped(paths: Vec<std::path::PathBuf>, library: &Arc<Mutex<Library>>, proxy: &EventLoopProxy<UserEvent>) {
    let mut ids = Vec::new();
    let mut new_folder = false;
    {
        let mut l = library.lock().unwrap();
        for p in &paths {
            if p.is_dir() {
                let d = p.to_string_lossy().to_string();
                if !l.folders.contains(&d) {
                    l.folders.push(d);
                    new_folder = true;
                }
            } else if let Some(t) = library::read_file(p) {
                ids.push(t.id.clone());
                if l.find(&t.id).is_none() {
                    l.extra.push(t);
                }
            }
        }
        if !ids.is_empty() {
            let extra: Vec<&library::LocalTrack> = l.extra.iter().chain(l.tracks.iter()).filter(|t| ids.contains(&t.id)).collect();
            let _ = proxy.send_event(UserEvent::ToUi(json!({ "type": "dropped", "ids": ids, "tracks": extra })));
        }
    }
    if new_folder {
        scan(library, proxy);
    }
}

fn audio_event_json(e: AudioEvent) -> Value {
    match e {
        AudioEvent::Loaded { duration, rate, bits, channels, codec } => {
            json!({ "type": "local.loaded", "duration": duration, "rate": rate, "bits": bits, "channels": channels, "codec": codec })
        }
        AudioEvent::Position(p) => json!({ "type": "local.position", "position": p }),
        AudioEvent::Playing(p) => json!({ "type": "local.playing", "playing": p }),
        AudioEvent::Ended => json!({ "type": "local.ended" }),
        AudioEvent::Error(e) => json!({ "type": "local.error", "message": e }),
    }
}

fn send_library(lib: &Library, proxy: &EventLoopProxy<UserEvent>) {
    let _ = proxy.send_event(UserEvent::ToUi(json!({
        "type": "library",
        "folders": lib.folders,
        "tracks": lib.tracks,
    })));
}

fn scan(library: &Arc<Mutex<Library>>, proxy: &EventLoopProxy<UserEvent>) {
    let lib = library.clone();
    let p = proxy.clone();
    std::thread::spawn(move || {
        let mut l = {
            let g = lib.lock().unwrap();
            Library {
                folders: g.folders.clone(),
                tracks: g.tracks.clone(),
                extra: g.extra.clone(),
            }
        };
        let progress = |n: usize| {
            let _ = p.send_event(UserEvent::ToUi(json!({ "type": "scanProgress", "count": n })));
        };
        l.scan(&progress);
        let mut g = lib.lock().unwrap();
        // dossiers ajoutés ou retirés pendant l'analyse : la liste actuelle fait foi
        l.folders = g.folders.clone();
        let folders = l.folders.clone();
        l.tracks.retain(|t| folders.iter().any(|f| std::path::Path::new(&t.path).starts_with(f)));
        l.extra = std::mem::take(&mut g.extra);
        l.save();
        send_library(&l, &p);
        *g = l;
    });
}

fn handle(
    v: &Value,
    webview: &wry::WebView,
    engine: &Engine,
    library: &Arc<Mutex<Library>>,
    controls: &mut media::Controls,
    proxy: &EventLoopProxy<UserEvent>,
    window: &tao::window::Window,
) {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let f = |k: &str| v[k].as_f64().unwrap_or(0.0);
    match v["cmd"].as_str().unwrap_or("") {
        "ready" => {
            send_library(&library.lock().unwrap(), proxy);
            // une fois par lancement : fichiers ajoutés ou retirés depuis (seuls les nouveaux sont relus)
            static STARTUP_SCAN: std::sync::Once = std::sync::Once::new();
            if !library.lock().unwrap().folders.is_empty() {
                STARTUP_SCAN.call_once(|| scan(library, proxy));
            }
        }
        "local.addFolder" => {
            if let Some(dir) = glass::pick_folder(window.hwnd() as isize, langue::tr("Choisir un dossier de musique")) {
                {
                    let mut l = library.lock().unwrap();
                    if !l.folders.contains(&dir) {
                        l.folders.push(dir);
                    }
                }
                scan(library, proxy);
            }
        }
        "local.removeFolder" => {
            let dir = s("folder");
            let mut l = library.lock().unwrap();
            l.folders.retain(|d| *d != dir);
            // comparaison par dossiers entiers : retirer « Musique » garde « Musique 2 »
            let remaining = l.folders.clone();
            l.tracks.retain(|t| remaining.iter().any(|f| std::path::Path::new(&t.path).starts_with(f)));
            l.save();
            send_library(&l, proxy);
        }
        "local.rescan" => scan(library, proxy),
        "audio.load" => {
            let found = library.lock().unwrap().find(&s("id")).map(|t| (t.path.clone(), t.gain));
            match found {
                Some((path, gain)) => engine.send(AudioCmd::Load {
                    path,
                    start: f("start"),
                    play: v["play"].as_bool().unwrap_or(true),
                    gain: if v["normalize"].as_bool().unwrap_or(false) { gain } else { 0.0 },
                }),
                None => {
                    let _ = proxy.send_event(UserEvent::ToUi(json!({ "type": "local.error", "message": "Fichier introuvable" })));
                }
            }
        }
        "audio.devices" => {
            let _ = proxy.send_event(UserEvent::ToUi(json!({ "type": "devices", "list": audio::devices() })));
        }
        "audio.device" => engine.send(AudioCmd::Device(Some(s("name")).filter(|n| !n.is_empty()))),
        "audio.labels" => assets::allow_device_labels(),
        "auth.close" => auth::close(),
        "window.mini" => mini(window, v["on"].as_bool().unwrap_or(false)),
        "audio.play" => engine.send(AudioCmd::Play),
        "audio.pause" => engine.send(AudioCmd::Pause),
        "audio.stop" => engine.send(AudioCmd::Stop),
        "audio.seek" => engine.send(AudioCmd::Seek(f("position"))),
        "audio.volume" => engine.send(AudioCmd::Volume(f("volume") as f32)),
        "audio.eq" => {
            if let Ok(eq) = serde_json::from_value(v["eq"].clone()) {
                engine.send(AudioCmd::Eq(eq));
            }
        }
        "lyrics" => {
            let req = net::LyricsRequest {
                title: s("title"),
                artist: s("artist"),
                album: s("album"),
                duration: f("duration"),
                local_path: library.lock().unwrap().find(&s("localId")).map(|t| t.path.clone()),
                key: s("key"),
            };
            let p = proxy.clone();
            std::thread::spawn(move || {
                let r = net::lyrics(&req);
                let _ = p.send_event(UserEvent::ToUi(json!({ "type": "lyrics", "key": req.key, "lyrics": r })));
            });
        }
        "media" => controls.update(v),
        "openExternal" => {
            let url = s("url");
            if url.starts_with("https://") {
                glass::open_url(&url);
            }
        }
        "log" => {
            if std::env::var_os("POMME_TEST").is_some() {
                test_log(&s("msg"));
            }
        }
        "devtools" => {
            #[cfg(debug_assertions)]
            webview.open_devtools();
            #[cfg(not(debug_assertions))]
            let _ = webview;
        }
        _ => {}
    }
}
