#![allow(dead_code)] // boîte à outils commune avec rJb : tout n'est pas utilisé ici
// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Petites fonctions Windows partagées : registre, dossiers connus,
//! raccourcis, boîtes de dialogue, processus.

use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Com::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_w(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }
}

// ── outils système ──────────────────────────────────────────────────────

/// Chemin complet d'un outil de System32 : jamais résolu par le PATH ni par
/// le dossier courant, pour qu'un exécutable déposé à côté de rJb ne puisse
/// pas se faire passer pour lui.
pub fn system_tool(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    root.join("System32").join(name)
}

/// Lance un outil sans fenêtre et attend son code de sortie.
pub fn run_hidden(tool: &str, args: &[&str]) -> Option<i32> {
    std::process::Command::new(system_tool(tool))
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .and_then(|s| s.code())
}

/// Lance `file` en administrateur (invite UAC) et attend sa fin si demandé.
/// Renvoie le code de sortie, ou None si l'utilisateur a refusé.
pub fn run_elevated(file: &Path, params: &str, wait: bool) -> Option<u32> {
    unsafe {
        let verb = w("runas");
        let f = w(&file.to_string_lossy());
        let p = w(params);
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = f.as_ptr();
        info.lpParameters = p.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            return None;
        }
        let mut code = 0u32;
        if !info.hProcess.is_null() {
            if wait {
                windows_sys::Win32::System::Threading::WaitForSingleObject(info.hProcess, u32::MAX);
                windows_sys::Win32::System::Threading::GetExitCodeProcess(info.hProcess, &mut code);
            }
            CloseHandle(info.hProcess);
        }
        Some(code)
    }
}

/// Ouvre un dossier ou une adresse avec le programme par défaut.
pub fn shell_open(target: &str) {
    let verb = w("open");
    let t = w(target);
    unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), t.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
    }
}

pub fn message(title: &str, text: &str, error: bool) {
    let t = w(title);
    let m = w(text);
    let flags = if error { MB_OK | MB_ICONERROR } else { MB_OK | MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), flags) };
}

// ── registre ────────────────────────────────────────────────────────────

pub fn reg_set_str(root: HKEY, key: &str, name: &str, value: &str) -> bool {
    let k = w(key);
    let n = w(name);
    let v = w(value);
    unsafe { RegSetKeyValueW(root, k.as_ptr(), n.as_ptr(), REG_SZ, v.as_ptr() as _, (v.len() * 2) as u32) == 0 }
}

pub fn reg_set_dword(root: HKEY, key: &str, name: &str, value: u32) -> bool {
    let k = w(key);
    let n = w(name);
    unsafe { RegSetKeyValueW(root, k.as_ptr(), n.as_ptr(), REG_DWORD, &value as *const u32 as _, 4) == 0 }
}

pub fn reg_get_str(root: HKEY, key: &str, name: &str) -> Option<String> {
    let k = w(key);
    let n = w(name);
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(root, k.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr() as _, &mut size)
    };
    (r == 0).then(|| from_w(buf.as_ptr()))
}

pub fn reg_get_dword(root: HKEY, key: &str, name: &str) -> Option<u32> {
    let k = w(key);
    let n = w(name);
    let mut v = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(root, k.as_ptr(), n.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), &mut v as *mut u32 as _, &mut size)
    };
    (r == 0).then_some(v)
}

pub fn reg_delete_value(root: HKEY, key: &str, name: &str) {
    let k = w(key);
    let n = w(name);
    unsafe { RegDeleteKeyValueW(root, k.as_ptr(), n.as_ptr()) };
}

pub fn reg_delete_tree(root: HKEY, key: &str) {
    let k = w(key);
    unsafe {
        RegDeleteTreeW(root, k.as_ptr());
        RegDeleteKeyW(root, k.as_ptr());
    }
}

// ── dossiers connus ─────────────────────────────────────────────────────

pub fn known_folder(id: &GUID) -> Option<PathBuf> {
    unsafe {
        let mut p: *mut u16 = std::ptr::null_mut();
        let r = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p);
        let s = from_w(p);
        CoTaskMemFree(p as _);
        (r == 0 && !s.is_empty()).then(|| PathBuf::from(s))
    }
}

pub fn program_files() -> PathBuf {
    known_folder(&FOLDERID_ProgramFiles).unwrap_or_else(|| PathBuf::from("C:\\Program Files"))
}

// ── raccourcis (.lnk) via IShellLinkW ───────────────────────────────────

const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
const IID_SHELL_LINK_W: GUID = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
const IID_PERSIST_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);

type Raw = *mut c_void;

/// Table virtuelle d'IShellLinkW, dans l'ordre de déclaration du SDK.
#[repr(C)]
struct ShellLinkVtbl {
    query_interface: unsafe extern "system" fn(Raw, *const GUID, *mut Raw) -> i32,
    add_ref: usize,
    release: unsafe extern "system" fn(Raw) -> u32,
    get_path: usize,
    get_id_list: usize,
    set_id_list: usize,
    get_description: usize,
    set_description: unsafe extern "system" fn(Raw, *const u16) -> i32,
    get_working_directory: usize,
    set_working_directory: unsafe extern "system" fn(Raw, *const u16) -> i32,
    get_arguments: usize,
    set_arguments: unsafe extern "system" fn(Raw, *const u16) -> i32,
    get_hotkey: usize,
    set_hotkey: usize,
    get_show_cmd: usize,
    set_show_cmd: usize,
    get_icon_location: usize,
    set_icon_location: unsafe extern "system" fn(Raw, *const u16, i32) -> i32,
    set_relative_path: usize,
    resolve: usize,
    set_path: unsafe extern "system" fn(Raw, *const u16) -> i32,
}

#[repr(C)]
struct PersistFileVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(Raw) -> u32,
    get_class_id: usize,
    is_dirty: usize,
    load: usize,
    save: unsafe extern "system" fn(Raw, *const u16, i32) -> i32,
}

/// Crée (ou remplace) un raccourci `lnk` vers `target`.
pub fn create_shortcut(lnk: &Path, target: &Path, args: &str, description: &str) -> bool {
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let mut link: Raw = std::ptr::null_mut();
        if CoCreateInstance(&CLSID_SHELL_LINK, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_SHELL_LINK_W, &mut link) != 0 || link.is_null() {
            return false;
        }
        let vt = &**(link as *mut *const ShellLinkVtbl);
        let t = w(&target.to_string_lossy());
        let dir = w(&target.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
        let a = w(args);
        let d = w(description);
        (vt.set_path)(link, t.as_ptr());
        (vt.set_working_directory)(link, dir.as_ptr());
        (vt.set_arguments)(link, a.as_ptr());
        (vt.set_description)(link, d.as_ptr());
        (vt.set_icon_location)(link, t.as_ptr(), 0);
        let mut pf: Raw = std::ptr::null_mut();
        let mut ok = false;
        if (vt.query_interface)(link, &IID_PERSIST_FILE, &mut pf) == 0 && !pf.is_null() {
            let pvt = &**(pf as *mut *const PersistFileVtbl);
            if let Some(d) = lnk.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let p = w(&lnk.to_string_lossy());
            ok = (pvt.save)(pf, p.as_ptr(), 1) == 0;
            (pvt.release)(pf);
        }
        (vt.release)(link);
        ok
    }
}

/// Taille de fenêtre (en points) ramenée à 92 % de la zone de travail de
/// l'écran : avec une mise à l'échelle de 150 % sur un écran 1080p, une
/// fenêtre trop haute aurait ses boutons hors de l'écran.
pub fn fit_window(w: f32, h: f32) -> [f32; 2] {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as _, 0) == 0 {
            return [w, h];
        }
        let scale = windows_sys::Win32::UI::HiDpi::GetDpiForSystem().max(96) as f32 / 96.0;
        let mw = (r.right - r.left) as f32 / scale * 0.92;
        let mh = (r.bottom - r.top) as f32 / scale * 0.92;
        [w.min(mw), h.min(mh)]
    }
}

// ── boîtes de dialogue ──────────────────────────────────────────────────

/// Sélecteur de dossier ; renvoie None si annulé.
pub fn pick_folder(title: &str) -> Option<PathBuf> {
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let t = w(title);
        let mut bi: BROWSEINFOW = std::mem::zeroed();
        bi.hwndOwner = GetForegroundWindow();
        bi.lpszTitle = t.as_ptr();
        bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
        let pidl = SHBrowseForFolderW(&bi);
        if pidl.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let ok = SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0;
        CoTaskMemFree(pidl as _);
        ok.then(|| PathBuf::from(from_w(buf.as_ptr())))
    }
}

/// Boîte « Enregistrer sous » / « Ouvrir » pour un fichier JSON.
pub fn pick_json(save: bool, default_name: &str) -> Option<PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "Configuration rJb (*.json)\0*.json\0Tous les fichiers\0*.*\0\0".encode_utf16().collect();
    let ext = w("json");
    let mut buf = [0u16; 1024];
    for (i, c) in default_name.encode_utf16().take(1000).enumerate() {
        buf[i] = c;
    }
    unsafe {
        let mut o: OPENFILENAMEW = std::mem::zeroed();
        o.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        // boîte modale sur notre fenêtre (sinon elle peut s'ouvrir derrière)
        o.hwndOwner = GetForegroundWindow();
        o.lpstrFilter = filter.as_ptr();
        o.lpstrFile = buf.as_mut_ptr();
        o.nMaxFile = buf.len() as u32;
        o.lpstrDefExt = ext.as_ptr();
        o.Flags = OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | if save { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST };
        let ok = if save { GetSaveFileNameW(&mut o) } else { GetOpenFileNameW(&mut o) };
        (ok != 0).then(|| PathBuf::from(from_w(buf.as_ptr())))
    }
}

// ── processus ───────────────────────────────────────────────────────────

/// Noms (en minuscules) des processus en cours.
pub fn running_processes() -> Vec<String> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push(String::from_utf16_lossy(&e.szExeFile[..n]).to_lowercase());
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out
}
