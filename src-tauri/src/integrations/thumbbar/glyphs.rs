//! Icônes des boutons de la miniature, dessinées en code (#333) : pas de fichier `.ico`,
//! taille au DPI du système et couleur au thème de la barre des tâches. Logique pure, testée.

/// Les quatre symboles de la barre d'outils de la miniature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Previous,
    Play,
    Pause,
    Next,
}

/// Formes dans un carré unité (x vers la droite, y vers le bas).
enum Shape {
    Triangle([(f32, f32); 3]),
    Rect { x0: f32, y0: f32, x1: f32, y1: f32 },
}

impl Shape {
    fn contains(&self, x: f32, y: f32) -> bool {
        match *self {
            Shape::Triangle([a, b, c]) => {
                let edge = |p: (f32, f32), q: (f32, f32)| {
                    (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0)
                };
                let (d1, d2, d3) = (edge(a, b), edge(b, c), edge(c, a));
                let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                !(negative && positive)
            }
            Shape::Rect { x0, y0, x1, y1 } => x >= x0 && x <= x1 && y >= y0 && y <= y1,
        }
    }
}

fn shapes(glyph: Glyph) -> Vec<Shape> {
    match glyph {
        Glyph::Play => vec![Shape::Triangle([(0.27, 0.18), (0.27, 0.82), (0.82, 0.5)])],
        Glyph::Pause => vec![
            Shape::Rect {
                x0: 0.24,
                y0: 0.2,
                x1: 0.42,
                y1: 0.8,
            },
            Shape::Rect {
                x0: 0.58,
                y0: 0.2,
                x1: 0.76,
                y1: 0.8,
            },
        ],
        Glyph::Next => vec![
            Shape::Triangle([(0.18, 0.22), (0.18, 0.78), (0.64, 0.5)]),
            Shape::Rect {
                x0: 0.68,
                y0: 0.22,
                x1: 0.8,
                y1: 0.78,
            },
        ],
        Glyph::Previous => vec![
            Shape::Rect {
                x0: 0.2,
                y0: 0.22,
                x1: 0.32,
                y1: 0.78,
            },
            Shape::Triangle([(0.82, 0.22), (0.82, 0.78), (0.36, 0.5)]),
        ],
    }
}

/// Sous-échantillons par côté de pixel, pour l'anticrénelage.
const SAMPLES: u32 = 4;

/// Pixels BGRA de `size` × `size`, de haut en bas, alpha non prémultiplié (format des
/// icônes 32 bits) : `rgb` partout, opacité selon la couverture du symbole.
pub fn render(glyph: Glyph, size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let shapes = shapes(glyph);
    let total = SAMPLES * SAMPLES;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut covered = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    if shapes.iter().any(|s| s.contains(x, y)) {
                        covered += 1;
                    }
                }
            }
            let alpha = (covered * 255 + total / 2) / total;
            pixels.extend_from_slice(&[rgb[2], rgb[1], rgb[0], alpha as u8]);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(pixels: &[u8], size: u32, x: u32, y: u32) -> u8 {
        pixels[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn glyphs_are_drawn_where_expected() {
        let size = 16;
        let play = render(Glyph::Play, size, [255, 255, 255]);
        assert_eq!(play.len(), (size * size * 4) as usize);
        // Coins vides, centre plein.
        assert_eq!(alpha(&play, size, 0, 0), 0);
        assert_eq!(alpha(&play, size, 15, 15), 0);
        assert_eq!(alpha(&play, size, 7, 8), 255);

        // Pause : deux barres séparées par un vide au centre.
        let pause = render(Glyph::Pause, size, [255, 255, 255]);
        assert_eq!(alpha(&pause, size, 5, 8), 255);
        assert_eq!(alpha(&pause, size, 10, 8), 255);
        assert_eq!(alpha(&pause, size, 8, 8), 0);
    }

    #[test]
    fn previous_mirrors_next() {
        let size = 20;
        let next = render(Glyph::Next, size, [0, 0, 0]);
        let previous = render(Glyph::Previous, size, [0, 0, 0]);
        let mut diff = 0;
        for y in 0..size {
            for x in 0..size {
                let a = i32::from(alpha(&next, size, x, y));
                let b = i32::from(alpha(&previous, size, size - 1 - x, y));
                diff = diff.max((a - b).abs());
            }
        }
        // Formes symétriques à l'arrondi près.
        assert!(diff <= 64, "écart {diff}");
    }

    #[test]
    fn color_is_stored_as_bgra() {
        let pixels = render(Glyph::Play, 8, [10, 20, 30]);
        assert_eq!(&pixels[..3], &[30, 20, 10]);
    }
}
