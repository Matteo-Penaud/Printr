//! Bons à offrir : « Bon pour un petit-déjeuner au lit », avec frise de cœurs, numéro de série
//! et ligne de découpe. Plusieurs bons à la suite si on en demande plusieurs.

use chrono::NaiveDate;
use image::GrayImage;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};

use super::picto::Shape;
use crate::doc::{Doc, Style};
use crate::{draw, i18n};

/// Idées de bons, quand le ticket n'en précise pas.
#[rustfmt::skip]
const IDEAS: &[&str] = &[
    "un petit-déjeuner au lit",
    "une vaisselle faite sans râler",
    "un câlin illimité",
    "une soirée film de ton choix",
    "un massage des épaules",
    "une grasse matinée tranquille",
    "un dessert maison",
    "une balade rien que tous les deux",
    "un joker corvée",
    "une soirée sans écrans",
    "une partie du jeu de ton choix",
    "un chocolat chaud avec chantilly",
    "une journée où c'est toi qui décides",
    "un pique-nique",
];

/// Mêmes idées, en anglais.
#[rustfmt::skip]
const IDEAS_EN: &[&str] = &[
    "breakfast in bed",
    "one dishwashing session without complaining",
    "an unlimited hug",
    "a movie night of your choice",
    "a shoulder massage",
    "a long lie-in",
    "a homemade dessert",
    "a walk, just the two of you",
    "a chore joker",
    "an evening with no screens",
    "one game of your choice",
    "a hot chocolate with whipped cream",
    "a day where you make all the calls",
    "a picnic",
];

const CUT: &str = "- - - - - - - - - - 8< - - - - - - - - - -";

/// Frise de petits cœurs entre deux filets, sur toute la largeur.
fn frieze() -> GrayImage {
    const SIDE: u32 = 18;
    const STEP: u32 = 32;
    let mut img = draw::canvas(SIDE + 14);
    let width = img.width();
    draw::fill_rect(&mut img, 0, 0, width as i64, 2);
    draw::fill_rect(&mut img, 0, (SIDE + 12) as i64, width as i64, 2);
    let count = width / STEP;
    let left = (width - count * STEP) / 2 + (STEP - SIDE) / 2;
    for k in 0..count {
        for py in 0..SIDE {
            for px in 0..SIDE {
                let (x, y) = (px as f64 / (SIDE - 1) as f64 * 2.0 - 1.0, py as f64 / (SIDE - 1) as f64 * 2.0 - 1.0);
                if Shape::Heart.ink(x, y) {
                    img.put_pixel(left + k * STEP + px, 7 + py, draw::BLACK);
                }
            }
        }
    }
    img
}

/// Numéro de série du bon : « 2026-4F7A ».
fn serial(today: NaiveDate, rng: &mut StdRng) -> String {
    format!("{}-{:04X}", today.format("%Y"), rng.random::<u16>())
}

pub struct Coupon<'a> {
    pub text: Option<&'a str>,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub valid_until: Option<NaiveDate>,
    pub count: u8,
}

pub fn build(coupon: &Coupon, today: NaiveDate) -> Doc {
    let mut rng = StdRng::from_rng(&mut rand::rng());
    let count = coupon.count.clamp(1, 6) as usize;
    let mut ideas = if i18n::en() { IDEAS_EN.to_vec() } else { IDEAS.to_vec() };
    ideas.shuffle(&mut rng);
    let texts: Vec<&str> = match coupon.text.filter(|t| !t.trim().is_empty()) {
        Some(text) => vec![text; count],
        None => ideas.into_iter().take(count).collect(),
    };

    let mut doc = Doc::new();
    for text in texts {
        doc.line(CUT, Style::default());
        doc.feed(1);
        doc.image(frieze());
        doc.feed(1);
        // `line` et non `text` : les espaces entre les lettres sont gardés tels quels.
        doc.line(i18n::t("B O N   P O U R", "C O U P O N   F O R"), Style::default().bold().center().size(2));
        doc.feed(1);
        doc.text(text.trim(), Style::default().bold().center().size(2));
        doc.feed(1);
        let people = match (coupon.from, coupon.to) {
            (Some(from), Some(to)) => Some(tr!("Offert par {from} à {to}", "Given by {from} to {to}")),
            (Some(from), None) => Some(tr!("Offert par {from}", "Given by {from}")),
            (None, Some(to)) => Some(tr!("Pour {to}", "For {to}")),
            (None, None) => None,
        };
        if let Some(people) = people {
            doc.text(&people, Style::default().center());
        }
        let validity = match coupon.valid_until {
            Some(date) => tr!("Valable jusqu'au {} {}", "Valid until {}, {}", i18n::day_month(date), date.format("%Y")),
            None => i18n::t("Valable pour toujours (ou presque)", "Valid forever (or almost)").to_owned(),
        };
        doc.text(&validity, Style::default().center());
        doc.feed(1);
        doc.text(
            i18n::t(
                "Non remboursable, non échangeable, à utiliser sans modération.",
                "Non-refundable, non-exchangeable, use without moderation.",
            ),
            Style::default().small().center(),
        );
        doc.text(&tr!("Bon n° {}", "Coupon No. {}", serial(today, &mut rng)), Style::default().small().center());
        doc.feed(1);
        doc.image(frieze());
        doc.feed(1);
    }
    doc.line(CUT, Style::default());
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Op;

    #[test]
    fn distinct_ideas_and_lines_fit() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        let coupon = Coupon { text: None, from: Some("Alex"), to: Some("Camille"), valid_until: None, count: 3 };
        let doc = build(&coupon, today);
        let lines: Vec<&str> = doc
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Line { text, style } => {
                    assert!(text.chars().count() <= style.columns(), "{text}");
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(lines.iter().filter(|l| l.contains("8<")).count(), 4);
        assert_eq!(lines.iter().filter(|l| l.contains("Offert par Alex à Camille")).count(), 3);
    }

    #[test]
    fn english_coupons_fit() {
        use crate::i18n::{Lang, with_lang};
        let today = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        let coupon = Coupon {
            text: None,
            from: Some("Alex"),
            to: Some("Camille"),
            valid_until: NaiveDate::from_ymd_opt(2026, 10, 31),
            count: 3,
        };
        with_lang(Lang::En, || {
            let doc = build(&coupon, today);
            let lines: Vec<&str> = doc
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::Line { text, style } => {
                        assert!(text.chars().count() <= style.columns(), "{text}");
                        Some(text.as_str())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(lines.iter().filter(|l| l.contains("C O U P O N   F O R")).count(), 3);
            assert_eq!(lines.iter().filter(|l| l.contains("Given by Alex to Camille")).count(), 3);
            assert!(lines.iter().any(|l| l.contains("Valid until October 31, 2026")));
            assert!(lines.iter().any(|l| l.contains("Coupon No. 2026-")));
        });
    }
}
