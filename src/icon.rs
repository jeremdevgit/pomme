// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
// Icône de Pomme dessinée par calcul (aucune image dans le dépôt) : une pomme
// en dégradé rose → violet sur un carré de verre. Partagé avec build.rs.

#[allow(dead_code)]
pub fn render_icon(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    const SS: u32 = 4; // suréchantillonnage pour des bords lisses
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / s;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / s;
                    let c = sample(x, y);
                    // composition « par-dessus » prémultipliée
                    let a = c[3];
                    for k in 0..3 {
                        acc[k] += c[k] * a;
                    }
                    acc[3] += a;
                }
            }
            let n = (SS * SS) as f32;
            let a = acc[3] / n;
            let i = ((py * size + px) * 4) as usize;
            if a > 0.0 {
                for k in 0..3 {
                    out[i + k] = ((acc[k] / acc[3]).clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
            out[i + 3] = (a.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    out
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Couleur RGBA (0..1) au point (x, y) de l'icône (coordonnées 0..1).
fn sample(x: f32, y: f32) -> [f32; 4] {
    // carré de verre arrondi
    let r = 0.22;
    let (cx, cy) = (x.clamp(r, 1.0 - r), y.clamp(r, 1.0 - r));
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    if d > r {
        return [0.0; 4];
    }
    let edge = r - d;
    let mut col = mix([0.10, 0.10, 0.17], [0.05, 0.05, 0.10], y);
    let mut alpha = 0.92;
    if edge < 0.012 {
        col = mix(col, [1.0, 1.0, 1.0], 0.35);
        alpha = 1.0;
    }
    // reflet du verre en haut
    if y < 0.42 {
        col = mix(col, [1.0, 1.0, 1.0], (0.42 - y) * 0.18);
    }

    // pomme : deux lobes, creux en haut, légère pointe en bas
    let lobe = |cx: f32| ((x - cx).powi(2) / 0.26f32.powi(2) + (y - 0.6).powi(2) / 0.27f32.powi(2)).sqrt();
    let notch = ((x - 0.5).powi(2) + (y - 0.31).powi(2)).sqrt() < 0.075;
    let bottom = ((x - 0.5).powi(2) + (y - 0.86).powi(2)).sqrt() < 0.05 && y > 0.84;
    let in_apple = (lobe(0.39) < 1.0 || lobe(0.61) < 1.0) && !notch && !bottom;
    // tige et feuille
    let stem = (x - 0.505).abs() < 0.018 && y > 0.2 && y < 0.34;
    let (lx, ly) = (x - 0.6, y - 0.22);
    let (rx, ry) = (lx * 0.8 + ly * 0.6, -lx * 0.6 + ly * 0.8);
    let leaf = (rx / 0.11).powi(2) + (ry / 0.045).powi(2) < 1.0;

    if in_apple {
        let t = ((y - 0.3) / 0.6).clamp(0.0, 1.0);
        let mut c = if t < 0.5 {
            mix([1.0, 0.24, 0.45], [0.95, 0.30, 0.85], t * 2.0)
        } else {
            mix([0.95, 0.30, 0.85], [0.49, 0.36, 1.0], (t - 0.5) * 2.0)
        };
        // brillance liquide
        let hl = ((x - 0.4).powi(2) + (y - 0.48).powi(2)).sqrt();
        if hl < 0.12 {
            c = mix(c, [1.0, 1.0, 1.0], (0.12 - hl) / 0.12 * 0.45);
        }
        return [c[0], c[1], c[2], 1.0];
    }
    if leaf {
        return [0.25, 0.9, 0.55, 1.0];
    }
    if stem {
        return [0.85, 0.85, 0.9, 1.0];
    }
    [col[0], col[1], col[2], alpha]
}

/// Fichier .ico multi-tailles (PNG intégrés non compressés en BMP).
#[allow(dead_code)]
pub fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let mut images = Vec::new();
    for &sz in sizes {
        let rgba = render_icon(sz);
        // BMP 32 bits de bas en haut + masque AND
        let mut bmp = Vec::new();
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(sz as i32).to_le_bytes());
        bmp.extend_from_slice(&((sz * 2) as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0u8; 24]);
        for y in (0..sz).rev() {
            for x in 0..sz {
                let i = ((y * sz + x) * 4) as usize;
                bmp.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        let mask_row = sz.div_ceil(32) * 4;
        bmp.extend(std::iter::repeat(0u8).take((mask_row * sz) as usize));
        images.push((sz, bmp));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (sz, data) in &images {
        let d = if *sz >= 256 { 0 } else { *sz as u8 };
        out.extend_from_slice(&[d, d, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend_from_slice(&data);
    }
    out
}
