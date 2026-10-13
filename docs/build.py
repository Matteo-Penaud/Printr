"""Construit le site dans _site/ : la page d'accueil (docs/) et les pages de documentation,
rendues depuis le README et le guide de contribution du dépôt pour ne jamais s'en écarter.

    uvx --with markdown --with pymdown-extensions python docs/build.py && python3 -m http.server -d _site
"""

import html
import json
import os
import re
import shutil
import struct
from pathlib import Path

import markdown
from pymdownx.slugs import slugify

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "docs"
OUT = ROOT / "_site"
REPO = "https://github.com/XNinety9/Printr"
BRANCH = "master"

# (page produite, titre, source, libellé dans la navigation)
PAGES = [
    ("guide.html", "Guide", ROOT / "README.md", "Guide"),
    ("contribuer.html", "Contribuer", ROOT / "CONTRIBUTING.md", "Contribuer"),
]
# Page des blocs, générée depuis le catalogue et les exemples de docs/exemples/.
BLOCKS_PAGE = "blocs.html"
CATALOGUE = SITE / "exemples" / "catalogue.json"
# Navigation des pages de documentation : la page des blocs, puis les pages Markdown.
NAV = [(BLOCKS_PAGE, "Blocs")] + [(page, label) for page, _, _, label in PAGES]
# Fichiers du dépôt qui ont leur page sur le site.
PAGE_FOR = {"README.md": "guide.html", "CONTRIBUTING.md": "contribuer.html"}
# Ce qui, dans docs/, ne sert qu'à fabriquer le site ou ses illustrations.
SKIP = shutil.ignore_patterns("build.py", "demo", "*.sh", "*.py", "catalogue.json", "__pycache__")

TEMPLATE = """<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Printr</title>
<meta name="description" content="Documentation de Printr : {title_lower}.">
<link rel="icon" href="mark.png" type="image/png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
<script>
  // Thème choisi sur la page d'accueil, avant le premier affichage.
  try {{ var t = localStorage.getItem("printr-theme"); if (t === "dark" || t === "light") document.documentElement.setAttribute("data-theme", t); }} catch (e) {{}}
</script>
<link rel="stylesheet" href="docs.css">
</head>
<body>
<nav class="nav">
  <div class="wrap">
    <a class="brand" href="./" aria-label="Printr"><img src="mark.png" alt=""><span class="brand-name">Printr</span></a>
    <div class="links">{nav}<a href="{repo}">GitHub</a></div>
  </div>
</nav>
<main class="wrap docs">
  <aside class="toc"><div class="title">Sur cette page</div>{toc}</aside>
  <article class="doc">
    <h1>{title}</h1>
    {content}
    <p class="caption">Page générée depuis <a href="{repo}/blob/{branch}/{source}">{source}</a>.</p>
  </article>
</main>
<script src="toc.js" defer></script>
</body>
</html>
"""


def readme_body(text: str) -> str:
    """Le README sans son logo, sa devise, ses badges et ses liens d'en-tête : le site les a."""
    return re.sub(r"\A(?:\s*<p align=\"center\">.*?</p>)+\s*", "", text, flags=re.S)


def rewrite_links(body: str, source: Path) -> str:
    """Liens vers les autres pages Markdown : leur page du site ; images de docs/ : le site ;
    autres fichiers du dépôt : GitHub."""
    base = source.parent.relative_to(ROOT)

    def target(url: str) -> str:
        if re.match(r"^(https?:|mailto:|#|data:)", url):
            return url
        path, _, anchor = url.partition("#")
        repo_path = os.path.normpath((base / path).as_posix()).replace("\\", "/")
        suffix = f"#{anchor}" if anchor else ""
        if repo_path in PAGE_FOR:
            return PAGE_FOR[repo_path] + suffix
        if repo_path.startswith("docs/") and (ROOT / repo_path).is_file():
            return repo_path.removeprefix("docs/") + suffix
        kind = "tree" if (ROOT / repo_path).is_dir() else "blob"
        return f"{REPO}/{kind}/{BRANCH}/{repo_path}{suffix}"

    def fix(match: re.Match) -> str:
        attr, url = match.group(1), html.unescape(match.group(2))
        if attr == "srcset":
            url = ", ".join(" ".join([target(p.split()[0]), *p.split()[1:]]) for p in url.split(","))
        else:
            url = target(url)
        return f'{attr}="{html.escape(url)}"'

    return re.sub(r'(href|src|srcset)="([^"]+)"', fix, body)


def nav_links(current: str) -> str:
    return "".join(f'<a href="{p}"{" aria-current=\"page\"" if p == current else ""}>{label}</a>' for p, label in NAV)


def render(page: str, title: str, source: Path) -> str:
    text = source.read_text()
    text = readme_body(text) if source.name == "README.md" else re.sub(r"^# .*\n", "", text, count=1)
    md = markdown.Markdown(
        # superfences : blocs de code dans les listes ; md_in_html : Markdown dans le HTML du README.
        extensions=["pymdownx.superfences", "tables", "toc", "sane_lists", "attr_list", "md_in_html"],
        extension_configs={
            # Ancres à la GitHub (accents gardés) : les liens internes du README restent valides.
            "toc": {"permalink": "#", "toc_depth": "2-3", "slugify": slugify(case="lower")},
        },
    )
    body = rewrite_links(md.convert(text), source)
    return TEMPLATE.format(
        title=title,
        title_lower=title.lower(),
        nav=nav_links(page),
        # Le sommaire de Python-Markdown a son propre <div class="toc"> : la page fournit le sien.
        toc=re.sub(r'\A\s*<div class="toc">\s*|\s*</div>\s*\Z', "", md.toc),
        content=body,
        repo=REPO,
        branch=BRANCH,
        source=source.relative_to(ROOT).as_posix(),
    )


BLOCKS_TEMPLATE = """<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Les blocs · Printr</title>
<meta name="description" content="Les {count} blocs de Printr, chacun avec le ticket qu'il imprime et l'exemple JSON qui l'a produit.">
<link rel="icon" href="mark.png" type="image/png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
<script>
  // Langue et thème choisis sur la page d'accueil, avant le premier affichage.
  (function () {{
    var root = document.documentElement, lang = null, theme = null;
    try {{ lang = localStorage.getItem("printr-lang"); theme = localStorage.getItem("printr-theme"); }} catch (e) {{}}
    if (lang !== "en" && lang !== "fr") lang = /^fr\\b/i.test(navigator.language || "") ? "fr" : "en";
    root.lang = lang;
    if (theme === "dark" || theme === "light") root.setAttribute("data-theme", theme);
  }})();
  function toggleLang() {{
    var root = document.documentElement;
    root.lang = root.lang === "fr" ? "en" : "fr";
    try {{ localStorage.setItem("printr-lang", root.lang); }} catch (e) {{}}
  }}
</script>
<link rel="stylesheet" href="docs.css">
</head>
<body class="bilingual">
<nav class="nav">
  <div class="wrap">
    <a class="brand" href="./" aria-label="Printr"><img src="mark.png" alt=""><span class="brand-name">Printr</span></a>
    <div class="links">{nav}<a href="{repo}">GitHub</a>
      <button class="toggle" type="button" onclick="toggleLang()" aria-label="Français / English"><span class="fr">EN</span><span class="en">FR</span></button></div>
  </div>
</nav>
<main class="wrap docs">
  <aside class="toc"><div class="title"><span class="fr">Les blocs</span><span class="en">Blocks</span></div>{toc}</aside>
  <article class="doc">
    <h1><span class="fr">Les {count} blocs</span><span class="en">The {count} blocks</span></h1>
    <p class="fr">Chaque ticket ci-dessous sort de l'émulateur de la TM-T88V, au point près, à partir de l'exemple JSON indiqué (données fictives). Dans l'appli, on les ajoute et on les règle sans écrire de JSON. Les blocs marqués Claude font rédiger leur texte par Claude, à l'impression seulement.</p>
    <p class="en">Each ticket below comes out of the TM-T88V emulator, dot for dot, from the JSON example shown (made-up data). In the app, blocks are added and tuned without writing any JSON. Blocks marked Claude have their text written by Claude, at print time only.</p>
    {content}
    <p class="caption"><span class="fr">Page générée depuis</span><span class="en">Generated from</span> <a href="{repo}/tree/{branch}/docs/exemples">docs/exemples</a>.</p>
  </article>
</main>
<script src="toc.js" defer></script>
</body>
</html>
"""


def png_size(path: Path) -> tuple[int, int]:
    """Largeur et hauteur d'un PNG (en-tête IHDR), pour réserver sa place dans la page."""
    with path.open("rb") as f:
        head = f.read(24)
    return struct.unpack(">II", head[16:24])


def both(fr: str, en: str) -> str:
    return f'<span class="fr">{html.escape(fr)}</span><span class="en">{html.escape(en)}</span>'


def render_blocks() -> str:
    """Une fiche par bloc : son ticket imprimé, sa description et l'exemple JSON qui l'a produit."""
    catalogue = json.loads(CATALOGUE.read_text())
    slug = slugify(case="lower")
    toc, content, count = [], [], 0
    for group in catalogue:
        gid = slug(group["fr"], "-")
        items = []
        content.append(f'<h2 id="{gid}">{both(group["fr"], group["en"])}</h2>')
        for b in group["blocs"]:
            count += 1
            bid = b["id"]
            png = SITE / "exemples" / f"{bid}.png"
            example = json.loads((SITE / "exemples" / f"{bid}.json").read_text())
            width, height = png_size(png)
            badge = '<span class="claude">Claude</span>' if b.get("claude") else ""
            code = html.escape(json.dumps(example, ensure_ascii=False, indent=2))
            items.append(f'<li><a href="#{bid}">{b["emoji"]} {both(b["fr"], b["en"])}</a></li>')
            content.append(f"""<section class="bloc">
  <div class="bloc-text">
    <h3 id="{bid}"><span class="emoji">{b["emoji"]}</span> {both(b["fr"], b["en"])}{badge}<a class="headerlink" href="#{bid}">#</a></h3>
    <p>{both(b["description"]["fr"], b["description"]["en"])}</p>
    <details><summary>{both("Le ticket en JSON", "Ticket JSON")}</summary><pre><code>{code}</code></pre></details>
  </div>
  <a class="bloc-ticket" href="exemples/{bid}.png"><img src="exemples/{bid}.png" alt="{html.escape(b["fr"])}" width="{width}" height="{height}" loading="lazy"></a>
</section>""")
        toc.append(f'<li><a href="#{gid}">{both(group["fr"], group["en"])}</a><ul>{"".join(items)}</ul></li>')
    return BLOCKS_TEMPLATE.format(
        count=count, nav=nav_links(BLOCKS_PAGE), toc=f"<ul>{''.join(toc)}</ul>", content="\n".join(content),
        repo=REPO, branch=BRANCH,
    )


def main() -> None:
    shutil.rmtree(OUT, ignore_errors=True)
    shutil.copytree(SITE, OUT, ignore=SKIP)
    for page, title, source, _ in PAGES:
        (OUT / page).write_text(render(page, title, source))
    (OUT / BLOCKS_PAGE).write_text(render_blocks())
    (OUT / ".nojekyll").touch()  # fichiers servis tels quels
    pages = sorted(p.name for p in OUT.glob("*.html"))
    print(f"site construit dans {OUT.relative_to(ROOT)} : " + ", ".join(pages))


if __name__ == "__main__":
    main()
