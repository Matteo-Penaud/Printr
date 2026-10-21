//! Tâches ménagères tournantes : chaque jour (ou chaque semaine), les tâches passent à la personne
//! suivante. La rotation se déduit de la date : rien à retenir, et deux tickets du même jour disent
//! la même chose.

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, NaiveDate};
use serde::Deserialize;

use crate::doc::{Doc, Style};
use crate::i18n;

/// Au bout de quoi les tâches tournent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    #[serde(alias = "jour")]
    Day,
    #[default]
    #[serde(alias = "semaine")]
    Week,
}

impl Period {
    pub fn label(self) -> &'static str {
        match self {
            Period::Day => i18n::t("chaque jour", "daily"),
            Period::Week => i18n::t("chaque semaine", "weekly"),
        }
    }
}

/// Le lundi de la semaine de `date`.
fn monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

/// Le numéro du tour : jours ou semaines écoulés depuis un lundi de référence.
fn turn(period: Period, date: NaiveDate) -> i64 {
    let origin = NaiveDate::from_ymd_opt(2024, 1, 1).expect("date valide");
    match period {
        Period::Day => (date - origin).num_days(),
        Period::Week => (monday(date) - origin).num_days() / 7,
    }
}

/// Qui fait quoi : pour chaque personne, ses tâches de ce tour.
fn assign<'a>(tasks: &'a [String], people: &'a [String], turn: i64) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut out: Vec<(&str, Vec<&str>)> = people.iter().map(|p| (p.as_str(), Vec::new())).collect();
    for (i, task) in tasks.iter().enumerate() {
        let who = (i as i64 + turn).rem_euclid(people.len() as i64) as usize;
        out[who].1.push(task);
    }
    out
}

fn clean(list: &[String]) -> Vec<String> {
    list.iter().map(|s| s.trim().chars().take(60).collect::<String>()).filter(|s| !s.is_empty()).take(30).collect()
}

pub fn build(tasks: &[String], people: &[String], period: Period, today: NaiveDate) -> Result<Doc> {
    let (tasks, people) = (clean(tasks), clean(people));
    if tasks.is_empty() {
        bail!("{}", i18n::t("indiquer les tâches", "give the chores"));
    }
    if people.is_empty() {
        bail!("{}", i18n::t("indiquer qui les fait", "give who does them"));
    }
    let mut doc = Doc::new();
    match period {
        Period::Day => {
            doc.header(i18n::t("Tâches du jour", "Today's chores"));
        }
        Period::Week => {
            doc.header(i18n::t("Tâches de la semaine", "This week's chores"));
            let (from, to) = (monday(today), monday(today) + Duration::days(6));
            let day = |d: NaiveDate| format!("{} {}", i18n::weekday_short(d), d.day());
            doc.text(&tr!("du {} au {}", "{} to {}", day(from), day(to)), Style::default().small().center());
        }
    }
    for (person, mine) in assign(&tasks, &people, turn(period, today)) {
        doc.feed(1);
        doc.text(person, Style::default().bold());
        if mine.is_empty() {
            doc.text(i18n::t("    Repos !", "    Day off!"), Style::default());
        }
        for task in mine {
            doc.hanging("[ ] ", task, Style::default());
        }
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn chores_rotate_each_turn() {
        let tasks = names(&["vaisselle", "poubelles", "table"]);
        let people = names(&["Léo", "Camille", "Alex"]);
        let first = assign(&tasks, &people, 0);
        assert_eq!(first, vec![("Léo", vec!["vaisselle"]), ("Camille", vec!["poubelles"]), ("Alex", vec!["table"])]);
        let next = assign(&tasks, &people, 1);
        assert_eq!(next, vec![("Léo", vec!["table"]), ("Camille", vec!["vaisselle"]), ("Alex", vec!["poubelles"])]);
        // Plus de monde que de tâches : quelqu'un se repose, à tour de rôle.
        let few = assign(&tasks[..1], &people, 2);
        assert_eq!(few[2], ("Alex", vec!["vaisselle"]));
    }

    #[test]
    fn a_week_is_one_turn() {
        let monday = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        assert_eq!(turn(Period::Week, monday), turn(Period::Week, sunday));
        assert_eq!(turn(Period::Week, sunday) + 1, turn(Period::Week, sunday + Duration::days(1)));
        assert_ne!(turn(Period::Day, monday), turn(Period::Day, sunday));
    }

    #[test]
    fn empty_lists_are_refused() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        assert!(build(&[], &names(&["Léo"]), Period::Week, today).is_err());
        assert!(build(&names(&["table"]), &names(&["  "]), Period::Week, today).is_err());
    }
}
