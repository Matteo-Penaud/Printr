//! Météo via Open-Meteo (gratuit, sans clé) : géocodage puis prévisions.

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::doc::{Doc, Style};
use crate::i18n;

#[derive(Deserialize)]
struct GeoResults {
    results: Option<Vec<Place>>,
}

#[derive(Deserialize)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Deserialize)]
struct Forecast {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f64,
    apparent_temperature: f64,
    weather_code: u8,
    wind_speed_10m: f64,
}

#[derive(Deserialize)]
struct Daily {
    time: Vec<NaiveDate>,
    weather_code: Vec<u8>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    precipitation_probability_max: Vec<Option<f64>>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
    uv_index_max: Vec<Option<f64>>,
}

/// Libellé d'un code météo WMO, en français ou en anglais.
fn describe(code: u8) -> &'static str {
    use crate::i18n::t;
    match code {
        0 => t("Ciel dégagé", "Clear sky"),
        1 => t("Plutôt dégagé", "Mainly clear"),
        2 => t("Partiellement nuageux", "Partly cloudy"),
        3 => t("Couvert", "Overcast"),
        45 => t("Brouillard", "Fog"),
        48 => t("Brouillard givrant", "Rime fog"),
        51 => t("Bruine légère", "Light drizzle"),
        53 => t("Bruine", "Drizzle"),
        55 => t("Bruine dense", "Dense drizzle"),
        56 | 57 => t("Bruine verglaçante", "Freezing drizzle"),
        61 => t("Pluie faible", "Light rain"),
        63 => t("Pluie", "Rain"),
        65 => t("Forte pluie", "Heavy rain"),
        66 | 67 => t("Pluie verglaçante", "Freezing rain"),
        71 => t("Neige faible", "Light snow"),
        73 => t("Neige", "Snow"),
        75 => t("Forte neige", "Heavy snow"),
        77 => t("Grains de neige", "Snow grains"),
        80 => t("Averses faibles", "Light showers"),
        81 => t("Averses", "Showers"),
        82 => t("Fortes averses", "Heavy showers"),
        85 => t("Averses de neige", "Snow showers"),
        86 => t("Fortes averses de neige", "Heavy snow showers"),
        95 => t("Orage", "Thunderstorm"),
        96 | 99 => t("Orage et grêle", "Thunderstorm with hail"),
        _ => t("Temps indéterminé", "Unknown weather"),
    }
}

/// « Lyon », ou des coordonnées « 45.76,4.83 ».
pub fn locate(agent: &ureq::Agent, location: &str) -> Result<Place> {
    if let Some((lat, lon)) = location.split_once(',') {
        if let (Ok(latitude), Ok(longitude)) = (lat.trim().parse(), lon.trim().parse()) {
            return Ok(Place { name: location.to_owned(), latitude, longitude });
        }
    }
    let geo: GeoResults = agent
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query("name", location)
        .query("count", "1")
        .query("language", i18n::lang().code())
        .call()
        .context(i18n::t("géocodage injoignable", "geocoding unreachable"))?
        .body_mut()
        .read_json()?;
    match geo.results.and_then(|r| r.into_iter().next()) {
        Some(place) => Ok(place),
        None => bail!("{}", tr!("lieu introuvable : {location}", "place not found: {location}")),
    }
}

/// Heure « HH:MM » d'un horodatage ISO « 2026-10-07T07:52 ».
fn hour(iso: &str) -> &str {
    iso.split_once('T').map_or(iso, |(_, t)| t)
}

pub fn build(agent: &ureq::Agent, location: &str, days: u8) -> Result<Doc> {
    let days = days.clamp(1, 7);
    let place = locate(agent, location)?;
    let f: Forecast = agent
        .get("https://api.open-meteo.com/v1/forecast")
        .query("latitude", place.latitude.to_string())
        .query("longitude", place.longitude.to_string())
        .query("current", "temperature_2m,apparent_temperature,weather_code,wind_speed_10m")
        .query(
            "daily",
            "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,\
             sunrise,sunset,uv_index_max",
        )
        .query("timezone", "auto")
        .query("forecast_days", days.to_string())
        .call()
        .context(i18n::t("Open-Meteo injoignable", "Open-Meteo unreachable"))?
        .body_mut()
        .read_json()?;
    let (c, d) = (&f.current, &f.daily);
    let pct = |v: Option<f64>| v.map_or("-".to_owned(), |p| format!("{p:.0}%"));

    let mut doc = Doc::new();
    doc.header(&tr!("Météo · {}", "Weather · {}", place.name));
    doc.text(describe(c.weather_code), Style::default().bold().center());
    doc.text(
        &tr!(
            "{:.0}°C (ressenti {:.0}°C)",
            "{:.0}°C (feels {:.0}°C)",
            c.temperature_2m,
            c.apparent_temperature
        ),
        Style::default().size(2).center(),
    );
    doc.feed(1);
    doc.text(
        &tr!(
            "Min {:.0}°C · Max {:.0}°C · Pluie {}",
            "Min {:.0}°C · Max {:.0}°C · Rain {}",
            d.temperature_2m_min[0],
            d.temperature_2m_max[0],
            pct(d.precipitation_probability_max[0])
        ),
        Style::default().center(),
    );
    let uv = d.uv_index_max[0].map_or("-".to_owned(), |u| format!("{u:.0}"));
    doc.text(&tr!("Vent {:.0} km/h · UV {uv}", "Wind {:.0} km/h · UV {uv}", c.wind_speed_10m), Style::default().center());
    doc.text(
        &tr!("Soleil {} - {}", "Sun {} - {}", hour(&d.sunrise[0]), hour(&d.sunset[0])),
        Style::default().center(),
    );

    if d.time.len() > 1 {
        doc.feed(1);
        for i in 1..d.time.len() {
            let date = d.time[i];
            let line = format!(
                "{:<9}{:<24.24}{:>3}/{:>3}° {:>4}",
                format!("{} {}", i18n::weekday_short(date), date.format("%d")),
                describe(d.weather_code[i]),
                format!("{:.0}", d.temperature_2m_min[i]),
                format!("{:.0}", d.temperature_2m_max[i]),
                pct(d.precipitation_probability_max[i]),
            );
            doc.line(&line, Style::default().small());
        }
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_codes_follow_language() {
        assert_eq!(describe(3), "Couvert");
        crate::i18n::with_lang(crate::i18n::Lang::En, || assert_eq!(describe(3), "Overcast"));
    }
}
