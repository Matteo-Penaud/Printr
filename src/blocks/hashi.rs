//! Hashi (ponts) : relier toutes les îles par des ponts horizontaux ou verticaux, un ou deux entre
//! deux îles, sans qu'ils se croisent ; le chiffre d'une île dit combien de ponts en partent. Généré
//! sur place, à solution unique ; le numéro est la graine.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};

use super::sudoku::Difficulty;
use crate::doc::{Align, Doc, Style};
use crate::draw;

fn shape(difficulty: Difficulty) -> (usize, usize) {
    // (côté, nombre d'îles visé)
    match difficulty {
        Difficulty::Easy => (7, 9),
        Difficulty::Medium => (9, 15),
        Difficulty::Hard => (11, 22),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Puzzle {
    n: usize,
    /// (rangée, colonne, chiffre)
    islands: Vec<(usize, usize, u8)>,
    /// (île a, île b, ponts) de la solution, a < b.
    bridges: Vec<(usize, usize, u8)>,
}

/// Les liaisons possibles : deux îles qui se voient en ligne droite, sans île entre elles.
fn edges(n: usize, islands: &[(usize, usize, u8)]) -> Vec<(usize, usize)> {
    let mut at = vec![None; n * n];
    for (k, &(r, c, _)) in islands.iter().enumerate() {
        at[r * n + c] = Some(k);
    }
    let mut out = Vec::new();
    for (k, &(r, c, _)) in islands.iter().enumerate() {
        if let Some(j) = (c + 1..n).find_map(|cc| at[r * n + cc]) {
            out.push((k.min(j), k.max(j)));
        }
        if let Some(j) = (r + 1..n).find_map(|rr| at[rr * n + c]) {
            out.push((k.min(j), k.max(j)));
        }
    }
    out
}

/// Deux liaisons se croisent-elles (une horizontale, une verticale, en leur milieu) ?
fn cross(islands: &[(usize, usize, u8)], a: (usize, usize), b: (usize, usize)) -> bool {
    let seg = |e: (usize, usize)| (islands[e.0], islands[e.1]);
    let (((r1, c1, _), (r2, c2, _)), ((r3, c3, _), (r4, c4, _))) = (seg(a), seg(b));
    let (h, v) = if r1 == r2 && c3 == c4 {
        ((r1, c1.min(c2), c1.max(c2)), (c3, r3.min(r4), r3.max(r4)))
    } else if c1 == c2 && r3 == r4 {
        ((r3, c3.min(c4), c3.max(c4)), (c1, r1.min(r2), r1.max(r2)))
    } else {
        return false;
    };
    h.1 < v.0 && v.0 < h.2 && v.1 < h.0 && h.0 < v.2
}

struct Solver<'a> {
    islands: &'a [(usize, usize, u8)],
    edges: Vec<(usize, usize)>,
    crossing: Vec<Vec<usize>>,
    of_island: Vec<Vec<usize>>,
}

impl<'a> Solver<'a> {
    fn new(n: usize, islands: &'a [(usize, usize, u8)]) -> Self {
        let edges = edges(n, islands);
        let crossing = (0..edges.len())
            .map(|i| (0..edges.len()).filter(|&j| j != i && cross(islands, edges[i], edges[j])).collect())
            .collect();
        let mut of_island = vec![Vec::new(); islands.len()];
        for (k, &(a, b)) in edges.iter().enumerate() {
            of_island[a].push(k);
            of_island[b].push(k);
        }
        Solver { islands, edges, crossing, of_island }
    }

    /// Une île peut-elle encore atteindre son chiffre ? (`None` : liaison pas encore décidée)
    fn island_ok(&self, k: usize, values: &[Option<u8>]) -> bool {
        let target = u32::from(self.islands[k].2);
        let (mut now, mut room) = (0u32, 0u32);
        for &e in &self.of_island[k] {
            match values[e] {
                Some(v) => now += u32::from(v),
                None => {
                    let blocked = self.crossing[e].iter().any(|&x| values[x].is_some_and(|v| v > 0));
                    if !blocked {
                        room += 2;
                    }
                }
            }
        }
        now <= target && now + room >= target
    }

    fn connected(&self, values: &[Option<u8>]) -> bool {
        let mut seen = vec![false; self.islands.len()];
        let mut stack = vec![0];
        seen[0] = true;
        while let Some(k) = stack.pop() {
            for &e in &self.of_island[k] {
                if values[e].unwrap_or(0) > 0 {
                    let (a, b) = self.edges[e];
                    let other = if a == k { b } else { a };
                    if !seen[other] {
                        seen[other] = true;
                        stack.push(other);
                    }
                }
            }
        }
        seen.iter().all(|s| *s)
    }

    /// Nombre de solutions, plafonné à `limit`.
    fn count(&self, values: &mut Vec<Option<u8>>, limit: usize) -> usize {
        // La liaison à décider : celle de l'île qui a le moins de liaisons libres.
        let next = (0..self.islands.len())
            .filter_map(|k| {
                let open: Vec<usize> = self.of_island[k].iter().copied().filter(|&e| values[e].is_none()).collect();
                (!open.is_empty()).then(|| (open.len(), open[0]))
            })
            .min()
            .map(|(_, e)| e);
        let Some(e) = next else {
            return usize::from(self.connected(values));
        };
        let mut total = 0;
        for v in 0..=2u8 {
            if v > 0 && self.crossing[e].iter().any(|&x| values[x].is_some_and(|w| w > 0)) {
                break;
            }
            values[e] = Some(v);
            let (a, b) = self.edges[e];
            if self.island_ok(a, values) && self.island_ok(b, values) {
                total += self.count(values, limit - total);
            }
            values[e] = None;
            if total >= limit {
                break;
            }
        }
        total
    }
}

/// Fait pousser un archipel : chaque nouvelle île part d'une île existante, reliée par un ou deux
/// ponts qui ne croisent rien ; quelques ponts de plus entre îles qui se voient.
fn grow(n: usize, target: usize, rng: &mut StdRng) -> Puzzle {
    // 0 : vide ; 1 : île ; 2 : pont.
    let mut cell = vec![0u8; n * n];
    let mut islands: Vec<(usize, usize, u8)> = Vec::new();
    let mut bridges: Vec<(usize, usize, u8)> = Vec::new();
    let start = (rng.random_range(0..n), rng.random_range(0..n));
    cell[start.0 * n + start.1] = 1;
    islands.push((start.0, start.1, 0));
    let dirs: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    let mut tries = 0;
    while islands.len() < target && tries < 2000 {
        tries += 1;
        let from = rng.random_range(0..islands.len());
        let (r, c, _) = islands[from];
        let (dr, dc) = dirs[rng.random_range(0..4)];
        let len = rng.random_range(2..=n.min(6) as i64) as isize;
        let (tr, tc) = (r as isize + dr * len, c as isize + dc * len);
        if !(0..n as isize).contains(&tr) || !(0..n as isize).contains(&tc) {
            continue;
        }
        let path: Vec<usize> =
            (1..len).map(|k| (r as isize + dr * k) as usize * n + (c as isize + dc * k) as usize).collect();
        let (tr, tc) = (tr as usize, tc as usize);
        let neighbours = dirs.iter().any(|&(a, b)| {
            let (rr, cc) = (tr as isize + a, tc as isize + b);
            (0..n as isize).contains(&rr) && (0..n as isize).contains(&cc) && cell[rr as usize * n + cc as usize] == 1
        });
        if cell[tr * n + tc] != 0 || path.iter().any(|&i| cell[i] != 0) || neighbours {
            continue;
        }
        for &i in &path {
            cell[i] = 2;
        }
        cell[tr * n + tc] = 1;
        islands.push((tr, tc, 0));
        let new = islands.len() - 1;
        bridges.push((from.min(new), from.max(new), rng.random_range(1..=2)));
    }
    // Des ponts en plus, entre îles qui se voient déjà sans rien croiser : des boucles, moins évident.
    let mut extra = edges(n, &islands);
    extra.shuffle(rng);
    for (a, b) in extra.into_iter().take(islands.len() / 3) {
        if bridges.iter().any(|&(x, y, _)| (x, y) == (a, b)) {
            continue;
        }
        let ((r1, c1, _), (r2, c2, _)) = (islands[a], islands[b]);
        let path: Vec<usize> = if r1 == r2 {
            (c1.min(c2) + 1..c1.max(c2)).map(|c| r1 * n + c).collect()
        } else {
            (r1.min(r2) + 1..r1.max(r2)).map(|r| r * n + c1).collect()
        };
        if path.iter().all(|&i| cell[i] == 0) {
            for &i in &path {
                cell[i] = 2;
            }
            bridges.push((a, b, rng.random_range(1..=2)));
        }
    }
    for &(a, b, v) in &bridges {
        islands[a].2 += v;
        islands[b].2 += v;
    }
    Puzzle { n, islands, bridges }
}

fn generate(seed: u64, difficulty: Difficulty) -> Puzzle {
    let (n, target) = shape(difficulty);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut last = None;
    for _ in 0..200 {
        let p = grow(n, target, &mut rng);
        if p.islands.len() < target * 2 / 3 {
            continue;
        }
        let solver = Solver::new(n, &p.islands);
        if solver.count(&mut vec![None; solver.edges.len()], 2) == 1 {
            return p;
        }
        last = Some(p);
    }
    last.expect("un archipel")
}

/// Disque (contour) de rayon `radius` centré en (cx, cy).
fn ring(img: &mut image::GrayImage, cx: i64, cy: i64, radius: i64, thick: i64) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
                continue;
            }
            if d <= radius as f64 {
                let ink = if d > (radius - thick) as f64 { draw::BLACK } else { draw::WHITE };
                img.put_pixel(x as u32, y as u32, ink);
            }
        }
    }
}

fn render(p: &Puzzle, solution: bool) -> image::GrayImage {
    let n = p.n as i64;
    let cell = 496 / n;
    let side = cell * n;
    let (ox, oy) = ((512 - side) / 2, 4);
    let mut img = draw::canvas((side + 10) as u32);
    let center = |(r, c, _): (usize, usize, u8)| (ox + c as i64 * cell + cell / 2, oy + r as i64 * cell + cell / 2);
    if solution {
        for &(a, b, v) in &p.bridges {
            let ((x1, y1), (x2, y2)) = (center(p.islands[a]), center(p.islands[b]));
            let offsets: &[i64] = if v == 2 { &[-5, 4] } else { &[0] };
            for &o in offsets {
                if y1 == y2 {
                    draw::fill_rect(&mut img, x1.min(x2), y1 + o - 1, (x2 - x1).abs(), 3);
                } else {
                    draw::fill_rect(&mut img, x1 + o - 1, y1.min(y2), 3, (y2 - y1).abs());
                }
            }
        }
    } else {
        // Points discrets aux intersections, pour tracer droit.
        for r in 0..n {
            for c in 0..n {
                draw::fill_rect(&mut img, ox + c * cell + cell / 2, oy + r * cell + cell / 2, 1, 1);
            }
        }
    }
    let radius = cell * 2 / 5;
    let scale = if cell >= 50 { 3 } else { 2 };
    for &island in &p.islands {
        let (x, y) = center(island);
        ring(&mut img, x, y, radius, 3);
        draw::digit(&mut img, island.2, x - draw::DIGIT_W * scale / 2, y - draw::DIGIT_H * scale / 2, scale);
    }
    img
}

pub fn build(difficulty: Difficulty, seed: Option<u64>, solution: bool) -> Doc {
    let seed = seed.unwrap_or_else(|| rand::rng().random_range(1..100_000));
    let puzzle = generate(seed, difficulty);
    let mut doc = Doc::new();
    let title = if solution { crate::i18n::t("Solution du hashi", "Hashi solution") } else { "Hashi" };
    doc.header(&format!("{title} · {}", difficulty.label()));
    if !solution {
        doc.text(
            crate::i18n::t(
                "Relie toutes les îles par des ponts droits, un ou deux entre deux îles, sans croisement ; le chiffre dit combien de ponts partent de l'île.",
                "Connect all the islands with straight bridges, one or two between two islands, never crossing; the number says how many bridges leave the island.",
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
    fn archipelagos_are_connected_and_unique() {
        for (seed, difficulty) in [(1, Difficulty::Easy), (42, Difficulty::Medium), (7, Difficulty::Hard)] {
            let p = generate(seed, difficulty);
            assert!(p.islands.iter().all(|i| (1..=8).contains(&i.2)));
            let solver = Solver::new(p.n, &p.islands);
            assert_eq!(solver.count(&mut vec![None; solver.edges.len()], 2), 1, "{difficulty:?}");
            // La solution fait bien les chiffres de chaque île.
            for (k, island) in p.islands.iter().enumerate() {
                let total: u8 = p.bridges.iter().filter(|b| b.0 == k || b.1 == k).map(|b| b.2).sum();
                assert_eq!(total, island.2);
            }
        }
    }

    #[test]
    fn crossings_are_detected() {
        let islands = [(1, 0, 1), (1, 4, 1), (0, 2, 1), (3, 2, 1)];
        assert!(cross(&islands, (0, 1), (2, 3)));
        assert!(!cross(&islands, (0, 1), (0, 1)));
    }

    #[test]
    fn same_seed_same_grid() {
        assert_eq!(generate(3, Difficulty::Medium), generate(3, Difficulty::Medium));
    }
}
