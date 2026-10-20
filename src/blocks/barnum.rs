//! Horoscope hors ligne et gratuit, calculé sur le ciel réel par Barnum
//! (https://github.com/XNinety9/Barnum), appelé en ligne de commande avec `--json`.
//!
//! Commande : `$PRINTR_BARNUM` (par exemple `python3 /opt/barnum/main.py`), sinon `barnum`.
//!
//! Barnum écrit en français, et en anglais depuis sa version 0.3 (`--lang en`). En anglais, un
//! Barnum plus ancien refuse l'option : on redemande alors la lecture en français, et tout le
//! ticket reste en français plutôt que mi-anglais, mi-français.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::doc::{Doc, Style};
use crate::i18n;

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct Reading {
    /// Langue de rédaction (« fr », « en ») ; absente avant Barnum 0.3, qui n'écrit qu'en français.
    #[serde(default)]
    langue: Option<String>,
    signe: String,
    #[serde(default)]
    lune: Option<Moon>,
    #[serde(default)]
    climat: String,
    #[serde(default)]
    domaines: Vec<Domain>,
    #[serde(default)]
    conseil: String,
    #[serde(default)]
    couleur: String,
    #[serde(default)]
    chiffre: Option<i64>,
    #[serde(default)]
    humeur: String,
    #[serde(default)]
    ciel: Vec<Body>,
}

#[derive(Deserialize)]
struct Moon {
    phase: String,
    signe: String,
}

#[derive(Deserialize)]
struct Domain {
    cle: String,
    /// Nom affiché dans la langue de la lecture (Barnum 0.3) ; sinon, la clé fait l'affaire.
    #[serde(default)]
    libelle: Option<String>,
    note: u8,
    texte: String,
}

#[derive(Deserialize)]
struct Body {
    astre: String,
    signe: String,
    degre: f64,
    #[serde(default)]
    retrograde: bool,
}

/// Commande de Barnum, découpée en programme et arguments.
pub fn command() -> Vec<String> {
    std::env::var("PRINTR_BARNUM")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| "barnum".to_owned())
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Barnum est-il installé ? (programme trouvé dans le PATH, ou chemin existant)
pub fn available() -> bool {
    let Some(program) = command().into_iter().next() else { return false };
    if program.contains('/') {
        return std::path::Path::new(&program).exists();
    }
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(&program).is_file()))
}

/// Lance Barnum et lit sa réponse JSON.
fn run(command: &[String], who: &Who, date: NaiveDate, variant: Option<&str>, english: bool) -> Result<Reading> {
    let (program, base) = command.split_first().context(i18n::t("commande Barnum vide", "empty Barnum command"))?;
    let mut cmd = Command::new(program);
    cmd.args(base);
    // Options d'abord, sous la forme `--option=valeur`, puis `--` : un signe ou une variante
    // venus de l'appli ne peuvent jamais être pris pour une option de Barnum.
    cmd.arg(format!("--date={date}")).arg("--json");
    if english {
        cmd.arg("--lang=en");
    }
    if let Some(variant) = variant {
        cmd.arg(format!("--sel={variant}"));
    }
    match who {
        Who::Sign(sign) => cmd.arg("--").arg(sign),
        Who::Birth(birth) => cmd.arg(format!("--naissance={birth}")),
    };
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow::anyhow!(
            "{}",
            tr!(
                "Barnum introuvable (« {program} ») : installe-le ou définis PRINTR_BARNUM",
                "Barnum not found (\"{program}\"): install it or set PRINTR_BARNUM"
            )
        ),
        _ => anyhow::anyhow!("{}", tr!("impossible de lancer Barnum : {e}", "could not start Barnum: {e}")),
    })?;

    // Barnum répond en quelques dizaines de millisecondes ; au-delà de TIMEOUT, on abandonne.
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            bail!("{}", i18n::t("Barnum ne répond pas", "Barnum is not responding"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child.stdout.take().map(|mut s| s.read_to_string(&mut stdout));
    child.stderr.take().map(|mut s| s.read_to_string(&mut stderr));
    if !status.success() {
        // « barnum: error: signe inconnu : … » : on garde le message utile.
        let message = stderr.lines().last().unwrap_or(i18n::t("erreur inconnue", "unknown error"));
        if english && message.contains("--lang") {
            return Err(OldBarnum.into());
        }
        bail!("{}", message.trim_start_matches("barnum: error: "));
    }
    serde_json::from_str(&stdout).context(i18n::t("réponse de Barnum illisible", "Barnum response unreadable"))
}

/// Barnum antérieur à 0.3 : il ne connaît pas `--lang`.
#[derive(Debug)]
struct OldBarnum;

impl std::fmt::Display for OldBarnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Barnum < 0.3 : pas de --lang")
    }
}

impl std::error::Error for OldBarnum {}

pub enum Who {
    Sign(String),
    Birth(NaiveDate),
}

/// Jauge de note sur 5 : « ███░░ ».
fn gauge(note: u8) -> String {
    let note = note.min(5) as usize;
    format!("{}{}", "█".repeat(note), "░".repeat(5 - note))
}

/// Les libellés suivent la langue de la lecture, pas celle de l'installation : un ticket écrit par
/// un Barnum qui ne parle que français reste entièrement en français.
fn render(r: &Reading, sky: bool) -> Doc {
    let lang = if r.langue.as_deref() == Some("en") { i18n::Lang::En } else { i18n::Lang::Fr };
    i18n::with_lang(lang, || render_in_language(r, sky))
}

fn render_in_language(r: &Reading, sky: bool) -> Doc {
    let mut doc = Doc::new();
    doc.header(&format!("Horoscope · {}", r.signe));
    if let Some(moon) = &r.lune {
        doc.text(&tr!("Lune : {} en {}", "Moon: {} in {}", moon.phase, moon.signe), Style::default().small().center());
    }
    doc.feed(1);
    doc.text(&r.climat, Style::default());
    for d in &r.domaines {
        doc.feed(1);
        let name = d.libelle.as_deref().unwrap_or(&d.cle);
        doc.line(&format!("{:<14}{}", i18n::capitalize(name), gauge(d.note)), Style::default().bold());
        doc.text(&d.texte, Style::default());
    }
    if !r.conseil.is_empty() {
        doc.feed(1);
        doc.hanging(i18n::t("Conseil : ", "Advice: "), &r.conseil, Style::default().bold());
    }
    let mut extras = Vec::new();
    if !r.couleur.is_empty() {
        extras.push(tr!("couleur {}", "color {}", r.couleur));
    }
    if let Some(n) = r.chiffre {
        extras.push(tr!("chiffre {n}", "number {n}"));
    }
    if !r.humeur.is_empty() {
        extras.push(tr!("humeur {}", "mood {}", r.humeur));
    }
    if !extras.is_empty() {
        doc.text(&i18n::capitalize(&extras.join(" · ")), Style::default().small().center());
    }
    if sky && !r.ciel.is_empty() {
        doc.feed(1);
        doc.text(i18n::t("Le ciel du jour", "Today's sky"), Style::default().small().bold().center());
        for b in &r.ciel {
            // Même largeur pour toutes les lignes : les colonnes restent alignées une fois centrées.
            let retro = if b.retrograde { i18n::t("rétrograde", "retrograde") } else { "" };
            let line = format!("{:<9}{:<12}{:>3.0}°  {retro:<10}", b.astre, b.signe, b.degre);
            doc.line(&line, Style::default().small().center());
        }
    }
    doc
}

pub fn build(who: Who, date: NaiveDate, sky: bool, variant: Option<&str>) -> Result<Doc> {
    let command = command();
    let reading = match run(&command, &who, date, variant, i18n::en()) {
        Err(e) if e.is::<OldBarnum>() => run(&command, &who, date, variant, false)?,
        other => other?,
    };
    Ok(render(&reading, sky))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "signe": "Lion", "date": "2026-10-08",
        "lune": {"phase": "dernier croissant", "signe": "Vierge"},
        "transit": {"astre": "Mars", "aspect": "conjonction"},
        "climat": "Mars traverse votre signe et vous pousse à brûler les étapes.",
        "domaines": [
            {"cle": "amour", "note": 1, "texte": "Vénus vous complique la tâche.", "influence": "Vénus"},
            {"cle": "argent", "note": 4, "texte": "Bonne journée pour faire vos comptes.", "influence": null}
        ],
        "conseil": "La simplicité est une force.", "couleur": "violet", "chiffre": 20, "humeur": "joueuse",
        "ciel": [{"astre": "Vénus", "signe": "Scorpion", "degre": 7.95, "retrograde": true}]
    }"#;

    fn lines(doc: &Doc) -> Vec<String> {
        doc.ops
            .iter()
            .filter_map(|op| match op {
                crate::doc::Op::Line { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn renders_reading_with_gauges_and_sky() {
        let reading: Reading = serde_json::from_str(SAMPLE).unwrap();
        let all = lines(&render(&reading, true)).join("\n");
        assert!(all.contains("Amour         █░░░░"));
        assert!(all.contains("Argent        ████░"));
        assert!(all.contains("Couleur violet · chiffre 20 · humeur joueuse"));
        assert!(all.contains("rétrograde"));
        assert!(!lines(&render(&reading, false)).join("\n").contains("rétrograde"));
    }

    #[test]
    fn missing_program_is_explained() {
        let cmd = vec!["printr-barnum-introuvable".to_owned()];
        let date = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        let err = run(&cmd, &Who::Sign("lion".into()), date, None, false).err().unwrap();
        assert!(err.to_string().contains("Barnum introuvable"));
    }

    #[test]
    fn english_reading_gets_english_labels() {
        let json = SAMPLE
            .replacen('{', r#"{"langue": "en","#, 1)
            .replace(r#""cle": "amour","#, r#""cle": "amour", "libelle": "Love","#);
        let reading: Reading = serde_json::from_str(&json).unwrap();
        let all = lines(&render(&reading, true)).join("\n");
        assert!(all.contains("Love          █░░░░"));
        assert!(all.contains("Color violet · number 20 · mood joueuse"));
        assert!(all.contains("retrograde"));
    }

    /// Un Barnum d'avant 0.3 refuse `--lang` : en anglais, le ticket sort quand même, en français.
    #[test]
    fn old_barnum_falls_back_to_french() {
        let dir = std::env::temp_dir().join(format!("printr-barnum-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("old-barnum.sh");
        std::fs::write(&script, format!(
            "#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = --lang=en ] && {{ echo 'main.py: error: unrecognized arguments: --lang=en' >&2; exit 2; }}; done\ncat <<'JSON'\n{SAMPLE}\nJSON\n"
        )).unwrap();
        // Lancé par sh, pas exécuté directement : un autre test qui lance un processus au même moment
        // hériterait du fichier encore ouvert en écriture, et l'exécution échouerait (ETXTBSY).
        let cmd = vec!["sh".to_owned(), script.to_string_lossy().into_owned()];
        let date = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        let err = run(&cmd, &Who::Sign("lion".into()), date, None, true).err().unwrap();
        assert!(err.is::<OldBarnum>());
        let reading = run(&cmd, &Who::Sign("lion".into()), date, None, false).unwrap();
        let all = i18n::with_lang(i18n::Lang::En, || lines(&render(&reading, false)).join("\n"));
        assert!(all.contains("Couleur violet"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
