//! Heures locales de lever et coucher du soleil, via Open-Meteo.

use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use super::Ctx;
use crate::doc::{Doc, Style};
use crate::i18n;

#[derive(Deserialize)]
struct Forecast {
    daily: Daily,
}

#[derive(Deserialize)]
struct Daily {
    time: Vec<NaiveDate>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct SunTimes {
    sunrise: String,
    sunset: String,
}

pub fn build(ctx: &Ctx, location: &str) -> Result<Doc> {
    let location_key: String = location.bytes().map(|byte| format!("{byte:02x}")).collect();
    let key = format!("{}-sun-v1-{location_key}", ctx.today);
    let cached: Option<SunTimes> = ctx.cache.as_ref().filter(|_| !ctx.refresh).and_then(|cache| cache.get(&key));
    let from_cache = cached.is_some();
    let times = match cached {
        Some(times) => times,
        None => {
            let place = super::weather::locate(&ctx.http, location)?;
            let forecast: Forecast = ctx
                .http
                .get("https://api.open-meteo.com/v1/forecast")
                .query("latitude", place.latitude.to_string())
                .query("longitude", place.longitude.to_string())
                .query("daily", "sunrise,sunset")
                .query("timezone", "auto")
                .query("forecast_days", "7")
                .call()
                .context(i18n::t("prévisions Open-Meteo injoignables", "Open-Meteo forecast unreachable"))?
                .body_mut()
                .read_json()?;
            let index = forecast.daily.time.iter().position(|date| *date == ctx.today).context(i18n::t(
                "horaires du soleil indisponibles pour aujourd'hui",
                "sun times unavailable for today",
            ))?;
            let sunrise = forecast
                .daily
                .sunrise
                .get(index)
                .context(i18n::t("heure de lever absente", "sunrise time missing"))?
                .clone();
            let sunset = forecast
                .daily
                .sunset
                .get(index)
                .context(i18n::t("heure de coucher absente", "sunset time missing"))?
                .clone();
            let times = SunTimes { sunrise, sunset };
            if let Some(cache) = &ctx.cache {
                cache.put(&key, &times);
            }
            times
        }
    };

    let sunrise = local_time(&times.sunrise)?;
    let sunset = local_time(&times.sunset)?;
    let daylight = sunset.signed_duration_since(sunrise);
    if daylight.num_minutes() < 0 {
        bail!(
            "{}",
            i18n::t("horaires de lever et coucher du soleil incohérents", "sunrise and sunset times are inconsistent")
        );
    }
    let mut doc = Doc::new();
    doc.from_cache = from_cache;
    doc.header(&tr!("Soleil · {location}", "Sun · {location}"));
    doc.text(&i18n::capitalize(&i18n::long_date(ctx.today)), Style::default().small().center());
    doc.text(
        &tr!("Lever {} · coucher {}", "Sunrise {} · sunset {}", sunrise.format("%H:%M"), sunset.format("%H:%M")),
        Style::default().bold().center(),
    );
    doc.text(
        &tr!("Jour : {} h {:02} min", "Daylight: {}h {:02} min", daylight.num_hours(), daylight.num_minutes() % 60),
        Style::default().small().center(),
    );
    Ok(doc)
}

fn local_time(value: &str) -> Result<NaiveTime> {
    let time = value.split_once('T').map_or(value, |(_, time)| time);
    NaiveTime::parse_from_str(time, "%H:%M").context(i18n::t("heure solaire invalide", "invalid sun time"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_meteo_local_times() {
        assert_eq!(local_time("2026-10-07T07:58").unwrap(), NaiveTime::from_hms_opt(7, 58, 0).unwrap());
        assert_eq!(local_time("19:17").unwrap(), NaiveTime::from_hms_opt(19, 17, 0).unwrap());
    }
}
