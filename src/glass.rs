// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Intégration Windows : fenêtre sombre assortie à l'interface, sélecteur de
//! dossier, ouverture de liens dans le navigateur.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Fenêtre sombre aux coins arrondis, barre de titre teintée comme
/// l'interface. Pas de transparence réelle : l'effet de verre est dessiné par
/// l'interface, ce qui évite de voir les fenêtres placées derrière et coûte
/// moins cher au compositeur de Windows.
pub fn apply(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as _, 4);
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner as *const _ as _, 4);
        // COLORREF = 0x00BBGGRR : fond rgb(13, 14, 22), texte blanc cassé
        let caption: u32 = 0x0016_0E0D;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption as *const _ as _, 4);
        let text: u32 = 0x00F9_F3F1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &text as *const _ as _, 4);
        let border: u32 = 0x0030_2626;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &border as *const _ as _, 4);
    }
}

/// Boîte de sélection d'un dossier (Explorateur Windows).
pub fn pick_folder(owner: isize, titre: &str) -> Option<String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let opts = dlg.GetOptions().ok()?;
        dlg.SetOptions(opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        let title = HSTRING::from(titre);
        let _ = dlg.SetTitle(&title);
        dlg.Show(Some(HWND(owner as *mut _))).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s
    }
}

pub fn open_url(url: &str) {
    let u = HSTRING::from(url);
    let verb = HSTRING::from("open");
    unsafe {
        ShellExecuteW(None, &verb, &u, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}
