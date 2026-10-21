//! Pictogrammes météo dessinés en traits (soleil, nuages, pluie, neige, orage, brouillard), d'après
//! les codes WMO d'Open-Meteo : nets en noir et blanc à toutes les tailles, sans image à télécharger.

use image::GrayImage;

/// Le ciel d'un code WMO, réduit à ce qu'un pictogramme sait montrer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sky {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Storm,
}

impl Sky {
    pub fn from_code(code: u8) -> Sky {
        match code {
            0 => Sky::Clear,
            1 | 2 => Sky::PartlyCloudy,
            45 | 48 => Sky::Fog,
            51..=57 => Sky::Drizzle,
            61..=67 | 80..=82 => Sky::Rain,
            71..=77 | 85 | 86 => Sky::Snow,
            95..=99 => Sky::Storm,
            _ => Sky::Cloudy,
        }
    }
}

/// Le pictogramme dans un carré de `side` points.
pub fn render(sky: Sky, side: u32) -> GrayImage {
    // Trait d'au moins 3 points, plus épais sur les grands pictogrammes.
    let stroke = (6.0 / side as f64).max(0.075);
    let mut img = GrayImage::from_pixel(side, side, crate::draw::WHITE);
    for py in 0..side {
        for px in 0..side {
            let x = (px as f64 + 0.5) / side as f64 * 2.0 - 1.0;
            let y = (py as f64 + 0.5) / side as f64 * 2.0 - 1.0;
            if ink(sky, x, y, stroke) {
                img.put_pixel(px, py, crate::draw::BLACK);
            }
        }
    }
    img
}

/// Le point (x, y), dans [-1, 1]² (y vers le bas), est-il noir ?
fn ink(sky: Sky, x: f64, y: f64, w: f64) -> bool {
    // Nuage seul, ou remonté pour laisser la place à ce qui tombe.
    let (low, high) = ((0.0, 0.0, 1.15), (0.0, -0.2, 0.9));
    match sky {
        Sky::Clear => sun(x, y, w, (0.0, 0.0, 1.0)),
        Sky::PartlyCloudy => {
            let cloud_at = (0.18, 0.28, 0.82);
            match cloud(x, y, w, cloud_at) {
                Some(edge) => edge,
                None => sun(x, y, w, (-0.26, -0.26, 0.78)),
            }
        }
        Sky::Cloudy => cloud(x, y, w, low) == Some(true),
        Sky::Fog => {
            let lines = [(-0.45, -0.75, 0.75), (-0.15, -0.55, 0.85), (0.15, -0.85, 0.6), (0.45, -0.6, 0.75)];
            lines.iter().any(|&(ly, x0, x1)| segment(x, y, (x0, ly), (x1, ly)) <= w * 0.6)
        }
        Sky::Drizzle => {
            cloud(x, y, w, high) == Some(true)
                || [(-0.35, 0.62), (0.05, 0.62), (0.45, 0.62), (-0.15, 0.86), (0.25, 0.86)]
                    .iter()
                    .any(|&(cx, cy)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() <= w * 0.9)
        }
        Sky::Rain => {
            cloud(x, y, w, high) == Some(true)
                || [-0.32, 0.05, 0.42].iter().any(|&cx| segment(x, y, (cx, 0.55), (cx - 0.14, 0.92)) <= w * 0.55)
        }
        Sky::Snow => {
            cloud(x, y, w, high) == Some(true)
                || [(-0.42, 0.7), (0.0, 0.8), (0.42, 0.7)].iter().any(|&(cx, cy)| flake(x, y, w, cx, cy))
        }
        Sky::Storm => cloud(x, y, w, high) == Some(true) || bolt(x, y),
    }
}

/// Soleil centré en (ox, oy), à l'échelle s : cercle et huit rayons.
fn sun(x: f64, y: f64, w: f64, (ox, oy, s): (f64, f64, f64)) -> bool {
    let (x, y, w) = ((x - ox) / s, (y - oy) / s, w / s);
    let r = (x * x + y * y).sqrt();
    if (r - 0.42).abs() <= w / 2.0 {
        return true;
    }
    (0..8).any(|k| {
        let a = k as f64 * std::f64::consts::PI / 4.0;
        let (c, sn) = (a.cos(), a.sin());
        segment(x, y, (0.6 * c, 0.6 * sn), (0.86 * c, 0.86 * sn)) <= w / 2.0
    })
}

/// Nuage posé en (ox, oy), à l'échelle s : `Some(true)` sur son contour, `Some(false)` dedans
/// (blanc : il cache le soleil), `None` dehors.
fn cloud(x: f64, y: f64, w: f64, (ox, oy, s): (f64, f64, f64)) -> Option<bool> {
    let (x, y, w) = ((x - ox) / s, (y - oy) / s, w / s);
    let inside = |grow: f64| {
        let bumps = [(-0.45, 0.12, 0.3), (-0.05, -0.12, 0.42), (0.45, 0.12, 0.3)];
        bumps.iter().any(|&(cx, cy, r)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() <= r + grow)
            || ((-0.45..=0.45).contains(&x) && (0.12..=0.42 + grow).contains(&y))
    };
    if !inside(w / 2.0) { None } else { Some(!inside(-w / 2.0)) }
}

/// Flocon : trois traits croisés autour de (cx, cy).
fn flake(x: f64, y: f64, w: f64, cx: f64, cy: f64) -> bool {
    (0..3).any(|k| {
        let a = k as f64 * std::f64::consts::PI / 3.0 + std::f64::consts::PI / 2.0;
        let (dx, dy) = (0.17 * a.cos(), 0.17 * a.sin());
        segment(x, y, (cx - dx, cy - dy), (cx + dx, cy + dy)) <= w * 0.45
    })
}

/// Éclair plein, sous le nuage.
fn bolt(x: f64, y: f64) -> bool {
    const POINTS: [(f64, f64); 7] =
        [(0.12, 0.3), (-0.26, 0.7), (-0.02, 0.7), (-0.16, 1.0), (0.3, 0.56), (0.06, 0.56), (0.28, 0.3)];
    let mut inside = false;
    for k in 0..POINTS.len() {
        let ((x1, y1), (x2, y2)) = (POINTS[k], POINTS[(k + 1) % POINTS.len()]);
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

/// Distance du point (x, y) au segment [a, b].
fn segment(x: f64, y: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((x - a.0 - t * dx).powi(2) + (y - a.1 - t * dy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sky_draws_something_inside_its_square() {
        for sky in
            [Sky::Clear, Sky::PartlyCloudy, Sky::Cloudy, Sky::Fog, Sky::Drizzle, Sky::Rain, Sky::Snow, Sky::Storm]
        {
            for side in [48, 150] {
                let img = render(sky, side);
                let black = img.pixels().filter(|p| p.0[0] == 0).count();
                let area = (side * side) as usize;
                assert!(black > area / 50 && black < area / 2, "{sky:?} {side} : {black} points noirs");
            }
        }
    }

    #[test]
    fn codes_map_to_skies() {
        assert_eq!(Sky::from_code(0), Sky::Clear);
        assert_eq!(Sky::from_code(2), Sky::PartlyCloudy);
        assert_eq!(Sky::from_code(3), Sky::Cloudy);
        assert_eq!(Sky::from_code(63), Sky::Rain);
        assert_eq!(Sky::from_code(81), Sky::Rain);
        assert_eq!(Sky::from_code(75), Sky::Snow);
        assert_eq!(Sky::from_code(96), Sky::Storm);
    }
}
