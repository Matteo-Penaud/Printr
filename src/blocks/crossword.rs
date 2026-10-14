//! Mots croisés classiques, générés par CrossWords (https://github.com/XNinety9/CrossWords), comme
//! les mots fléchés dont ils partagent le dictionnaire, les définitions et le cache.
//!
//! Présentation selon la langue : à la française, rangées en chiffres romains et colonnes en
//! chiffres, définitions « Horizontalement » par rangée et « Verticalement » par colonne ; à
//! l'anglaise, cases numérotées et listes « Across » / « Down ».
//!
//! Définitions : le cache de CrossWords, complété à l'impression par Claude pour les mots inconnus
//! (jamais en aperçu, une seule fois par grille). Grille et définitions sont gardées par numéro.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, anyhow};
use crosswords::{Cell, GenerateOptions, Language, autodefs, letter_char, level_settings};
use image::GrayImage;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use super::Ctx;
use super::arrow_word::{dictionary, known_definitions, language, level_label, remember};
use crate::doc::{Align, Doc, Style};
use crate::draw;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    #[serde(alias = "petit", alias = "petite")]
    Small,
    #[default]
    #[serde(alias = "moyen", alias = "moyenne")]
    Medium,
    #[serde(alias = "grand", alias = "grande")]
    Large,
}

impl Size {
    /// Grilles carrées : 9 × 9 donne des cases de plus de 6 mm, de quoi écrire au stylo.
    fn side(self) -> usize {
        match self {
            Size::Small => 7,
            Size::Medium => 9,
            Size::Large => 11,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Size::Small => crate::i18n::t("petite", "small"),
            Size::Medium => crate::i18n::t("moyenne", "medium"),
            Size::Large => crate::i18n::t("grande", "large"),
        }
    }
}

/// Un mot de la grille et sa définition.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Word {
    across: bool,
    row: usize,
    col: usize,
    word: String,
    clue: Option<String>,
}

/// Grille gardée en cache, avec ce qu'on sait de ses définitions.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Saved {
    /// Une chaîne par rangée : `#` pour une case noire, la lettre sinon.
    rows: Vec<String>,
    words: Vec<Word>,
    /// Claude a déjà été sollicité : les mots restés sans définition ne lui sont pas redemandés.
    claude_tried: bool,
}

impl Saved {
    fn width(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }

    fn is_block(&self, row: usize, col: usize) -> bool {
        self.rows[row].as_bytes()[col] == b'#'
    }

    /// Mots encore sans définition, sans doublon.
    fn missing(&self) -> Vec<String> {
        let mut words: Vec<String> = self.words.iter().filter(|w| w.clue.is_none()).map(|w| w.word.clone()).collect();
        words.sort();
        words.dedup();
        words
    }

    /// Numéros des cases qui commencent un mot, dans l'ordre de lecture (présentation anglaise).
    fn numbers(&self) -> BTreeMap<(usize, usize), u32> {
        let mut starts: Vec<(usize, usize)> = self.words.iter().map(|w| (w.row, w.col)).collect();
        starts.sort();
        starts.dedup();
        starts.into_iter().zip(1..).collect()
    }
}

fn generate(lang: Language, side: usize, level: u8, seed: u64) -> Result<Saved> {
    let settings = level_settings(level);
    let dict = dictionary(lang, settings.min_score)?;
    let mut options = GenerateOptions::new(side, side);
    options.seed = seed;
    options.score_weight = settings.score_weight;
    let (solution, _) = crosswords::generate(&dict, &options)
        .map_err(|e| anyhow!("{}", tr!("grille impossible : {e}", "could not build the grid: {e}")))?;
    let grid = &solution.grid;
    let rows = (0..grid.height())
        .map(|r| {
            (0..grid.width())
                .map(|c| match grid.get(r, c) {
                    Cell::Block => '#',
                    Cell::Open(Some(l)) => letter_char(l),
                    Cell::Open(None) => '?',
                })
                .collect()
        })
        .collect();
    let defs = known_definitions(lang).at_level(level);
    let words = solution
        .entries
        .iter()
        .map(|e| Word {
            across: e.across,
            row: e.row,
            col: e.col,
            word: e.word.clone(),
            clue: defs.get(&e.word).map(str::to_owned),
        })
        .collect();
    Ok(Saved { rows, words, claude_tried: false })
}

/// Fait rédiger par Claude les définitions qui manquent.
fn complete(lang: Language, saved: &mut Saved, level: u8) -> Result<()> {
    let missing = saved.missing();
    if missing.is_empty() {
        return Ok(());
    }
    let report = autodefs::generate_for(lang, &missing, &known_definitions(lang), level, |_| {}).map_err(|e| {
        anyhow!("{}", tr!("définitions par Claude impossibles : {e}", "Claude could not write the clues: {e}"))
    })?;
    remember(lang, &report.definitions, level, &report.model)?;
    let written: HashMap<&str, &str> = report.definitions.iter().map(|(w, d)| (w.as_str(), d.as_str())).collect();
    for w in saved.words.iter_mut().filter(|w| w.clue.is_none()) {
        if let Some(clue) = written.get(w.word.as_str()) {
            w.clue = Some(clue.to_string());
        }
    }
    Ok(())
}

// ---- Rendu ----

/// Un texte en majuscules et chiffres, avec la police bitmap de `draw`.
fn label(img: &mut GrayImage, text: &str, x: i64, y: i64, scale: i64) {
    // Un nombre : `draw::number`, qui connaît le zéro (« 10 », « 11 »).
    if let Ok(n) = text.parse::<u32>() {
        return draw::number(img, n, x, y, scale);
    }
    let advance = (draw::DIGIT_W + 1) * scale;
    for (i, ch) in text.bytes().enumerate().filter(|(_, c)| c.is_ascii_uppercase()) {
        draw::letter(img, ch, x + i as i64 * advance, y, scale, draw::BLACK);
    }
}

fn label_width(text: &str, scale: i64) -> i64 {
    (text.len() as i64 * (draw::DIGIT_W + 1) - 1) * scale
}

fn render(g: &Saved, solution: bool, lang: Language) -> GrayImage {
    let side = g.width() as i64;
    let french = lang != Language::En;
    // À la française, une marge pour les rangées (chiffres romains) et une pour les colonnes.
    let scale_head = 2;
    // Le plus long des chiffres romains (VIII est plus large que IX).
    let widest = (1..=g.rows.len()).map(|k| label_width(&crosswords::roman(k), scale_head)).max().unwrap_or(0);
    let left = if french { widest + 10 } else { 0 };
    let top = if french { draw::DIGIT_H * scale_head + 8 } else { 0 };
    let cell = ((512 - 8 - left) / side).min(64);
    let grid_w = cell * side;
    let ox = (512 - grid_w + left) / 2;
    let oy = 4 + top;
    let mut img = draw::canvas((oy + grid_w + 4) as u32);
    let at = |r: usize, c: usize| (ox + c as i64 * cell, oy + r as i64 * cell);

    if french {
        for k in 0..g.width() {
            let n = (k + 1).to_string();
            let (x, _) = at(0, k);
            label(&mut img, &n, x + (cell - label_width(&n, scale_head)) / 2, 4, scale_head);
            let roman = crosswords::roman(k + 1);
            let (_, y) = at(k, 0);
            let w = label_width(&roman, scale_head);
            label(&mut img, &roman, ox - 8 - w, y + (cell - draw::DIGIT_H * scale_head) / 2, scale_head);
        }
    }

    let numbers = g.numbers();
    for r in 0..g.rows.len() {
        for c in 0..g.width() {
            let (x, y) = at(r, c);
            if g.is_block(r, c) {
                draw::fill_rect(&mut img, x, y, cell, cell);
            } else if solution {
                let ch = g.rows[r].as_bytes()[c];
                let scale = (cell * 9 / 20 / draw::DIGIT_H).max(1);
                let (lw, lh) = (draw::DIGIT_W * scale, draw::DIGIT_H * scale);
                draw::letter(&mut img, ch, x + (cell - lw) / 2, y + (cell - lh) / 2, scale, draw::BLACK);
            } else if !french && let Some(&n) = numbers.get(&(r, c)) {
                draw::number(&mut img, n, x + 4, y + 4, 2);
            }
        }
    }
    for k in 0..=side {
        let w = if k == 0 || k == side { 3 } else { 2 };
        draw::fill_rect(&mut img, ox + k * cell - w / 2, oy, w, grid_w);
        draw::fill_rect(&mut img, ox, oy + k * cell - w / 2, grid_w, w);
    }
    img
}

/// La définition d'un mot, ou sa longueur si personne n'a su l'écrire.
fn clue(w: &Word) -> String {
    match &w.clue {
        Some(c) => c.clone(),
        None => tr!("({} lettres)", "({} letters)", w.word.len()),
    }
}

/// Définitions à la française : une ligne par rangée (horizontalement) et par colonne
/// (verticalement), les mots d'une même ligne séparés par un tiret.
fn french_clues(doc: &mut Doc, g: &Saved) {
    for (across, title) in [(true, "Horizontalement"), (false, "Verticalement")] {
        doc.feed(1);
        doc.text(title, Style::default().bold().underline());
        for line in 0..g.rows.len().max(g.width()) {
            let mut here: Vec<&Word> =
                g.words.iter().filter(|w| w.across == across && if across { w.row } else { w.col } == line).collect();
            if here.is_empty() {
                continue;
            }
            here.sort_by_key(|w| if across { w.col } else { w.row });
            let head = if across { crosswords::roman(line + 1) } else { (line + 1).to_string() };
            let text: Vec<String> = here.into_iter().map(clue).collect();
            doc.hanging(&format!("{head:>4}. "), &text.join(" – "), Style::default());
        }
    }
}

/// Définitions à l'anglaise : « Across » puis « Down », chaque mot précédé du numéro de sa case.
fn english_clues(doc: &mut Doc, g: &Saved) {
    let numbers = g.numbers();
    for (across, title) in [(true, "Across"), (false, "Down")] {
        doc.feed(1);
        doc.text(title, Style::default().bold().underline());
        let mut words: Vec<(u32, &Word)> =
            g.words.iter().filter(|w| w.across == across).map(|w| (numbers[&(w.row, w.col)], w)).collect();
        words.sort_by_key(|(n, _)| *n);
        for (n, w) in words {
            doc.hanging(&format!("{n:>3}. "), &clue(w), Style::default());
        }
    }
}

pub struct Options {
    pub size: Size,
    pub level: u8,
    pub seed: Option<u64>,
    pub solution: bool,
}

pub fn build(ctx: &Ctx, options: &Options) -> Result<Doc> {
    let level = options.level.clamp(1, 4);
    let seed = options.seed.unwrap_or_else(|| rand::rng().random_range(1..100_000));
    let side = options.size.side();
    let lang = language();
    let key = format!("mots-croises-v1-{side}-n{level}-{seed}-{}", lang.code());
    let cache = ctx.cache.as_ref();

    let cached: Option<Saved> = cache.filter(|_| !ctx.refresh).and_then(|c| c.get(&key));
    let from_cache = cached.is_some();
    let mut grid = match cached {
        Some(saved) => saved,
        None => generate(lang, side, level, seed)?,
    };
    // Définitions manquantes : Claude, une seule fois, à l'impression (l'aperçu ne coûte rien).
    let mut note = None;
    let mut changed = !from_cache;
    if !grid.missing().is_empty() && !grid.claude_tried && !ctx.preview && !options.solution {
        if let Err(e) = complete(lang, &mut grid, level) {
            note = Some(tr!(
                "Définitions indisponibles ({e}) : la longueur est indiquée.",
                "Clues unavailable ({e}): the word length is shown."
            ));
        }
        grid.claude_tried = true;
        changed = true;
    }
    if grid.claude_tried && note.is_none() && !grid.missing().is_empty() && !options.solution {
        let n = grid.missing().len();
        note = Some(tr!(
            "{n} définition(s) n'ont pas pu être rédigées : la longueur est indiquée.",
            "{n} clue(s) could not be written: the word length is shown."
        ));
    }
    if let Some(cache) = cache
        && changed
    {
        cache.put(&key, &grid);
    }

    let mut doc = Doc::new();
    doc.from_cache = from_cache && grid.missing().is_empty();
    if options.solution {
        doc.header(crate::i18n::t("Solution des mots croisés", "Crossword solution"));
    } else {
        doc.header(&tr!("Mots croisés · {}", "Crossword · {}", level_label(level)));
    }
    doc.feed(1);
    doc.image(render(&grid, options.solution, lang));
    if !options.solution {
        if lang == Language::En {
            english_clues(&mut doc, &grid)
        } else {
            french_clues(&mut doc, &grid)
        }
        if ctx.preview && !grid.missing().is_empty() {
            doc.feed(1);
            doc.text(
                crate::i18n::t(
                    "Les définitions manquantes seront rédigées par Claude à l'impression.",
                    "Missing clues will be written by Claude when printing.",
                ),
                Style::default().small().center(),
            );
        }
    }
    if let Some(note) = note {
        doc.feed(1);
        doc.text(&note, Style::default().small().center());
    }
    doc.text(
        &tr!("n° {seed} · grille {} · niveau {level}", "No. {seed} · {} grid · level {level}", options.size.label()),
        Style::default().small().align(Align::Right),
    );
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Saved {
        // C A P
        // # R E
        // B I S
        let w = |across, row, col, word: &str, clue: Option<&str>| Word {
            across,
            row,
            col,
            word: word.to_owned(),
            clue: clue.map(str::to_owned),
        };
        Saved {
            rows: vec!["CAP".into(), "#RE".into(), "BIS".into()],
            words: vec![
                w(true, 0, 0, "CAP", Some("Pointe")),
                w(true, 1, 1, "RE", None),
                w(true, 2, 0, "BIS", Some("Encore")),
                w(false, 0, 1, "ARI", None),
                w(false, 0, 2, "PES", Some("Pèse")),
            ],
            claude_tried: false,
        }
    }

    #[test]
    fn numbers_follow_reading_order() {
        let n = sample().numbers();
        assert_eq!(n[&(0, 0)], 1);
        assert_eq!(n[&(0, 1)], 2);
        assert_eq!(n[&(0, 2)], 3);
        assert_eq!(n[&(1, 1)], 4);
        assert_eq!(n[&(2, 0)], 5);
    }

    /// Grande grille : colonnes 10 et 11, rangées X et XI.
    #[test]
    fn eleven_columns_render() {
        let rows: Vec<String> = (0..11).map(|_| "ABCDEFGHIJK".to_owned()).collect();
        let g = Saved { rows, words: vec![], claude_tried: false };
        for lang in [Language::Fr, Language::En] {
            assert_eq!(render(&g, true, lang).width(), 512);
        }
    }

    #[test]
    fn missing_clues_are_listed_once() {
        assert_eq!(sample().missing(), ["ARI", "RE"]);
    }

    #[test]
    fn renders_both_layouts_within_paper() {
        let g = sample();
        for lang in [Language::Fr, Language::En] {
            for solution in [false, true] {
                assert_eq!(render(&g, solution, lang).width(), 512);
            }
        }
        let mut doc = Doc::new();
        french_clues(&mut doc, &g);
        let lines: Vec<String> = doc
            .ops
            .iter()
            .filter_map(|op| if let crate::doc::Op::Line { text, .. } = op { Some(text.clone()) } else { None })
            .collect();
        assert!(lines.iter().any(|l| l.contains("I. Pointe")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("(2 lettres)")), "{lines:#?}");
    }
}
