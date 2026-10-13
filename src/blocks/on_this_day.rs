//! Éphéméride : événements historiques du jour, via Wikipédia en français.

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;

use crate::doc::{Doc, Style};
use crate::i18n;

#[derive(Deserialize)]
struct Feed {
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Event {
    text: String,
    year: Option<i32>,
}

pub fn build(agent: &ureq::Agent, today: NaiveDate, count: u8) -> Result<Doc> {
    let url = format!(
        "https://api.wikimedia.org/feed/v1/wikipedia/{}/onthisday/events/{:02}/{:02}",
        i18n::lang().code(),
        today.month(),
        today.day()
    );
    let mut feed: Feed = agent
        .get(&url)
        .header("User-Agent", "printr/0.1 (imprimante à tickets personnelle)")
        .call()
        .context(i18n::t("Wikipédia injoignable", "Wikipedia unreachable"))?
        .body_mut()
        .read_json()?;

    // Tirage stable sur la journée, puis ordre chronologique.
    feed.events.retain(|e| e.year.is_some());
    feed.events.shuffle(&mut StdRng::seed_from_u64(today.num_days_from_ce() as u64));
    let mut events: Vec<_> = feed.events.into_iter().take(count.clamp(1, 10) as usize).collect();
    events.sort_by_key(|e| e.year);

    let mut doc = Doc::new();
    doc.header(&tr!("Un {} dans l'histoire", "On this day, {}", i18n::day_month(today)));
    for e in events {
        doc.feed(1);
        doc.hanging(&tr!("{} : ", "{}: ", e.year.unwrap_or_default()), &i18n::capitalize(&e.text), Style::default());
    }
    Ok(doc)
}
