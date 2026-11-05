# SPDX-License-Identifier: GPL-3.0-or-later - Copyright (C) 2026 jeremdevgit
# Construit la version distribuee de Pomme (dist\Pomme-Setup.exe, le meme executable sert
# d'installateur quand son nom contient "Setup"), puis refuse de la livrer au moindre ecart :
#  - aucun chemin ni identifiant de la machine de compilation dans le binaire ;
#  - aucune dependance au Runtime Visual C++ ;
#  - aucun code de test.
# Valeurs privees supplementaires a chercher : une par ligne dans
# %LOCALAPPDATA%\BoiteAOutils-build\denylist.txt (hors du projet, jamais distribue).
# Les versions publiees sur GitHub sont construites par .github/workflows/windows.yml avec ce script.
# -Ci (GitHub Actions) : seuls les chemins et noms de la machine de construction sont cherches.
param([string]$Out = (Join-Path (Split-Path $PSScriptRoot -Parent) 'dist'), [switch]$Ci)
$Name = 'Pomme'
$TestMarkers = @()
$ErrorActionPreference = 'Stop'
$projet = Split-Path $PSScriptRoot -Parent
$target = 'x86_64-pc-windows-msvc'

# Chemins neutres dans les messages internes de Rust ; memes options pour les trois logiciels
$registry = Join-Path $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }) 'registry\src'
$rustup = $(if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' })
function Build($dir, $name) {
  # la derniere regle qui correspond l'emporte : du plus general au plus precis
  $env:CARGO_ENCODED_RUSTFLAGS = @('-Ctarget-feature=+crt-static', "--remap-path-prefix=$env:USERPROFILE=/home", "--remap-path-prefix=$registry=/cargo", "--remap-path-prefix=$rustup=/rustup", "--remap-path-prefix=$dir=/src") -join [char]0x1f
  try {
    $p = Start-Process cargo -ArgumentList 'build', '--release', '--locked', '--target', $target -WorkingDirectory $dir -NoNewWindow -Wait -PassThru
    if ($p.ExitCode) { throw "echec de la compilation de $name" }
  } finally { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue }
  Join-Path $dir "target\$target\release\$name.exe"
}

# -- controles ------------------------------------------------------------
function Identifiers {
  $v = New-Object System.Collections.ArrayList
  $add = { param($s) $s = ([string]$s).Trim(); if ($s.Length -ge 4 -and $s -notmatch '^(To be filled|Default string|None|Unknown|0+$|System Serial)') { [void]$v.Add($s) } }
  & $add $env:USERNAME; & $add $env:COMPUTERNAME; & $add $env:USERPROFILE
  if ($Ci) { & $add $env:GITHUB_WORKSPACE; & $add $env:RUNNER_TEMP; return $v | Sort-Object -Unique }
  $cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
  & $add $cv.RegisteredOwner; & $add $cv.ProductId
  & $add ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value)
  $b = Get-CimInstance Win32_BaseBoard; & $add $b.SerialNumber; & $add $b.Product
  & $add (Get-CimInstance Win32_ComputerSystem).Model
  & $add (Get-CimInstance Win32_BIOS).SerialNumber
  foreach ($d in Get-CimInstance Win32_DiskDrive) { & $add $d.SerialNumber }
  foreach ($r in Get-CimInstance Win32_PhysicalMemory) { & $add $r.SerialNumber; & $add $r.PartNumber }
  foreach ($n in Get-NetAdapter -ErrorAction SilentlyContinue) { & $add $n.MacAddress; & $add ($n.MacAddress -replace '-', ':') }
  foreach ($i in Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue) { if ($i.IPAddress -notmatch '^(127|169)\.') { & $add $i.IPAddress } }
  $extra = Join-Path $env:LOCALAPPDATA 'BoiteAOutils-build\denylist.txt'
  if (Test-Path $extra) { foreach ($l in Get-Content $extra -Encoding UTF8) { & $add $l } }
  $v | Sort-Object -Unique
}

Add-Type -TypeDefinition @'
using System; using System.Text;
public static class BaoScan {
  public static bool Has(byte[] h, string s, Encoding e) {
    byte[] lo = e.GetBytes(s.ToLowerInvariant()), up = e.GetBytes(s.ToUpperInvariant());
    if (lo.Length != up.Length || lo.Length == 0) return false;
    for (int i = 0; i <= h.Length - lo.Length; i++) {
      if (h[i] != lo[0] && h[i] != up[0]) continue;
      int j = 1; while (j < lo.Length && (h[i + j] == lo[j] || h[i + j] == up[j])) j++;
      if (j == lo.Length) return true;
    }
    return false;
  }
}
'@

$dumpbin = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio", "$env:ProgramFiles\Microsoft Visual Studio" -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue | Where-Object FullName -match 'Hostx64\\x64' | Select-Object -First 1
if (-not $dumpbin) { throw 'dumpbin introuvable : dependances non verifiables' }

function Check($exe, $ids) {
  $bytes = [IO.File]::ReadAllBytes($exe)
  $bad = @()
  foreach ($s in $ids) {
    foreach ($e in [Text.Encoding]::UTF8, [Text.Encoding]::Unicode) {
      if ([BaoScan]::Has($bytes, $s, $e)) { $bad += "$s ($($e.WebName))"; break }
    }
  }
  foreach ($s in @($TestMarkers) + '.cargo\registry') {
    if ([BaoScan]::Has($bytes, $s, [Text.Encoding]::UTF8)) { $bad += "code de test ou chemin : $s" }
  }
  if ($bad) { throw "$exe contient encore :`n  $($bad -join "`n  ")" }
  $deps = @(& $dumpbin.FullName /dependents $exe | Select-String '\.dll' | ForEach-Object { $_.Line.Trim() } | Where-Object { $_ -notmatch 'Dump of' })
  $crt = $deps | Where-Object { $_ -match '(?i)^(vcruntime|msvcp|ucrtbase|api-ms-win-crt)' }
  if ($crt) { throw "$exe depend encore du Runtime Visual C++ : $($crt -join ', ')" }
  Write-Host "  OK $([IO.Path]::GetFileName($exe)) : $($ids.Count) identifiants absents ; DLL : $($deps -join ' ')"
}

# -- construction ---------------------------------------------------------
$ids = @(Identifiers)
$exe = Build $projet $Name
Check $exe $ids

New-Item -ItemType Directory -Force $Out | Out-Null
$setup = Join-Path $Out "$Name-Setup.exe"
Copy-Item $exe $setup -Force
"$((Get-FileHash $setup -Algorithm SHA256).Hash.ToLower())  $Name-Setup.exe" | Set-Content (Join-Path $Out 'SHA256SUMS.txt') -Encoding ascii
Write-Host "Livre dans $Out : $((Get-Item $setup).Length) octets, SHA-256 $((Get-FileHash $setup).Hash)"
