// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Installateur et outil de maintenance (réparer / désinstaller), dans la
//! même fenêtre « liquid glass » que l'application. C'est le même exécutable :
//! nommé Pomme-Setup.exe il installe, nommé uninstall.exe il entretient.

use crate::install::{self, Options};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::windows::WindowExtWindows;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const PAGE: &str = include_str!("../ui/setup.html");

static UNINSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Setup,
    Maintenance,
}

enum Ev {
    Ipc(String),
    Ui(Value),
}

pub fn run(mode: Mode, args: &[String]) {
    // installation silencieuse : Pomme-Setup.exe /S
    if mode == Mode::Setup && args.iter().any(|a| a == "/S" || a == "--quiet") {
        let mut o = Options::default();
        if let Some(i) = args.iter().position(|a| a == "--dir") {
            if let Some(d) = args.get(i + 1) {
                o.dir = d.clone();
            }
        }
        std::process::exit(if install::install(&o, &|_| {}).is_ok() { 0 } else { 1 });
    }

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let title = if mode == Mode::Setup { "Installer Pomme" } else { "Pomme — désinstaller ou réparer" };
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(tao::dpi::LogicalSize::new(640.0, 660.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(520.0, 480.0))
        .with_window_icon(crate::assets::window_icon())
        .build(&event_loop)
        .expect("fenêtre");
    crate::glass::apply(window.hwnd() as isize);
    let glass = false;
    let p = proxy.clone();
    let mut ctx = wry::WebContext::new(Some(std::env::temp_dir().join("PommeSetup")));
    let webview = WebViewBuilder::new_with_web_context(&mut ctx)
        .with_html(PAGE)
        .with_background_color((13, 14, 22, 255))
        .with_ipc_handler(move |m| {
            let _ = p.send_event(Ev::Ipc(m.body().clone()));
        })
        .build(&window)
        .expect("moteur web");

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ev::Ui(v)) => {
                let _ = webview.evaluate_script(&format!("window.setup&&setup.recv({v})"));
            }
            Event::UserEvent(Ev::Ipc(msg)) => {
                let Ok(v) = serde_json::from_str::<Value>(&msg) else { return };
                let send = {
                    let p = proxy.clone();
                    move |v: Value| {
                        let _ = p.send_event(Ev::Ui(v));
                    }
                };
                match v["cmd"].as_str().unwrap_or("") {
                    "init" => send(json!({
                        "type": "init",
                        "mode": if mode == Mode::Setup { "setup" } else { "maintenance" },
                        "existing": install::installed_dir().is_some(),
                        "existingVersion": install::installed_version(),
                        "opts": Options::default(),
                        "version": env!("CARGO_PKG_VERSION"),
                        "glass": glass,
                    })),
                    "browse" => {
                        if let Some(d) = crate::glass::pick_folder(window.hwnd() as isize) {
                            let p = std::path::PathBuf::from(&d);
                            let d = if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("Pomme")) { p } else { p.join("Pomme") };
                            send(json!({ "type": "dir", "dir": d.to_string_lossy() }));
                        }
                    }
                    "run" => {
                        let task = v["task"].as_str().unwrap_or("").to_string();
                        let opts: Options = serde_json::from_value(v["opts"].clone()).unwrap_or_default();
                        std::thread::spawn(move || {
                            let step = |s: &str| send(json!({ "type": "step", "text": s }));
                            let r = match task.as_str() {
                                "install" => install::install(&opts, &step),
                                "repair" => install::repair(&step),
                                "uninstall" => install::uninstall(false, &step).map(|_| Vec::new()),
                                _ => install::uninstall(true, &step).map(|_| Vec::new()),
                            };
                            if r.is_ok() && task.starts_with("uninstall") {
                                UNINSTALLED.store(true, Ordering::Relaxed);
                            }
                            match r {
                                Ok(notes) => send(json!({ "type": "done", "ok": true, "notes": notes })),
                                Err(e) => send(json!({ "type": "done", "ok": false, "error": e })),
                            }
                        });
                    }
                    "launch" => {
                        let dir = install::installed_dir().unwrap_or_else(install::default_dir);
                        let _ = std::process::Command::new(dir.join(install::APP_EXE)).current_dir(&dir).spawn();
                    }
                    "close" => *flow = ControlFlow::Exit,
                    _ => {}
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *flow = ControlFlow::Exit,
            Event::LoopDestroyed => {
                if UNINSTALLED.load(Ordering::Relaxed) {
                    install::finish_uninstall();
                }
            }
            _ => {}
        }
    });
}
