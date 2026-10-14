//! Liste de courses partagée : chacun ajoute des articles depuis l'appli au fil de la semaine,
//! le bloc les imprime en cases à cocher. Avec `clear`, le serveur vide la liste une fois imprimée.

use anyhow::Result;

use crate::doc::{Doc, Style};
use crate::store::{self, ShoppingItem};

fn render(title: &str, items: &[ShoppingItem]) -> Doc {
    let mut doc = Doc::new();
    doc.header(title);
    if items.is_empty() {
        doc.text(
            crate::i18n::t("La liste est vide : rien à acheter !", "The list is empty: nothing to buy!"),
            Style::default().small().center(),
        );
        return doc;
    }
    for item in items {
        doc.hanging("[ ] ", &item.text, Style::default());
    }
    // Qui a ajouté quoi, en une ligne discrète.
    let mut people: Vec<&str> = Vec::new();
    for item in items {
        if !people.contains(&item.by.as_str()) {
            people.push(&item.by);
        }
    }
    let count = items.len();
    let who = people.join(", ");
    let line = if count > 1 {
        tr!("{count} articles, ajoutés par {who}", "{count} items, added by {who}")
    } else {
        tr!("{count} article, ajouté par {who}", "{count} item, added by {who}")
    };
    doc.text(&line, Style::default().small().center());
    doc
}

pub fn build(title: Option<&str>) -> Result<Doc> {
    let items = store::read_data()?.shopping;
    Ok(render(title.unwrap_or(crate::i18n::t("Liste de courses", "Shopping list")), &items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Op;

    #[test]
    fn lists_items_and_authors() {
        let item = |text: &str, by: &str| ShoppingItem {
            id: text.into(),
            text: text.into(),
            by: by.into(),
            added: chrono::Utc::now(),
        };
        let doc = render("Courses", &[item("Lait", "Camille"), item("Pain", "Léo"), item("Œufs", "Camille")]);
        let lines: Vec<&str> = doc
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Line { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(lines.contains(&"[ ] Lait"));
        assert!(lines.iter().any(|l| l.contains("3 articles, ajoutés par Camille, Léo")));
    }
}
