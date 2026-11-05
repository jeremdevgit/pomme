// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Interface intégrée à l'exécutable, servie par interception des requêtes
//! vers https://music.apple.com/pomme/… (aucun fichier sur le disque, aucune
//! requête réseau pour l'interface), ainsi que les pochettes des fichiers locaux.

use crate::library::Library;
use std::sync::{Arc, Mutex};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
use windows::core::{HSTRING, PWSTR};
use windows::Win32::UI::Shell::SHCreateMemStream;
use wry::WebViewExtWindows;

#[path = "icon.rs"]
mod icon;

const INDEX: &str = include_str!("../ui/index.html");
const STYLE: &str = include_str!("../ui/style.css");
const APP: &str = include_str!("../ui/app.js");

pub fn icon_rgba(size: u32) -> Vec<u8> {
    icon::render_icon(size)
}

pub fn window_icon() -> Option<tao::window::Icon> {
    tao::window::Icon::from_rgba(icon::render_icon(64), 64, 64).ok()
}

fn resolve(path: &str) -> Option<(Vec<u8>, &'static str, &'static str)> {
    let no_cache = "no-cache";
    Some(match path {
        "" | "index.html" => (INDEX.as_bytes().to_vec(), "text/html; charset=utf-8", no_cache),
        "style.css" => (STYLE.as_bytes().to_vec(), "text/css; charset=utf-8", no_cache),
        "app.js" => (APP.as_bytes().to_vec(), "text/javascript; charset=utf-8", no_cache),
        "icon.png" => (png(&icon::render_icon(128), 128), "image/png", "max-age=31536000"),
        _ => return None,
    })
}

// ── pochettes des fichiers locaux : lues hors du fil de l'interface ─────
// La requête est mise en attente (deferral), un fil lit le fichier, puis la
// réponse est donnée sur le fil de l'interface (seul autorisé à toucher WebView2).

type Pending = (ICoreWebView2WebResourceRequestedEventArgs, ICoreWebView2Deferral);

thread_local! {
    static PENDING: std::cell::RefCell<std::collections::HashMap<u64, Pending>> = Default::default();
    static ENV: std::cell::RefCell<Option<ICoreWebView2Environment>> = const { std::cell::RefCell::new(None) };
}

static ART_TX: std::sync::OnceLock<Mutex<std::sync::mpsc::Sender<(u64, Option<String>)>>> = std::sync::OnceLock::new();

fn art_worker() -> std::sync::mpsc::Sender<(u64, Option<String>)> {
    ART_TX
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<(u64, Option<String>)>();
            let _ = std::thread::Builder::new().name("pomme-art".into()).spawn(move || {
                for (n, file) in rx {
                    let art = file.and_then(|f| std::panic::catch_unwind(|| crate::library::artwork(&f)).ok().flatten());
                    crate::post(crate::UserEvent::Art(n, art));
                }
            });
            Mutex::new(tx)
        })
        .lock()
        .unwrap()
        .clone()
}

fn respond(env: &ICoreWebView2Environment, body: Option<(Vec<u8>, &str, &str)>) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
    unsafe {
        match body {
            Some((body, mime, cache)) => {
                let stream = SHCreateMemStream(Some(&body));
                // scripts : uniquement ceux de Pomme et le lecteur d'Apple
                let csp = "Content-Security-Policy: default-src 'self' https: data: blob:; script-src 'self' https://js-cdn.music.apple.com https://*.apple.com blob: 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline' https:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";
                let headers = HSTRING::from(format!("Content-Type: {mime}\nCache-Control: {cache}\nX-Content-Type-Options: nosniff\n{csp}"));
                env.CreateWebResourceResponse(stream.as_ref(), 200, &HSTRING::from("OK"), &headers)
            }
            None => env.CreateWebResourceResponse(None, 404, &HSTRING::from("Not Found"), &HSTRING::new()),
        }
    }
}

/// Pochette lue : la requête en attente reçoit sa réponse (fil de l'interface).
pub fn art_ready(n: u64, art: Option<(Vec<u8>, &'static str)>) {
    let Some((args, deferral)) = PENDING.with(|p| p.borrow_mut().remove(&n)) else { return };
    ENV.with(|e| {
        if let Some(env) = e.borrow().as_ref() {
            if let Ok(resp) = respond(env, art.map(|(b, m)| (b, m, "max-age=86400"))) {
                unsafe {
                    let _ = args.SetResponse(&resp);
                }
            }
        }
    });
    unsafe {
        let _ = deferral.Complete();
    }
}

/// PNG minimal non compressé (blocs « stored ») pour l'icône.
fn png(rgba: &[u8], size: u32) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..size as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * size as usize * 4..(y + 1) * size as usize * 4]);
    }
    // zlib sans compression
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(last as u8);
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Instant (ms) jusqu'auquel l'accès micro est accordé : uniquement juste après
/// l'ouverture du menu des sorties audio par l'utilisateur.
static LABELS_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// L'interface va demander les noms des sorties audio (menu « Sortie audio »).
pub fn allow_device_labels() {
    LABELS_UNTIL.store(now_ms() + 4000, std::sync::atomic::Ordering::Relaxed);
}

/// Autorisations du moteur web : caméra, position, notifications, capteurs…
/// refusées sans question. Le micro n'est accordé (une fois, mémorisé) que
/// pour lire le nom des sorties audio, à la demande de l'utilisateur ; aucun son
/// n'est capté (le flux est arrêté aussitôt).
pub fn guard_permissions(webview: &wry::WebView) {
    let wv: ICoreWebView2 = webview.webview();
    unsafe {
        let mut token = 0i64;
        let _ = wv.add_PermissionRequested(
            &webview2_com::PermissionRequestedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                args.PermissionKind(&mut kind)?;
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_pwstr(uri);
                // la demande porte l'origine (https://music.apple.com/) ; la vue principale
                // ne peut afficher que l'interface de Pomme (navigation verrouillée)
                let ours = uri.starts_with("https://music.apple.com/");
                let mic = kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE;
                let allow = ours && mic && now_ms() < LABELS_UNTIL.load(std::sync::atomic::Ordering::Relaxed);
                if std::env::var_os("POMME_TEST").is_some() {
                    crate::test_log(&format!("autorisation {} pour {uri} : {}", kind.0, if allow { "accordée" } else { "refusée" }));
                }
                if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                    return Ok(());
                }
                // accord du micro mémorisé (plus de demande ensuite) ; un refus du micro ne l'est jamais
                if let Ok(a3) = windows::core::Interface::cast::<ICoreWebView2PermissionRequestedEventArgs3>(&args) {
                    let _ = a3.SetSavesInProfile(allow || !mic);
                }
                args.SetState(if allow { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DENY })?;
                Ok(())
            })),
            &mut token,
        );
    }
}

pub fn serve(webview: &wry::WebView, lib: Arc<Mutex<Library>>) {
    let wv: ICoreWebView2 = webview.webview();
    let env: ICoreWebView2Environment = webview.environment();
    ENV.with(|e| *e.borrow_mut() = Some(env.clone()));
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    unsafe {
        let filter = HSTRING::from(format!("{}*", crate::APP_URL));
        let _ = wv.AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
        let mut token = 0i64;
        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let req = args.Request()?;
            let mut uri = PWSTR::null();
            req.Uri(&mut uri)?;
            let uri = take_pwstr(uri);
            let path = uri
                .strip_prefix(crate::APP_URL)
                .unwrap_or("")
                .split(['?', '#'])
                .next()
                .unwrap_or("")
                .to_string();
            if std::env::var_os("POMME_TEST").is_some() {
                crate::test_log(&format!("servi : {uri}"));
            }
            if let Some(id) = path.strip_prefix("art/") {
                let file = lib.lock().unwrap().find(id).map(|t| t.path.clone());
                let deferral = args.GetDeferral()?;
                let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                PENDING.with(|p| p.borrow_mut().insert(n, (args.clone(), deferral)));
                let _ = art_worker().send((n, file));
                return Ok(());
            }
            let resp = respond(&env, resolve(&path))?;
            args.SetResponse(&resp)?;
            Ok(())
        }));
        let _ = wv.add_WebResourceRequested(&handler, &mut token);
    }
}
