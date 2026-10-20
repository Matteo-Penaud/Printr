//! Kakuro : des mots croisés de chiffres. Chaque tranche de cases blanches doit faire la somme
//! indiquée (en haut à droite pour l'horizontale, en bas à gauche pour la verticale), avec des
//! chiffres de 1 à 9 tous différents. Généré sur place, à solution unique : si les sommes seules en
//! laissent plusieurs, quelques chiffres sont donnés. Le numéro est la graine.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};

use super::sudoku::Difficulty;
use crate::doc::{Align, Doc, Style};
use crate::draw;

fn size(difficulty: Difficulty) -> usize {
    match difficulty {
        Difficulty::Easy => 6,
        Difficulty::Medium => 8,
        Difficulty::Hard => 9,
    }
}

/// Une tranche : ses cases et sa somme.
#[derive(Clone, Debug, PartialEq)]
struct Run {
    cells: Vec<usize>,
    sum: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct Puzzle {
    n: usize,
    white: Vec<bool>,
    /// Chiffres donnés (0 : à trouver).
    given: Vec<u8>,
    solution: Vec<u8>,
    across: Vec<Run>,
    down: Vec<Run>,
}

/// Forme de la grille : première ligne et première colonne noires, des cases noires au hasard
/// (symétriques), et pas de case blanche seule dans sa tranche.
fn pattern(n: usize, rng: &mut StdRng) -> Vec<bool> {
    loop {
        let mut white = vec![true; n * n];
        for k in 0..n {
            white[k] = false;
            white[k * n] = false;
        }
        for r in 1..n {
            for c in 1..n {
                if rng.random_range(0..100) < 22 {
                    white[r * n + c] = false;
                    // Symétrie centrale de la partie jouable.
                    let (sr, sc) = (n - r, n - c);
                    if sr >= 1 && sc >= 1 && sr < n && sc < n {
                        white[sr * n + sc] = false;
                    }
                }
            }
        }
        // Une case blanche doit être dans une tranche d'au moins deux, dans les deux sens.
        loop {
            let mut changed = false;
            for i in 0..n * n {
                if !white[i] {
                    continue;
                }
                let (r, c) = (i / n, i % n);
                let h = c > 0 && white[i - 1] || c + 1 < n && white[i + 1];
                let v = r > 0 && white[i - n] || r + 1 < n && white[i + n];
                if !h || !v {
                    white[i] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let count = white.iter().filter(|w| **w).count();
        if count >= (n - 1) * (n - 1) / 2 {
            return white;
        }
    }
}

/// Les tranches horizontales et verticales (sommes à zéro).
fn runs(white: &[bool], n: usize) -> (Vec<Run>, Vec<Run>) {
    let mut across = Vec::new();
    let mut down = Vec::new();
    for row in [true, false] {
        for k in 0..n {
            let mut current: Vec<usize> = Vec::new();
            for i in 0..=n {
                let idx = if row { k * n + i } else { i * n + k };
                if i < n && white[idx] {
                    current.push(idx);
                } else if !current.is_empty() {
                    let run = Run { cells: std::mem::take(&mut current), sum: 0 };
                    if row { across.push(run) } else { down.push(run) }
                }
            }
        }
    }
    (across, down)
}

/// Pour chaque case, ses deux tranches (horizontale, verticale).
fn membership(n: usize, across: &[Run], down: &[Run]) -> Vec<(usize, usize)> {
    let mut of = vec![(usize::MAX, usize::MAX); n * n];
    for (k, run) in across.iter().enumerate() {
        for &i in &run.cells {
            of[i].0 = k;
        }
    }
    for (k, run) in down.iter().enumerate() {
        for &i in &run.cells {
            of[i].1 = k;
        }
    }
    of
}

/// Remplit les cases blanches au hasard, sans chiffre répété dans une tranche.
fn fill(
    grid: &mut [u8],
    order: &[usize],
    of: &[(usize, usize)],
    across: &[Run],
    down: &[Run],
    rng: &mut StdRng,
) -> bool {
    let Some((&i, rest)) = order.split_first() else { return true };
    let used = |run: &Run, grid: &[u8]| run.cells.iter().map(|&j| grid[j]).collect::<Vec<u8>>();
    let (a, d) = (used(&across[of[i].0], grid), used(&down[of[i].1], grid));
    let mut digits: Vec<u8> = (1..=9).filter(|v| !a.contains(v) && !d.contains(v)).collect();
    digits.shuffle(rng);
    for v in digits {
        grid[i] = v;
        if fill(grid, rest, of, across, down, rng) {
            return true;
        }
    }
    grid[i] = 0;
    false
}

/// Les chiffres (bits 1 à 9) encore possibles dans une tranche : ceux d'une combinaison de la
/// bonne longueur et de la bonne somme qui contient déjà les chiffres posés.
fn run_candidates(run: &Run, grid: &[u8]) -> u16 {
    let mut used = 0u16;
    for &i in &run.cells {
        if grid[i] != 0 {
            used |= 1 << grid[i];
        }
    }
    let mut out = 0u16;
    for combo in (0u16..512).map(|m| m << 1) {
        let digits = combo.count_ones() as usize;
        let sum: u32 = (1..=9).filter(|d| combo & (1 << d) != 0).sum();
        if digits == run.cells.len() && sum == run.sum && combo & used == used {
            out |= combo & !used;
        }
    }
    out
}

/// Résout la grille (chiffres donnés déjà posés) ; garde au plus `limit` solutions. Toujours la
/// case la plus contrainte d'abord.
fn solve(grid: &mut Vec<u8>, p: &Puzzle, of: &[(usize, usize)], limit: usize, out: &mut Vec<Vec<u8>>) {
    if out.len() >= limit {
        return;
    }
    let mut best: Option<(usize, u16)> = None;
    for i in (0..p.n * p.n).filter(|&i| p.white[i] && grid[i] == 0) {
        let cand = run_candidates(&p.across[of[i].0], grid) & run_candidates(&p.down[of[i].1], grid);
        if cand == 0 {
            return;
        }
        if best.is_none_or(|(_, b)| cand.count_ones() < b.count_ones()) {
            best = Some((i, cand));
        }
    }
    let Some((i, cand)) = best else {
        // Tout est posé : les sommes sont justes si chaque tranche n'a plus de candidat manquant.
        if p.across.iter().chain(&p.down).all(|r| r.cells.iter().map(|&j| u32::from(grid[j])).sum::<u32>() == r.sum) {
            out.push(grid.clone());
        }
        return;
    };
    for v in 1..=9u8 {
        if cand & (1 << v) != 0 {
            grid[i] = v;
            solve(grid, p, of, limit, out);
            grid[i] = 0;
        }
    }
}

fn generate(seed: u64, difficulty: Difficulty) -> Puzzle {
    let n = size(difficulty);
    let mut rng = StdRng::seed_from_u64(seed);
    let white = pattern(n, &mut rng);
    let (mut across, mut down) = runs(&white, n);
    let of = membership(n, &across, &down);
    // Ordre de remplissage : ligne par ligne (les tranches se contraignent vite).
    let order: Vec<usize> = (0..n * n).filter(|&i| white[i]).collect();
    // Plusieurs remplissages de la même forme : on garde celui qui demande le moins de chiffres donnés.
    let mut best: Option<Puzzle> = None;
    for _ in 0..6 {
        let mut solution = vec![0u8; n * n];
        // Un remplissage existe toujours (tranches de 8 cases au plus).
        fill(&mut solution, &order, &of, &across, &down, &mut rng);
        for run in across.iter_mut().chain(down.iter_mut()) {
            run.sum = run.cells.iter().map(|&i| u32::from(solution[i])).sum();
        }
        let mut p = Puzzle {
            n,
            white: white.clone(),
            given: vec![0; n * n],
            solution,
            across: across.clone(),
            down: down.clone(),
        };
        // Tant que deux solutions coexistent, on donne le chiffre d'une case où elles diffèrent.
        loop {
            let mut found = Vec::new();
            solve(&mut p.given.clone(), &p, &of, 2, &mut found);
            if found.len() < 2 {
                break;
            }
            let differ: Vec<usize> = order.iter().copied().filter(|&i| found[0][i] != found[1][i]).collect();
            let i = differ[rng.random_range(0..differ.len())];
            p.given[i] = p.solution[i];
        }
        let givens = |p: &Puzzle| p.given.iter().filter(|g| **g != 0).count();
        if best.as_ref().is_none_or(|b| givens(&p) < givens(b)) {
            best = Some(p);
        }
    }
    best.expect("au moins un essai")
}

fn render(p: &Puzzle, solution: bool) -> image::GrayImage {
    let n = p.n as i64;
    let cell = (500 / n).min(62);
    let side = cell * n;
    let (ox, oy) = ((512 - side) / 2, 3);
    let mut img = draw::canvas((side + 16) as u32);
    let of = membership(p.n, &p.across, &p.down);
    // Somme de la tranche qui commence juste après la case noire (r, c), à droite ou en dessous.
    let clue = |i: usize, right: bool| -> Option<u32> {
        let next = if right { (i % p.n + 1 < p.n).then_some(i + 1) } else { (i + p.n < p.n * p.n).then_some(i + p.n) }?;
        if !p.white[next] {
            return None;
        }
        Some(if right { p.across[of[next].0].sum } else { p.down[of[next].1].sum })
    };
    for i in 0..p.n * p.n {
        let (r, c) = ((i / p.n) as i64, (i % p.n) as i64);
        let (x, y) = (ox + c * cell, oy + r * cell);
        if p.white[i] {
            let v = if solution { p.solution[i] } else { p.given[i] };
            if v != 0 {
                let scale = 4;
                draw::digit(
                    &mut img,
                    v,
                    x + (cell - draw::DIGIT_W * scale) / 2,
                    y + (cell - draw::DIGIT_H * scale) / 2,
                    scale,
                );
            }
            continue;
        }
        let (right, below) = (clue(i, true), clue(i, false));
        if right.is_none() && below.is_none() {
            // Case noire sans indice : hachurée.
            for dy in 0..cell {
                for dx in 0..cell {
                    if (dx + dy) % 4 == 0 {
                        draw::fill_rect(&mut img, x + dx, y + dy, 1, 1);
                    }
                }
            }
            continue;
        }
        // Diagonale, somme horizontale en haut à droite, verticale en bas à gauche.
        for t in 0..cell {
            draw::fill_rect(&mut img, x + t, y + t, 2, 2);
        }
        let scale = 2;
        if let Some(sum) = right {
            let w = draw::number_width(sum, scale);
            draw::number(&mut img, sum, x + cell - w - 4, y + 5, scale);
        }
        if let Some(sum) = below {
            draw::number(&mut img, sum, x + 5, y + cell - draw::DIGIT_H * scale - 5, scale);
        }
    }
    for k in 0..=n {
        let thick = if k == 0 || k == n { 4 } else { 2 };
        draw::fill_rect(&mut img, ox + k * cell - thick / 2, oy - 2, thick, side + 4);
        draw::fill_rect(&mut img, ox - 2, oy + k * cell - thick / 2, side + 4, thick);
    }
    img
}

pub fn build(difficulty: Difficulty, seed: Option<u64>, solution: bool) -> Doc {
    let seed = seed.unwrap_or_else(|| rand::rng().random_range(1..100_000));
    let puzzle = generate(seed, difficulty);
    let mut doc = Doc::new();
    let title = if solution { crate::i18n::t("Solution du kakuro", "Kakuro solution") } else { "Kakuro" };
    doc.header(&format!("{title} · {}", difficulty.label()));
    if !solution {
        doc.text(
            crate::i18n::t(
                "Chaque tranche fait la somme indiquée (à droite : horizontale, en bas : verticale), avec des chiffres de 1 à 9 tous différents.",
                "Each run adds up to its clue (right: across, below: down), using digits 1 to 9, all different.",
            ),
            Style::default().small(),
        );
    }
    doc.feed(1);
    doc.image(render(&puzzle, solution));
    doc.text(&tr!("n° {seed}", "No. {seed}"), Style::default().small().align(Align::Right));
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puzzles_are_consistent_and_unique() {
        for (seed, difficulty) in [(1, Difficulty::Easy), (42, Difficulty::Medium), (7, Difficulty::Hard)] {
            let p = generate(seed, difficulty);
            let n = p.n;
            for run in p.across.iter().chain(&p.down) {
                assert!(run.cells.len() >= 2 && run.cells.len() <= 9);
                let digits: Vec<u8> = run.cells.iter().map(|&i| p.solution[i]).collect();
                assert_eq!(digits.iter().map(|&d| u32::from(d)).sum::<u32>(), run.sum);
                let mut sorted = digits.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), digits.len(), "chiffres répétés");
            }
            let of = membership(n, &p.across, &p.down);
            let mut found = Vec::new();
            solve(&mut p.given.clone(), &p, &of, 2, &mut found);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0], p.solution);
        }
    }

    #[test]
    fn same_seed_same_grid() {
        assert_eq!(generate(5, Difficulty::Medium), generate(5, Difficulty::Medium));
    }
}
