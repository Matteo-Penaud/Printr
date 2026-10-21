//! Problème d'échecs du jour : le puzzle quotidien de Lichess (API publique, gratuite et sans clé),
//! l'échiquier dessiné du côté qui joue, et la solution à l'envers, en notation algébrique.
//!
//! Lichess donne la position (FEN) et la solution en coordonnées (« d6e5 ») ; un petit moteur de
//! règles suffit pour l'écrire comme dans les livres (« Fe5 », « Txf8+ », « Dh7# ») : coups légaux,
//! échec et mat.

use anyhow::{Context, Result, bail};
use image::GrayImage;
use serde::{Deserialize, Serialize};

use super::Ctx;
use crate::doc::{Doc, Style};
use crate::{draw, i18n};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Color {
    White,
    Black,
}

impl Color {
    fn other(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

impl Kind {
    /// La lettre de la pièce en notation algébrique (« F » pour fou en français, « B » en anglais).
    fn letter(self) -> &'static str {
        match self {
            Kind::King => i18n::t("R", "K"),
            Kind::Queen => i18n::t("D", "Q"),
            Kind::Rook => i18n::t("T", "R"),
            Kind::Bishop => i18n::t("F", "B"),
            Kind::Knight => i18n::t("C", "N"),
            Kind::Pawn => "",
        }
    }

    /// Rang dans la planche des pièces (roi, dame, tour, fou, cavalier, pion).
    fn sprite(self) -> u32 {
        self as u32
    }
}

type Piece = (Color, Kind);
/// Case : (colonne, rangée), de 0 à 7 (a1 = (0, 0)).
type Square = (i8, i8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Move {
    from: Square,
    to: Square,
    promotion: Option<Kind>,
}

#[derive(Clone, Debug)]
struct Position {
    board: [[Option<Piece>; 8]; 8],
    turn: Color,
    /// Roques encore permis : blanc côté roi, blanc côté dame, noir côté roi, noir côté dame.
    castling: [bool; 4],
    en_passant: Option<Square>,
}

impl Position {
    fn parse(fen: &str) -> Result<Position> {
        let bad = || anyhow::anyhow!("{}", i18n::t("position d'échecs invalide", "invalid chess position"));
        let mut fields = fen.split_whitespace();
        let rows = fields.next().ok_or_else(bad)?;
        let mut board = [[None; 8]; 8];
        let ranks: Vec<&str> = rows.split('/').collect();
        if ranks.len() != 8 {
            return Err(bad());
        }
        for (i, rank) in ranks.iter().enumerate() {
            let y = 7 - i;
            let mut x = 0usize;
            for c in rank.chars() {
                if let Some(n) = c.to_digit(10) {
                    x += n as usize;
                    continue;
                }
                let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                let kind = match c.to_ascii_lowercase() {
                    'k' => Kind::King,
                    'q' => Kind::Queen,
                    'r' => Kind::Rook,
                    'b' => Kind::Bishop,
                    'n' => Kind::Knight,
                    'p' => Kind::Pawn,
                    _ => return Err(bad()),
                };
                *board.get_mut(y).and_then(|r| r.get_mut(x)).ok_or_else(bad)? = Some((color, kind));
                x += 1;
            }
            if x != 8 {
                return Err(bad());
            }
        }
        let turn = match fields.next() {
            Some("w") => Color::White,
            Some("b") => Color::Black,
            _ => return Err(bad()),
        };
        let rights = fields.next().unwrap_or("-");
        let castling = ['K', 'Q', 'k', 'q'].map(|c| rights.contains(c));
        let en_passant = fields.next().and_then(square);
        Ok(Position { board, turn, castling, en_passant })
    }

    fn at(&self, (x, y): Square) -> Option<Piece> {
        if (0..8).contains(&x) && (0..8).contains(&y) { self.board[y as usize][x as usize] } else { None }
    }

    fn king(&self, color: Color) -> Option<Square> {
        (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).find(|&s| self.at(s) == Some((color, Kind::King)))
    }

    /// La case `s` est-elle attaquée par les pièces de `by` ?
    fn attacked(&self, (x, y): Square, by: Color) -> bool {
        let is = |s: Square, kinds: &[Kind]| matches!(self.at(s), Some((c, k)) if c == by && kinds.contains(&k));
        let forward = if by == Color::White { -1 } else { 1 };
        if is((x - 1, y + forward), &[Kind::Pawn]) || is((x + 1, y + forward), &[Kind::Pawn]) {
            return true;
        }
        if KNIGHT.iter().any(|&(dx, dy)| is((x + dx, y + dy), &[Kind::Knight])) {
            return true;
        }
        if KING.iter().any(|&(dx, dy)| is((x + dx, y + dy), &[Kind::King])) {
            return true;
        }
        let ray = |dirs: &[(i8, i8)], kinds: &[Kind]| {
            dirs.iter().any(|&(dx, dy)| {
                let (mut cx, mut cy) = (x + dx, y + dy);
                while inside((cx, cy)) {
                    if self.at((cx, cy)).is_some() {
                        return is((cx, cy), kinds);
                    }
                    cx += dx;
                    cy += dy;
                }
                false
            })
        };
        ray(&ROOK, &[Kind::Rook, Kind::Queen]) || ray(&BISHOP, &[Kind::Bishop, Kind::Queen])
    }

    fn in_check(&self, color: Color) -> bool {
        self.king(color).is_some_and(|k| self.attacked(k, color.other()))
    }

    /// Les coups possibles sans tenir compte des échecs au roi.
    fn pseudo_moves(&self) -> Vec<Move> {
        let me = self.turn;
        let mut moves = Vec::new();
        let mut push = |from, to, pawn_to_last: bool| {
            if pawn_to_last {
                for kind in [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight] {
                    moves.push(Move { from, to, promotion: Some(kind) });
                }
            } else {
                moves.push(Move { from, to, promotion: None });
            }
        };
        for y in 0..8 {
            for x in 0..8 {
                let Some((color, kind)) = self.at((x, y)) else { continue };
                if color != me {
                    continue;
                }
                let free_or_enemy = |s: Square| inside(s) && self.at(s).is_none_or(|(c, _)| c != me);
                match kind {
                    Kind::Pawn => {
                        let dir = if me == Color::White { 1 } else { -1 };
                        let start = if me == Color::White { 1 } else { 6 };
                        let last = |ty: i8| ty == 7 || ty == 0;
                        if inside((x, y + dir)) && self.at((x, y + dir)).is_none() {
                            push((x, y), (x, y + dir), last(y + dir));
                            if y == start && self.at((x, y + 2 * dir)).is_none() {
                                push((x, y), (x, y + 2 * dir), false);
                            }
                        }
                        for dx in [-1, 1] {
                            let to = (x + dx, y + dir);
                            let enemy = matches!(self.at(to), Some((c, _)) if c != me);
                            if inside(to) && (enemy || self.en_passant == Some(to)) {
                                push((x, y), to, last(to.1));
                            }
                        }
                    }
                    Kind::Knight | Kind::King => {
                        let steps: &[(i8, i8)] = if kind == Kind::Knight { &KNIGHT } else { &KING };
                        for &(dx, dy) in steps {
                            if free_or_enemy((x + dx, y + dy)) {
                                push((x, y), (x + dx, y + dy), false);
                            }
                        }
                        if kind == Kind::King {
                            self.castles((x, y), &mut |to| push((x, y), to, false));
                        }
                    }
                    _ => {
                        let dirs: &[(i8, i8)] = match kind {
                            Kind::Rook => &ROOK,
                            Kind::Bishop => &BISHOP,
                            _ => &KING,
                        };
                        for &(dx, dy) in dirs {
                            let (mut cx, mut cy) = (x + dx, y + dy);
                            while free_or_enemy((cx, cy)) {
                                push((x, y), (cx, cy), false);
                                if self.at((cx, cy)).is_some() {
                                    break;
                                }
                                cx += dx;
                                cy += dy;
                            }
                        }
                    }
                }
            }
        }
        moves
    }

    /// Les roques possibles : droits intacts, cases libres et non attaquées.
    fn castles(&self, (x, y): Square, add: &mut dyn FnMut(Square)) {
        let me = self.turn;
        let (home, rights) = if me == Color::White { (0, (0, 1)) } else { (7, (2, 3)) };
        if (x, y) != (4, home) || self.in_check(me) {
            return;
        }
        let empty = |xs: &[i8]| xs.iter().all(|&cx| self.at((cx, home)).is_none());
        let safe = |xs: &[i8]| xs.iter().all(|&cx| !self.attacked((cx, home), me.other()));
        if self.castling[rights.0] && empty(&[5, 6]) && safe(&[5, 6]) {
            add((6, home));
        }
        if self.castling[rights.1] && empty(&[1, 2, 3]) && safe(&[2, 3]) {
            add((2, home));
        }
    }

    fn play(&self, m: Move) -> Position {
        let mut next = self.clone();
        let piece = self.at(m.from);
        next.board[m.from.1 as usize][m.from.0 as usize] = None;
        if let Some((color, kind)) = piece {
            // Prise en passant : le pion pris n'est pas sur la case d'arrivée.
            if kind == Kind::Pawn && Some(m.to) == self.en_passant && m.from.0 != m.to.0 {
                next.board[m.from.1 as usize][m.to.0 as usize] = None;
            }
            // Roque : la tour suit le roi.
            if kind == Kind::King && (m.to.0 - m.from.0).abs() == 2 {
                let (rook_from, rook_to) = if m.to.0 == 6 { (7, 5) } else { (0, 3) };
                let row = m.from.1 as usize;
                next.board[row][rook_to] = next.board[row][rook_from].take();
            }
            next.board[m.to.1 as usize][m.to.0 as usize] = Some((color, m.promotion.unwrap_or(kind)));
            next.en_passant =
                (kind == Kind::Pawn && (m.to.1 - m.from.1).abs() == 2).then_some((m.from.0, (m.from.1 + m.to.1) / 2));
        }
        for (i, corner) in [(7, 0), (0, 0), (7, 7), (0, 7)].iter().enumerate() {
            if m.from == *corner || m.to == *corner {
                next.castling[i] = false;
            }
        }
        if m.from == (4, 0) {
            next.castling[0] = false;
            next.castling[1] = false;
        }
        if m.from == (4, 7) {
            next.castling[2] = false;
            next.castling[3] = false;
        }
        next.turn = self.turn.other();
        next
    }

    fn legal_moves(&self) -> Vec<Move> {
        self.pseudo_moves().into_iter().filter(|&m| !self.play(m).in_check(self.turn)).collect()
    }

    /// Le coup en notation algébrique, dans la langue du ticket.
    fn san(&self, m: Move) -> String {
        let Some((_, kind)) = self.at(m.from) else { return String::new() };
        let mut out = String::new();
        if kind == Kind::King && (m.to.0 - m.from.0).abs() == 2 {
            out.push_str(if m.to.0 == 6 { "O-O" } else { "O-O-O" });
        } else {
            let capture = self.at(m.to).is_some() || (kind == Kind::Pawn && m.from.0 != m.to.0);
            out.push_str(kind.letter());
            if kind == Kind::Pawn {
                if capture {
                    out.push(file(m.from.0));
                }
            } else {
                // Une autre pièce du même genre pourrait aller sur la même case : on précise laquelle.
                let rivals: Vec<Square> = self
                    .legal_moves()
                    .into_iter()
                    .filter(|o| o.to == m.to && o.from != m.from && self.at(o.from) == self.at(m.from))
                    .map(|o| o.from)
                    .collect();
                if !rivals.is_empty() {
                    if rivals.iter().all(|r| r.0 != m.from.0) {
                        out.push(file(m.from.0));
                    } else if rivals.iter().all(|r| r.1 != m.from.1) {
                        out.push(rank(m.from.1));
                    } else {
                        out.push(file(m.from.0));
                        out.push(rank(m.from.1));
                    }
                }
            }
            if capture {
                out.push('x');
            }
            out.push(file(m.to.0));
            out.push(rank(m.to.1));
            if let Some(p) = m.promotion {
                out.push('=');
                out.push_str(p.letter());
            }
        }
        let next = self.play(m);
        if next.in_check(next.turn) {
            out.push(if next.legal_moves().is_empty() { '#' } else { '+' });
        }
        out
    }
}

const KNIGHT: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK: [(i8, i8); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
const BISHOP: [(i8, i8); 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];

fn inside((x, y): Square) -> bool {
    (0..8).contains(&x) && (0..8).contains(&y)
}

fn file(x: i8) -> char {
    (b'a' + x as u8) as char
}

fn rank(y: i8) -> char {
    (b'1' + y as u8) as char
}

/// « e4 » → (4, 3).
fn square(s: &str) -> Option<Square> {
    let b = s.as_bytes();
    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return None;
    }
    Some(((b[0] - b'a') as i8, (b[1] - b'1') as i8))
}

/// « d6e5 », « e7e8q » → coup.
fn uci(s: &str) -> Option<Move> {
    let promotion = match s.get(4..5) {
        None => None,
        Some("q") => Some(Kind::Queen),
        Some("r") => Some(Kind::Rook),
        Some("b") => Some(Kind::Bishop),
        Some("n") => Some(Kind::Knight),
        Some(_) => return None,
    };
    Some(Move { from: square(s.get(0..2)?)?, to: square(s.get(2..4)?)?, promotion })
}

/// La solution numérotée : « 1… Fe5 2. Txf8+ Fxf8 » quand les noirs commencent.
fn solution_text(position: &Position, moves: &[String]) -> Result<String> {
    let mut pos = position.clone();
    let mut out = Vec::new();
    let mut number = 1;
    for (i, text) in moves.iter().enumerate() {
        let m = uci(text)
            .filter(|m| pos.legal_moves().contains(m))
            .with_context(|| tr!("coup invalide dans la solution : {text}", "invalid move in the solution: {text}"))?;
        match (pos.turn, i) {
            (Color::White, _) => out.push(format!("{number}.")),
            (Color::Black, 0) => out.push(format!("{number}…")),
            _ => {}
        }
        out.push(pos.san(m));
        if pos.turn == Color::Black {
            number += 1;
        }
        pos = pos.play(m);
    }
    Ok(out.join(" "))
}

/// Le puzzle du jour, tel que Lichess le donne (le reste est ignoré).
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Daily {
    puzzle: Puzzle,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Puzzle {
    id: String,
    rating: u32,
    fen: String,
    solution: Vec<String>,
    #[serde(rename = "lastMove", default)]
    last_move: Option<String>,
}

const SQUARE: u32 = 56;
const PIECES: &[u8] = include_bytes!("chess-pieces.png");

/// L'échiquier, vu du côté qui joue : cases foncées hachurées, dernier coup encadré, coordonnées.
fn board_image(pos: &Position, last: Option<Move>) -> Result<GrayImage> {
    let sprite = image::load_from_memory(PIECES)?.to_luma8();
    let side = SQUARE * 8;
    let (left, top) = ((crate::raster::PRINT_WIDTH - side) / 2, 4);
    let mut img = draw::canvas(top + side + 4 + 18);
    let flipped = pos.turn == Color::Black;
    // (colonne, rangée) à l'écran → case de l'échiquier.
    let at = |col: u32, row: u32| -> Square {
        if flipped { (7 - col as i8, row as i8) } else { (col as i8, 7 - row as i8) }
    };
    let framed = |s: Square| last.is_some_and(|m| m.from == s || m.to == s);
    for row in 0..8 {
        for col in 0..8 {
            let sq = at(col, row);
            let (x0, y0) = (left + col * SQUARE, top + row * SQUARE);
            let dark = (sq.0 + sq.1) % 2 == 0;
            for dy in 0..SQUARE {
                for dx in 0..SQUARE {
                    let hatch = dark && (dx + dy) % 5 == 0;
                    let frame = framed(sq) && (dx < 4 || dy < 4 || dx >= SQUARE - 4 || dy >= SQUARE - 4);
                    if hatch || frame {
                        img.put_pixel(x0 + dx, y0 + dy, draw::BLACK);
                    }
                }
            }
            if let Some((color, kind)) = pos.at(sq) {
                let index = kind.sprite() + if color == Color::Black { 6 } else { 0 };
                for dy in 0..SQUARE {
                    for dx in 0..SQUARE {
                        match sprite.get_pixel(index * SQUARE + dx, dy).0[0] {
                            0 => img.put_pixel(x0 + dx, y0 + dy, draw::BLACK),
                            255 => img.put_pixel(x0 + dx, y0 + dy, draw::WHITE),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    // Cadre et coordonnées.
    let (l, t, s) = (i64::from(left), i64::from(top), i64::from(side));
    draw::fill_rect(&mut img, l - 3, t - 3, s + 6, 3);
    draw::fill_rect(&mut img, l - 3, t + s, s + 6, 3);
    draw::fill_rect(&mut img, l - 3, t - 3, 3, s + 6);
    draw::fill_rect(&mut img, l + s, t - 3, 3, s + 6);
    for i in 0..8u32 {
        let (x, _) = at(i, 0);
        let (_, y) = at(0, i);
        let middle = i64::from(i * SQUARE + SQUARE / 2);
        draw::text(&mut img, &file(x).to_string(), l + middle - 5, t + s + 6, 2);
        draw::text(&mut img, &rank(y).to_string(), l - 18, t + middle - 7, 2);
    }
    Ok(img)
}

fn fetch(ctx: &Ctx) -> Result<Puzzle> {
    let key = format!("echecs-{}", ctx.today);
    let cache = ctx.cache.as_ref();
    if let Some(daily) = cache.filter(|_| !ctx.refresh).and_then(|c| c.get::<Daily>(&key)) {
        return Ok(daily.puzzle);
    }
    let daily: Daily = ctx
        .http
        .get("https://lichess.org/api/puzzle/daily")
        .call()
        .context(i18n::t("Lichess injoignable", "Lichess unreachable"))?
        .body_mut()
        .read_json()
        .context(i18n::t("réponse de Lichess illisible", "unreadable answer from Lichess"))?;
    if let Some(cache) = cache {
        cache.put(&key, &daily);
    }
    Ok(daily.puzzle)
}

pub fn build(ctx: &Ctx, solution: bool) -> Result<Doc> {
    let puzzle = fetch(ctx)?;
    let pos = Position::parse(&puzzle.fen)?;
    if puzzle.solution.is_empty() {
        bail!("{}", i18n::t("problème sans solution", "puzzle without a solution"));
    }
    let answer = solution_text(&pos, &puzzle.solution)?;
    let mut doc = Doc::new();
    doc.header(i18n::t("Échecs · problème du jour", "Chess · daily puzzle"));
    doc.feed(1);
    let to_play = match pos.turn {
        Color::White => i18n::t("Les blancs jouent et gagnent", "White to play and win"),
        Color::Black => i18n::t("Les noirs jouent et gagnent", "Black to play and win"),
    };
    doc.text(to_play, Style::default().bold().center());
    let moves = puzzle.solution.len().div_ceil(2);
    doc.text(
        &tr!(
            "en {moves} coup{} · niveau {}",
            "in {moves} move{} · rating {}",
            if moves > 1 { "s" } else { "" },
            puzzle.rating
        ),
        Style::default().small().center(),
    );
    doc.feed(1);
    doc.image(board_image(&pos, puzzle.last_move.as_deref().and_then(uci))?);
    doc.text(
        i18n::t("Encadré : le dernier coup de l'adversaire", "Framed: the opponent's last move"),
        Style::default().small().center(),
    );
    if solution {
        doc.feed(1);
        doc.text(&tr!("Solution : {answer}", "Solution: {answer}"), Style::default().upside_down());
    }
    doc.text(&format!("lichess.org/training/{}", puzzle.id), Style::default().small().center());
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notation(fen: &str, moves: &[&str]) -> String {
        let pos = Position::parse(fen).unwrap();
        let moves: Vec<String> = moves.iter().map(|m| m.to_string()).collect();
        i18n::with_lang(i18n::Lang::En, || solution_text(&pos, &moves).unwrap())
    }

    #[test]
    fn lichess_puzzle_reads_like_a_book() {
        // Puzzle kRapK : les noirs jouent, la tour blanche prend en f8, le fou reprend.
        let fen = "5r2/pp1k2p1/2np4/1Nq1N3/2B1P1b1/3P4/PPP3P1/R1B2R1K b - - 0 1";
        assert_eq!(notation(fen, &["d6e5", "f1f8", "c5f8"]), "1… dxe5 2. Rxf8 Qxf8");
        let pos = Position::parse(fen).unwrap();
        let fr = i18n::with_lang(i18n::Lang::Fr, || {
            solution_text(&pos, &["d6e5".into(), "f1f8".into(), "c5f8".into()]).unwrap()
        });
        assert_eq!(fr, "1… dxe5 2. Txf8 Dxf8");
    }

    #[test]
    fn checks_mates_castling_and_promotion() {
        // Mat du couloir.
        assert_eq!(notation("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1", &["a1a8"]), "1. Ra8#");
        // Petit roque.
        assert_eq!(notation("4k3/8/8/8/8/8/8/4K2R w K - 0 1", &["e1g1"]), "1. O-O");
        // Promotion avec échec.
        assert_eq!(notation("k7/4P3/8/8/8/8/8/4K3 w - - 0 1", &["e7e8q"]), "1. e8=Q+");
        // Deux cavaliers peuvent aller en d2 : on précise la colonne.
        assert_eq!(notation("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1", &["b1d2"]), "1. Nbd2");
    }

    #[test]
    fn illegal_moves_are_refused() {
        let pos = Position::parse("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(solution_text(&pos, &["e1e3".into()]).is_err());
        assert!(Position::parse("8/8/8 w - - 0 1").is_err());
    }

    #[test]
    fn the_board_fits_the_paper() {
        let pos = Position::parse("5r2/pp1k2p1/2np4/1Nq1N3/2B1P1b1/3P4/PPP3P1/R1B2R1K b - - 0 1").unwrap();
        let img = board_image(&pos, uci("f7e5")).unwrap();
        assert_eq!(img.width(), crate::raster::PRINT_WIDTH);
    }
}
