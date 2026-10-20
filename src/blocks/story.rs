//! Histoire du soir : Claude invente un conte où les enfants sont les héros, sur un thème au choix.
//! Une nouvelle histoire par jour (en cache pour la journée), jamais en aperçu ; les titres déjà
//! racontés sont gardés pour ne pas se répéter.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::Ctx;
use crate::claude::Claude;
use crate::doc::{Align, Doc, Style};
use crate::i18n;

/// Longueur de l'histoire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Length {
    #[default]
    #[serde(alias = "courte")]
    Short,
    #[serde(alias = "longue")]
    Long,
}

impl Length {
    fn words(self) -> &'static str {
        match self {
            Length::Short => "250",
            Length::Long => "500",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Story {
    title: String,
    paragraphs: Vec<String>,
}

/// Titres déjà racontés, pour varier.
const HISTORY: usize = 30;

fn names(children: &str) -> Vec<String> {
    children
        .split([',', '\n', ';'])
        .flat_map(|part| part.split(" et ").flat_map(|p| p.split(" and ")))
        .map(|n| n.trim().chars().take(30).collect::<String>())
        .filter(|n| !n.is_empty())
        .take(6)
        .collect()
}

fn ask(claude: &Claude, children: &[String], theme: &str, length: Length, past: &[String]) -> Result<Story> {
    let kids = children.join(", ");
    let words = length.words();
    let past = if past.is_empty() { String::new() } else { past.join(" ; ") };
    let (system, prompt) = if i18n::en() {
        (
            "You write bedtime stories for young children, printed on a small thermal printer and read \
             aloud by a parent. Plain text only: no emoji, no markdown. Gentle, warm and funny, with a little \
             adventure and a calm, reassuring ending that leads to sleep. No violence, nothing scary, no \
             moral lecture. Simple words, short sentences, a bit of dialogue.",
            format!(
                "A bedtime story of about {words} words where the heroes are: {kids}.{}{}\n- title: a short, \
                 inviting title.\n- paragraphs: 4 to 8 short paragraphs.",
                if theme.is_empty() { String::new() } else { format!(" Theme: {theme}.") },
                if past.is_empty() {
                    String::new()
                } else {
                    format!(" Not the same as these earlier stories: {past}.")
                },
            ),
        )
    } else {
        (
            "Tu écris des histoires du soir pour de jeunes enfants, imprimées sur une petite imprimante \
             thermique et lues à voix haute par un parent. Texte brut uniquement : ni emoji, ni markdown ; \
             les accents français s'impriment, utilise-les. Douce, chaleureuse et drôle, avec une petite \
             aventure et une fin calme et rassurante qui mène au sommeil. Ni violence, ni peur, ni morale \
             appuyée. Des mots simples, des phrases courtes, un peu de dialogue.",
            format!(
                "Une histoire du soir d'environ {words} mots dont les héros sont : {kids}.{}{}\n- title : un \
                 titre court et engageant.\n- paragraphs : 4 à 8 paragraphes courts.",
                if theme.is_empty() { String::new() } else { format!(" Thème : {theme}.") },
                if past.is_empty() {
                    String::new()
                } else {
                    format!(" Pas la même que ces histoires déjà racontées : {past}.")
                },
            ),
        )
    };
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "paragraphs": { "type": "array", "items": { "type": "string" } },
        },
        "required": ["title", "paragraphs"],
        "additionalProperties": false,
    });
    let mut story: Story = claude.generate(system, &prompt, schema.clone(), "low")?;
    if story.paragraphs.is_empty() {
        story = claude.generate(system, &prompt, schema, "low")?;
    }
    anyhow::ensure!(
        !story.paragraphs.is_empty(),
        "{}",
        i18n::t("réponse de Claude incomplète", "incomplete answer from Claude")
    );
    Ok(story)
}

pub fn build(ctx: &Ctx, children: &str, theme: &str, length: Length) -> Result<Doc> {
    let children = names(children);
    if children.is_empty() {
        bail!("{}", i18n::t("indiquer les prénoms des enfants (`children`)", "give the children's names (`children`)"));
    }
    let theme: String = theme.trim().chars().take(80).collect();
    let key = format!(
        "histoire-{}-{}-{}-{:?}-{}",
        ctx.today,
        children.join("+").to_lowercase(),
        theme.to_lowercase(),
        length,
        i18n::lang().code()
    );
    let history = format!("histoires-{}", i18n::lang().code());
    let cache = ctx.cache.as_ref();
    let cached: Option<Story> = cache.filter(|_| !ctx.refresh).and_then(|c| c.get(&key));
    let from_cache = cached.is_some();
    let story = match cached {
        Some(s) => s,
        None if ctx.preview => {
            return Ok(super::ai_placeholder(
                i18n::t("Histoire du soir", "Bedtime story"),
                &tr!("Une histoire avec {}", "A story with {}", children.join(", ")),
            ));
        }
        None => {
            let claude = ctx.claude.as_ref().context(crate::claude::missing())?;
            let past = cache.map(|c| c.history(&history, HISTORY)).unwrap_or_default();
            let story = ask(claude, &children, &theme, length, &past)?;
            if let Some(cache) = cache {
                cache.put(&key, &story);
                cache.remember(&history, &story.title, HISTORY);
            }
            story
        }
    };
    let mut doc = Doc::new();
    doc.from_cache = from_cache;
    doc.header(i18n::t("Histoire du soir", "Bedtime story"));
    doc.feed(1);
    doc.text(&story.title, Style::default().bold().center());
    for paragraph in &story.paragraphs {
        doc.feed(1);
        doc.text(paragraph.trim(), Style::default());
    }
    doc.feed(1);
    doc.text(i18n::t("Fin. Bonne nuit !", "The end. Good night!"), Style::default().bold().center());
    doc.text(
        i18n::t("Histoire inventée par Claude", "Story made up by Claude"),
        Style::default().small().align(Align::Right),
    );
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_split_every_way() {
        assert_eq!(names("Léo, Camille"), ["Léo", "Camille"]);
        assert_eq!(names("Léo et Camille\nNina"), ["Léo", "Camille", "Nina"]);
        assert_eq!(names("Leo and Nina"), ["Leo", "Nina"]);
        assert!(names(" , ").is_empty());
    }

    #[test]
    fn preview_never_calls_claude() {
        let ctx = Ctx::new(false).for_preview();
        let doc = build(&ctx, "Léo", "", Length::Short).unwrap();
        assert!(!doc.ops.is_empty());
        assert!(build(&ctx, "  ", "", Length::Short).is_err());
    }
}
