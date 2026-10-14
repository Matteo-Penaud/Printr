//! Message codé : une phrase chiffrée (César, morse ou nombres) avec de quoi la déchiffrer,
//! et la réponse imprimée à l'envers en bas.

use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use serde::Deserialize;

use crate::doc::{Doc, Style};
use crate::i18n::{en, t};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cipher {
    /// Chaque lettre décalée dans l'alphabet.
    #[default]
    #[serde(alias = "césar", alias = "caesar")]
    Cesar,
    Morse,
    /// A = 1, B = 2…
    #[serde(alias = "numbers")]
    Nombres,
}

impl Cipher {
    pub fn label(self) -> &'static str {
        match self {
            Cipher::Cesar => t("César", "Caesar"),
            Cipher::Morse => "morse",
            Cipher::Nombres => t("nombres", "numbers"),
        }
    }
}

/// Messages en anglais, en lettres simples (sans apostrophe : elle serait remplacée par un espace).
#[rustfmt::skip]
const MESSAGES_EN: &[&str] = &[
    "The treasure is hidden under the couch",
    "Tonight we are having pizza",
    "Meet in the kitchen at eight sharp",
    "The cat ate my homework",
    "You are a top secret agent",
    "The password is banana",
    "Look behind the living room door",
    "Whoever reads this must give a hug",
    "There is chocolate left in the cupboard",
    "Complete the mission and return to base",
    "Aliens are landing on Tuesday",
    "Tell nobody anything",
    "Check the mailbox right now",
    "Tomorrow we go to the park",
];

/// Messages tirés au hasard quand le ticket n'en donne pas.
fn messages() -> &'static [&'static str] {
    if en() { MESSAGES_EN } else { MESSAGES }
}

#[rustfmt::skip]
const MESSAGES: &[&str] = &[
    "Le trésor est caché sous le canapé",
    "Ce soir, c'est pizza",
    "Rendez-vous dans la cuisine à huit heures",
    "Le chat a mangé mes devoirs",
    "Tu es un agent secret de première classe",
    "Le mot de passe est banane",
    "Regarde derrière la porte du salon",
    "Celui qui lit ceci doit faire un câlin",
    "Il reste du chocolat dans le placard",
    "Mission accomplie, rentre à la base",
    "Les extraterrestres arrivent mardi",
    "Ne dis rien à personne",
    "Va voir dans la boîte aux lettres",
    "Demain, on va au parc",
];

#[rustfmt::skip]
const MORSE: [&str; 26] = [
    ".-", "-...", "-.-.", "-..", ".", "..-.", "--.", "....", "..", ".---", "-.-", ".-..", "--", "-.", "---", ".--.",
    "--.-", ".-.", "...", "-", "..-", "...-", ".--", "-..-", "-.--", "--..",
];

/// Majuscules sans accents ; tout ce qui n'est pas une lettre devient un espace.
fn normalize(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let form = super::word_search::grid_form(&c.to_string());
        if form.is_empty() {
            out.push(' ');
        } else {
            out.push_str(&form);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn letters(word: &str) -> impl Iterator<Item = usize> + '_ {
    word.bytes().map(|b| (b - b'A') as usize)
}

fn encode(text: &str, cipher: Cipher, shift: u8) -> String {
    let words = text.split(' ');
    match cipher {
        Cipher::Cesar => words
            .map(|w| letters(w).map(|i| (b'A' + ((i + shift as usize) % 26) as u8) as char).collect::<String>())
            .collect::<Vec<_>>()
            .join(" "),
        Cipher::Morse => {
            words.map(|w| letters(w).map(|i| MORSE[i]).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" / ")
        }
        Cipher::Nombres => words
            .map(|w| letters(w).map(|i| (i + 1).to_string()).collect::<Vec<_>>().join("-"))
            .collect::<Vec<_>>()
            .join(" / "),
    }
}

/// Lignes d'aide au déchiffrement, en petit (56 colonnes).
fn key(cipher: Cipher, shift: u8) -> Vec<String> {
    let alphabet: Vec<char> = ('A'..='Z').collect();
    match cipher {
        Cipher::Cesar => {
            let shifted: String = alphabet.iter().map(|&c| (b'A' + (c as u8 - b'A' + shift) % 26) as char).collect();
            vec![
                tr!(
                    "Chaque lettre a avancé de {shift} : A est devenu {}.",
                    "Each letter moved forward by {shift}: A became {}.",
                    &shifted[..1]
                ),
                t(
                    "En haut la lettre codée, en dessous la vraie lettre :",
                    "Top: the coded letter. Bottom: the real one:",
                )
                .to_owned(),
                String::new(),
                shifted.chars().map(String::from).collect::<Vec<_>>().join(" "),
                alphabet.iter().map(char::to_string).collect::<Vec<_>>().join(" "),
            ]
        }
        Cipher::Morse => {
            let cells: Vec<String> = alphabet.iter().zip(MORSE).map(|(c, m)| format!("{c} {m:<6}")).collect();
            let hint = t(
                "Un espace entre les lettres, « / » entre les mots.",
                "One space between letters, \"/\" between words.",
            );
            let mut lines = vec![hint.to_owned(), String::new()];
            lines.extend(cells.chunks(6).map(|row| row.join(" ").trim_end().to_owned()));
            lines
        }
        Cipher::Nombres => {
            let cells: Vec<String> = alphabet.iter().enumerate().map(|(i, c)| format!("{c}={:<2}", i + 1)).collect();
            let hint = t(
                "Chaque nombre est le rang d'une lettre dans l'alphabet.",
                "Each number is a letter's place in the alphabet.",
            );
            let mut lines = vec![hint.to_owned(), String::new()];
            lines.extend(cells.chunks(9).map(|row| row.join(" ").trim_end().to_owned()));
            lines
        }
    }
}

pub fn build(message: Option<&str>, cipher: Cipher, shift: Option<u8>, answer: bool, seed: Option<u64>) -> Doc {
    let mut rng = match seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_rng(&mut rand::rng()),
    };
    let plain =
        message.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| messages().choose(&mut rng).expect("messages"));
    let shift = shift.map(|s| s % 26).filter(|&s| s != 0).unwrap_or_else(|| rng.random_range(1..26));
    let coded = encode(&normalize(plain), cipher, shift);

    let mut doc = Doc::new();
    doc.header(t("Message codé", "Secret message"));
    doc.text(&tr!("Code : {}", "Code: {}", cipher.label()), Style::default().small().center());
    doc.feed(1);
    let style = if cipher == Cipher::Cesar { Style::default().bold().size(2) } else { Style::default().bold() };
    doc.text(&coded, style.center());
    doc.feed(1);
    for line in key(cipher, shift) {
        doc.line(&line, Style::default().small());
    }
    if answer {
        doc.feed(1);
        doc.text(&tr!("Message : {plain}", "Message: {plain}"), Style::default().small().upside_down());
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Op;

    #[test]
    fn encodes_each_cipher() {
        assert_eq!(normalize("Ce soir, c'est pizza !"), "CE SOIR C EST PIZZA");
        assert_eq!(encode("ABC XYZ", Cipher::Cesar, 3), "DEF ABC");
        assert_eq!(encode("SOS", Cipher::Morse, 0), "... --- ...");
        assert_eq!(encode("AB C", Cipher::Nombres, 0), "1-2 / 3");
    }

    #[test]
    fn english_messages_are_plain_letters() {
        use crate::i18n::{Lang, with_lang};
        with_lang(Lang::En, || {
            assert!(!messages().is_empty());
            assert_eq!(messages().len(), 14);
            for m in messages() {
                assert!(m.chars().all(|c| c.is_ascii_alphabetic() || c == ' '), "{m}");
                assert_eq!(normalize(m), m.to_ascii_uppercase(), "{m}");
            }
            for cipher in [Cipher::Cesar, Cipher::Morse, Cipher::Nombres] {
                for seed in 0..30 {
                    let doc = build(None, cipher, None, true, Some(seed));
                    for op in &doc.ops {
                        if let Op::Line { text, style } = op {
                            assert!(text.chars().count() <= style.columns(), "{text}");
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn lines_fit() {
        for cipher in [Cipher::Cesar, Cipher::Morse, Cipher::Nombres] {
            for seed in 0..30 {
                let doc = build(None, cipher, None, true, Some(seed));
                for op in &doc.ops {
                    if let Op::Line { text, style } = op {
                        assert!(text.chars().count() <= style.columns(), "{text}");
                    }
                }
            }
        }
    }
}
