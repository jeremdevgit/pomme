// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
// Icône .ico générée par calcul, informations de version et manifeste.

include!("src/icon.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out.join("pomme.ico");
    std::fs::write(&ico, ico_bytes(&[16, 20, 24, 32, 48, 64, 256])).unwrap();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        // Cross-compilation depuis Linux : utiliser le compilateur MinGW du target.
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") && cfg!(unix) {
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        res.set_icon(ico.to_str().unwrap());
        res.set("ProductName", "Pomme");
        res.set(
            "FileDescription",
            "Pomme - lecteur Apple Music et musique locale",
        );
        res.set("CompanyName", "jeremdevgit");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 jeremdevgit - logiciel libre sous licence GPL-3.0-or-later",
        );
        res.set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
<requestedExecutionLevel level="asInvoker" uiAccess="false"/>
</requestedPrivileges></security></trustInfo>
<application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
<dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
</windowsSettings></application>
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>"#,
        );
        res.compile().expect("ressources Windows");
        // DLL importées cherchées dans System32 uniquement (installateur
        // lancé depuis Téléchargements)
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
}
