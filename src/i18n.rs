//! Langue de l'installation : `PRINTR_LANG=en` passe l'interface, les messages et les tickets en
//! anglais ; le français reste la langue par défaut.
//!
//! Les traductions sont écrites à côté du texte français, là où il sert :
//! `t("Bonjour", "Hello")` pour une chaîne fixe, `tr!("{n} blocs", "{n} blocks")` pour une chaîne
//! formatée. Pas de catalogue à part : une phrase et sa traduction ne peuvent pas se perdre de vue.

use std::cell::Cell;
use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    fn parse(value: &str) -> Option<Lang> {
        match value.trim().to_ascii_lowercase().get(..2)? {
            "fr" => Some(Lang::Fr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// Code BCP 47, pour `<html lang>` et les en-têtes HTTP.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
}

static LANG: OnceLock<Lang> = OnceLock::new();

thread_local! {
    /// Langue imposée au fil courant (tests, rendu des exemples dans les deux langues).
    static FORCED: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Langue en vigueur : `PRINTR_LANG` (« en », « en_US.UTF-8 »…), français sinon.
pub fn lang() -> Lang {
    FORCED.get().unwrap_or_else(|| *LANG.get_or_init(from_env))
}

#[cfg(not(test))]
fn from_env() -> Lang {
    std::env::var("PRINTR_LANG").ok().as_deref().and_then(Lang::parse).unwrap_or(Lang::Fr)
}

/// Les tests ne dépendent pas de l'environnement : français par défaut, `with_lang` pour l'anglais.
#[cfg(test)]
fn from_env() -> Lang {
    Lang::Fr
}

pub fn en() -> bool {
    lang() == Lang::En
}

/// Exécute `f` dans la langue donnée, sur ce fil seulement.
#[cfg(test)]
pub fn with_lang<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let before = FORCED.replace(Some(lang));
    let out = f();
    FORCED.set(before);
    out
}

/// La chaîne de la langue en vigueur.
pub fn t(fr: &'static str, en: &'static str) -> &'static str {
    if self::en() { en } else { fr }
}

/// `format!` dans la langue en vigueur : `tr!("{n} blocs", "{n} blocks")`.
#[macro_export]
macro_rules! tr {
    ($fr:literal, $en:literal $(,)?) => {
        if $crate::i18n::en() { format!($en) } else { format!($fr) }
    };
    ($fr:literal, $en:literal, $($arg:tt)+) => {
        if $crate::i18n::en() { format!($en, $($arg)+) } else { format!($fr, $($arg)+) }
    };
}

const DAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
];
const DAYS_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS_EN: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December",
];

/// « mercredi », « Wednesday ».
pub fn weekday(date: NaiveDate) -> &'static str {
    let i = date.weekday().num_days_from_monday() as usize;
    t(DAYS[i], DAYS_EN[i])
}

/// « mer. », « Wed ».
pub fn weekday_short(date: NaiveDate) -> String {
    let day = weekday(date);
    if en() { day[..3].to_owned() } else { format!("{}.", &day[..3]) }
}

/// « octobre », « October ».
pub fn month(month0: u32) -> &'static str {
    t(MONTHS[month0 as usize], MONTHS_EN[month0 as usize])
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// « 1er octobre », « 7 octobre » ; « October 1 », « October 7 ».
pub fn day_month(date: NaiveDate) -> String {
    let name = month(date.month0());
    match (en(), date.day()) {
        (true, d) => format!("{name} {d}"),
        (false, 1) => format!("1er {name}"),
        (false, d) => format!("{d} {name}"),
    }
}

/// « mardi 7 octobre 2026 », « Tuesday, October 7, 2026 ».
pub fn long_date(date: NaiveDate) -> String {
    if en() {
        format!("{}, {}, {}", weekday(date), day_month(date), date.year())
    } else {
        format!("{} {} {}", weekday(date), day_month(date), date.year())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        with_lang(Lang::Fr, || {
            assert_eq!(long_date(d), "mercredi 7 octobre 2026");
            assert_eq!(weekday_short(d), "mer.");
            let d = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
            assert_eq!(capitalize(&long_date(d)), "Samedi 1er août 2026");
        });
        with_lang(Lang::En, || {
            assert_eq!(long_date(d), "Wednesday, October 7, 2026");
            assert_eq!(weekday_short(d), "Wed");
            assert_eq!(tr!("{} blocs", "{} blocks", 3), "3 blocks");
        });
    }

    #[test]
    fn parses_locales() {
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::parse("FR"), Some(Lang::Fr));
        assert_eq!(Lang::parse("de"), None);
    }
}
