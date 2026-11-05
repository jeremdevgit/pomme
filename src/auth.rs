// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Fenêtres ouvertes par Apple (connexion à l'identifiant Apple) : affichées
//! dans la fenêtre de Pomme, au-dessus de l'interface, plutôt que dans une
//! seconde fenêtre. La page de connexion garde l'accès aux passkeys
//! (iPhone, Windows Hello) puisqu'elle tourne dans le même moteur.

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::cell::RefCell;
use std::num::NonZeroIsize;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::WindowCloseRequestedEventHandler;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use wry::{Rect, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

thread_local! {
    static CHILD: RefCell<Option<wry::WebView>> = const { RefCell::new(None) };
}

struct Parent(isize);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        // SAFETY : la fenêtre principale vit aussi longtemps que l'application
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
    }
}

/// Zone de la fenêtre de connexion : centrée, 460 × 720 au plus.
fn bounds(hwnd: isize) -> Rect {
    unsafe {
        let mut r = RECT::default();
        let _ = GetClientRect(HWND(hwnd as *mut _), &mut r);
        let scale = GetDpiForWindow(HWND(hwnd as *mut _)).max(96) as f64 / 96.0;
        let (w, h) = ((r.right - r.left) as f64 / scale, (r.bottom - r.top) as f64 / scale);
        let bw = 460f64.min(w - 24.0).max(280.0);
        let bh = 720f64.min(h - 96.0).max(320.0);
        Rect {
            position: tao::dpi::LogicalPosition::new(((w - bw) / 2.0).max(0.0), (56f64).min(((h - bh) / 2.0).max(0.0))).into(),
            size: tao::dpi::LogicalSize::new(bw, bh).into(),
        }
    }
}

/// Adresse https sur apple.com ou un de ses sous-domaines.
pub fn is_apple(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_lowercase();
    // identifiants éventuels (user@hôte) et port écartés
    let host = host.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("");
    host == "apple.com" || host.ends_with(".apple.com")
}

/// Crée la vue de connexion et la renvoie au moteur pour qu'il y ouvre la page.
pub fn open(hwnd: isize, env: ICoreWebView2Environment) -> Option<ICoreWebView2> {
    close();
    let child = WebViewBuilder::new()
        .with_environment(env)
        .with_bounds(bounds(hwnd))
        .with_focused(true)
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        // la fenêtre de connexion reste chez Apple ; un autre lien part dans le navigateur
        .with_navigation_handler(|url| {
            if url == "about:blank" || is_apple(&url) {
                return true;
            }
            if url.starts_with("https://") {
                crate::glass::open_url(&url);
            }
            false
        })
        .build_as_child(&Parent(hwnd))
        .ok()?;
    let core = child.webview();
    unsafe {
        let mut token = 0i64;
        let _ = core.add_WindowCloseRequested(
            &WindowCloseRequestedEventHandler::create(Box::new(|_, _| {
                crate::post(crate::UserEvent::AuthClosed);
                Ok(())
            })),
            &mut token,
        );
    }
    CHILD.with(|c| *c.borrow_mut() = Some(child));
    crate::post(crate::UserEvent::ToUi(serde_json::json!({ "type": "auth", "open": true })));
    Some(core)
}

pub fn close() {
    let had = CHILD.with(|c| c.borrow_mut().take()).is_some();
    if had {
        crate::post(crate::UserEvent::ToUi(serde_json::json!({ "type": "auth", "open": false })));
    }
}

pub fn relayout(hwnd: isize) {
    CHILD.with(|c| {
        if let Some(w) = c.borrow().as_ref() {
            let _ = w.set_bounds(bounds(hwnd));
        }
    });
}
