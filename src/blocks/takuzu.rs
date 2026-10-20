//! Takuzu (binairo) : remplir la grille de 0 et de 1, jamais trois pareils à la suite, autant de 0
//! que de 1 par ligne et par colonne, et pas deux lignes (ni deux colonnes) identiques. Généré sur
//! place, à solution unique ; le numéro est la graine, comme pour le sudoku.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};

use super::sudoku::Difficulty;
use crate::doc::{Align, Doc, Style};
use crate::draw;

/// Une case : `None` si vide.
type Grid = Vec<Option<u8>>;

fn size(difficulty: Difficulty) -> usize {
    match difficulty {
        Difficulty::Easy => 6,
        Difficulty::Medium => 8,
        Difficulty::Hard => 10,
    }
}

/// Les cases d'une ligne (`row`) ou d'une colonne, dans l'ordre.
fn line(n: usize, k: usize, row: bool) -> Vec<usize> {
    (0..n).map(|i| if row { k * n + i } else { i * n + k }).collect()
}

/// La grille, même incomplète, respecte-t-elle les règles ?
fn valid(g: &Grid, n: usize) -> bool {
    let mut full_lines: Vec<Vec<u8>> = Vec::new();
    for row in [true, false] {
        full_lines.clear();
        for k in 0..n {
            let cells: Vec<Option<u8>> = line(n, k, row).into_iter().map(|i| g[i]).collect();
            for v in 0..2 {
                if cells.iter().filter(|c| **c == Some(v)).count() > n / 2 {
                    return false;
                }
            }
            if cells.windows(3).any(|w| w[0].is_some() && w[0] == w[1] && w[1] == w[2]) {
                return false;
            }
            if cells.iter().all(Option::is_some) {
                let full: Vec<u8> = cells.iter().map(|c| c.unwrap_or(0)).collect();
                if full_lines.contains(&full) {
                    return false;
                }
                full_lines.push(full);
            }
        }
    }
    true
}

/// Déductions immédiates (paires, trous entre deux pareils, lignes déjà équilibrées). `false` si
/// une contradiction apparaît.
fn propagate(g: &mut Grid, n: usize) -> bool {
    loop {
        let mut changed = false;
        for row in [true, false] {
            for k in 0..n {
                let idx = line(n, k, row);
                for i in 0..n {
                    if g[idx[i]].is_some() {
                        continue;
                    }
                    let at = |j: isize| (0..n as isize).contains(&j).then(|| g[idx[j as usize]]).flatten();
                    let i = i as isize;
                    let forced = [(at(i - 1), at(i - 2)), (at(i + 1), at(i + 2)), (at(i - 1), at(i + 1))]
                        .iter()
                        .find_map(|&(a, b)| (a.is_some() && a == b).then(|| 1 - a.unwrap_or(0)));
                    if let Some(v) = forced {
                        g[idx[i as usize]] = Some(v);
                        changed = true;
                    }
                }
                for v in 0..2u8 {
                    if idx.iter().filter(|&&i| g[i] == Some(v)).count() == n / 2 {
                        for &i in &idx {
                            if g[i].is_none() {
                                g[i] = Some(1 - v);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        if !valid(g, n) {
            return false;
        }
        if !changed {
            return true;
        }
    }
}

/// Nombre de solutions, plafonné à `limit`.
fn count(g: &Grid, n: usize, limit: usize) -> usize {
    let mut g = g.clone();
    if !propagate(&mut g, n) {
        return 0;
    }
    let Some(i) = g.iter().position(Option::is_none) else { return 1 };
    let mut total = 0;
    for v in 0..2 {
        let mut next = g.clone();
        next[i] = Some(v);
        total += count(&next, n, limit - total);
        if total >= limit {
            break;
        }
    }
    total
}

/// Une grille pleine au hasard.
fn fill(g: &mut Grid, n: usize, rng: &mut StdRng) -> bool {
    let Some(i) = g.iter().position(Option::is_none) else { return true };
    let mut values = [0u8, 1];
    values.shuffle(rng);
    for v in values {
        g[i] = Some(v);
        if valid(g, n) && fill(g, n, rng) {
            return true;
        }
    }
    g[i] = None;
    false
}

/// (grille à résoudre, solution, côté)
fn generate(seed: u64, difficulty: Difficulty) -> (Grid, Grid, usize) {
    let n = size(difficulty);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut solution = vec![None; n * n];
    fill(&mut solution, n, &mut rng);
    // On retire des chiffres tant que la solution reste unique ; en facile, on en laisse plus.
    let keep = match difficulty {
        Difficulty::Easy => n * n * 2 / 5,
        Difficulty::Medium => n * n / 4,
        Difficulty::Hard => 0,
    };
    let mut puzzle = solution.clone();
    let mut order: Vec<usize> = (0..n * n).collect();
    order.shuffle(&mut rng);
    for i in order {
        if puzzle.iter().filter(|c| c.is_some()).count() <= keep {
            break;
        }
        let v = puzzle[i].take();
        if count(&puzzle, n, 2) != 1 {
            puzzle[i] = v;
        }
    }
    (puzzle, solution, n)
}

fn render(grid: &Grid, n: usize) -> image::GrayImage {
    let cell = (480 / n as i64).min(60);
    let scale = if n <= 6 { 5 } else { 4 };
    let side = cell * n as i64;
    let (ox, oy) = ((512 - side) / 2, 3);
    let mut img = draw::canvas((side + 16) as u32);
    for k in 0..=n as i64 {
        let thick = if k == 0 || k == n as i64 { 5 } else { 2 };
        draw::fill_rect(&mut img, ox + k * cell - thick / 2, oy - 2, thick, side + 4);
        draw::fill_rect(&mut img, ox - 2, oy + k * cell - thick / 2, side + 4, thick);
    }
    for (i, v) in grid.iter().enumerate() {
        let Some(v) = v else { continue };
        let (r, c) = ((i / n) as i64, (i % n) as i64);
        let x = ox + c * cell + (cell - draw::DIGIT_W * scale) / 2;
        let y = oy + r * cell + (cell - draw::DIGIT_H * scale) / 2;
        draw::number(&mut img, u32::from(*v), x, y, scale);
    }
    img
}

pub fn build(difficulty: Difficulty, seed: Option<u64>, solution: bool) -> Doc {
    let seed = seed.unwrap_or_else(|| rand::rng().random_range(1..100_000));
    let (puzzle, solved, n) = generate(seed, difficulty);
    let mut doc = Doc::new();
    let title = if solution { crate::i18n::t("Solution du takuzu", "Takuzu solution") } else { "Takuzu" };
    doc.header(&format!("{title} · {}", difficulty.label()));
    if !solution {
        doc.text(
            crate::i18n::t(
                "Des 0 et des 1 : jamais trois pareils à la suite, autant de 0 que de 1 par ligne et par colonne, pas deux lignes ni deux colonnes identiques.",
                "0s and 1s: never three alike in a row, as many 0s as 1s in each row and column, no two identical rows or columns.",
            ),
            Style::default().small(),
        );
    }
    doc.feed(1);
    doc.image(render(if solution { &solved } else { &puzzle }, n));
    doc.text(&tr!("n° {seed}", "No. {seed}"), Style::default().small().align(Align::Right));
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puzzles_are_valid_and_unique() {
        for (seed, difficulty) in [(1, Difficulty::Easy), (42, Difficulty::Medium), (7, Difficulty::Hard)] {
            let (puzzle, solution, n) = generate(seed, difficulty);
            assert!(solution.iter().all(Option::is_some) && valid(&solution, n));
            assert!(puzzle.iter().zip(&solution).all(|(p, s)| p.is_none() || p == s));
            assert_eq!(count(&puzzle, n, 2), 1);
            assert!(puzzle.iter().any(Option::is_none));
        }
    }

    #[test]
    fn same_seed_same_grid() {
        assert_eq!(generate(9, Difficulty::Medium), generate(9, Difficulty::Medium));
    }
}
