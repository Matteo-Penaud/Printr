//! Nouvelle version disponible ? Régulièrement (une fois par jour par défaut,
//! `PRINTR_UPDATE_INTERVAL` minutes sinon), le serveur lit les étiquettes `vX.Y.Z` du dépôt GitHub
//! et signale à l'administrateur une version plus récente que la sienne.
//!
//! Rien n'est téléchargé ni installé : la mise à jour reste un geste humain, après lecture des
//! notes de version. Un compte GitHub piraté ne peut donc rien exécuter sur les machines.
//! `PRINTR_UPDATE_CHECK=off` coupe la vérification (aucun contact avec GitHub).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::t;

const REPO: &str = "XNinety9/Printr";
/// Première vérification peu après le démarrage, puis toutes les `interval()` minutes.
const FIRST_CHECK: Duration = Duration::from_secs(60);
const DEFAULT_MINUTES: u64 = 24 * 60;
/// Sans compte, GitHub accepte 60 requêtes par heure et par adresse : on reste très en dessous.
const MIN_MINUTES: u64 = 5;

/// Intervalle entre deux vérifications : `PRINTR_UPDATE_INTERVAL` minutes (5 au moins).
pub fn interval_minutes() -> u64 {
    minutes(std::env::var("PRINTR_UPDATE_INTERVAL").ok().as_deref())
}

fn minutes(setting: Option<&str>) -> u64 {
    setting.and_then(|v| v.trim().parse().ok()).unwrap_or(DEFAULT_MINUTES).max(MIN_MINUTES)
}

/// Version plus récente que celle qui tourne.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Available {
    pub version: String,
    /// Page de la version sur GitHub (notes, fichiers).
    pub url: String,
}

/// « v1.4.0 » ou « 1.4.0 » → (1, 4, 0) ; les préversions (« 1.4.0-rc1 ») sont ignorées.
fn parse(tag: &str) -> Option<(u32, u32, u32)> {
    let mut parts = tag.strip_prefix('v').unwrap_or(tag).split('.');
    let mut next = || parts.next()?.parse().ok();
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

/// La plus haute des étiquettes, si elle dépasse `current`.
fn newer(tags: &[String], current: &str) -> Option<Available> {
    let current = parse(current)?;
    let (tag, version) = tags.iter().filter_map(|t| Some((t, parse(t)?))).max_by_key(|(_, v)| *v)?;
    (version > current).then(|| Available {
        version: format!("{}.{}.{}", version.0, version.1, version.2),
        url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
    })
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

fn check(http: &ureq::Agent) -> Result<Option<Available>> {
    let url = format!("https://api.github.com/repos/{REPO}/tags?per_page=100");
    let tags: Vec<Tag> = http
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!("printr/", env!("CARGO_PKG_VERSION")))
        .call()
        .context(t("GitHub injoignable", "GitHub unreachable"))?
        .body_mut()
        .read_json()
        .context(t("réponse de GitHub illisible", "GitHub response unreadable"))?;
    let names: Vec<String> = tags.into_iter().map(|t| t.name).collect();
    Ok(newer(&names, env!("CARGO_PKG_VERSION")))
}

pub fn enabled() -> bool {
    !std::env::var("PRINTR_UPDATE_CHECK").is_ok_and(|v| matches!(v.trim(), "off" | "0" | "false" | "non"))
}

/// Lance la vérification quotidienne ; le résultat est rangé dans `latest`.
pub fn spawn(latest: Arc<Mutex<Option<Available>>>) {
    if !enabled() {
        return;
    }
    let interval = Duration::from_secs(interval_minutes() * 60);
    std::thread::spawn(move || {
        let http = crate::net::agent(true);
        std::thread::sleep(FIRST_CHECK);
        loop {
            match check(&http) {
                Ok(found) => {
                    let mut slot = latest.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(new) = found.as_ref().filter(|n| slot.as_ref() != Some(*n)) {
                        eprintln!("{}", tr!("nouvelle version disponible : printr {} ({})", "new version available: printr {} ({})", new.version, new.url));
                    }
                    *slot = found;
                }
                // Hors ligne, GitHub en panne : on réessaiera à la prochaine échéance.
                Err(e) => eprintln!("{}", tr!("vérification des mises à jour impossible : {e:#}", "cannot check for updates: {e:#}")),
            }
            std::thread::sleep(interval);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn versions_are_compared_as_numbers() {
        assert_eq!(parse("v1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse("1.4.0-rc1"), None);
        assert_eq!(parse("v2"), None);
        let found = newer(&tags(&["v1.3.0", "v1.10.0", "v1.9.2", "nightly"]), "1.9.9").unwrap();
        assert_eq!(found.version, "1.10.0");
        assert!(found.url.ends_with("/releases/tag/v1.10.0"));
    }

    #[test]
    fn interval_has_a_default_and_a_floor() {
        assert_eq!(minutes(None), 24 * 60);
        assert_eq!(minutes(Some("10")), 10);
        assert_eq!(minutes(Some("1")), MIN_MINUTES);
        assert_eq!(minutes(Some("bof")), 24 * 60);
    }

    #[test]
    fn nothing_when_up_to_date_or_ahead() {
        assert_eq!(newer(&tags(&["v1.2.0", "v1.3.0"]), "1.3.0"), None);
        assert_eq!(newer(&tags(&["v1.2.0"]), "1.3.0"), None);
        assert_eq!(newer(&tags(&[]), "1.3.0"), None);
    }
}
