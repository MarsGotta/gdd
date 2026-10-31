"""Build the documentation website from docs/, in English and Spanish.

    python3 scripts/build_site.py                 # build into build/site (Spanish in build/site/es)
    python3 scripts/build_site.py --serve         # preview the English site with live reload
    python3 scripts/build_site.py --base /docs/   # build for a site served under /docs/

docs/ is written for GitHub first, so each language is built from a staged copy:

- docs/ (English, without docs/es/) goes to build/site-src, docs/es/ to build/site-src-es;
- each README.md becomes the home page of its language, with a hero and four cards on top;
- links that leave a language's folder (CONTRIBUTING.md, GDD.md, the project's records) point
  to the file on GitHub, and links to a folder's README.md point to its index page;
- the project's own records (Brief, standards, ADRs, Discover notes) stay on GitHub;
- website/assets (logo, colors, the flow on the home page) are added to both, and the CNAME
  file of the custom domain to the English one, at the root of the site.

Configuration: zensical.toml, for English; the Spanish one is derived from it here (language,
folders, menu labels). Published to https://gdd.marsgotta.com by .github/workflows/docs.yml on
every push to main.
"""

from __future__ import annotations

import argparse
import posixpath
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
BUILD = ROOT / "build"
ASSETS = ROOT / "website" / "assets"
CONFIG = ROOT / "zensical.toml"
REPO = "https://github.com/MarsGotta/gdd"
DOMAIN = "gdd.marsgotta.com"
"""The site's custom domain, written to CNAME for GitHub Pages."""

SKIP = ("brief.md", "standards.md", "adr/", "discover/", "es/")
"""Left out of the English site: the project's own records (its Brief, standards, ADRs and
Discover notes) stay on GitHub, since the site is for people who use gdd; links to them point
to the file on GitHub. docs/es/ is the Spanish site."""

LANGS = {
    "en": {"source": DOCS, "stage": BUILD / "site-src", "site": BUILD / "site", "prefix": "docs"},
    "es": {"source": DOCS / "es", "stage": BUILD / "site-src-es", "site": BUILD / "site" / "es",
           "prefix": "docs/es"},
}

SPANISH = [
    ('site_description = "Gate-Driven Development: build software with AI coding agents, one gate at a time."',
     'site_description = "Gate-Driven Development: construye software con agentes de IA, puerta a puerta."'),
    ('docs_dir = "build/site-src"', 'docs_dir = "build/site-src-es"'),
    ('site_dir = "build/site"', 'site_dir = "build/site/es"'),
    ('language = "en"', 'language = "es"'),
    ('{ "Home" = "index.md" }', '{ "Inicio" = "index.md" }'),
    ('{ "Get started" = [', '{ "Empezar" = ['),
    ('{ "Guides" = [', '{ "Guías" = ['),
    ('{ "Reference" = [', '{ "Referencia" = ['),
    ('{ "Concepts" = [', '{ "Conceptos" = ['),
    ('{ "Contribute" = [', '{ "Contribuir" = ['),
    ('{ "Contributing" = ', '{ "Cómo contribuir (en inglés)" = '),
    ('{ "Writing the docs" = "STYLE.md" }',
     '{ "Escribir la documentación (en inglés)" = "https://github.com/MarsGotta/gdd/blob/main/docs/STYLE.md" }'),
    ('{ "The method\'s rules (GDD.md)" = ', '{ "Las reglas del método (GDD.md, en inglés)" = '),
    ('{ "Roadmap" = ', '{ "Hoja de ruta (en inglés)" = '),
    ('{ "Changelog" = ', '{ "Changelog (en inglés)" = '),
    ('toggle.name = "Switch to day"', 'toggle.name = "Cambiar a modo día"'),
    ('toggle.name = "Switch to night"', 'toggle.name = "Cambiar a modo noche"'),
]
"""How the Spanish configuration differs from zensical.toml: each pair must match once."""

LINK = re.compile(r"(\]\()([^)\s]+)((?:\s+\"[^\"]*\")?\))|(src=\")([^\"]+)(\")")
FENCE = re.compile(r"^\s*(```|~~~)")

HERO_EN = """\
<div class="gd-hero" markdown>
<div markdown>

# gdd

<p class="gd-tagline">Build software with AI coding agents, one gate at a time. You talk to your
agent; risk decides how many gates a change meets; machines check what can be checked, and you
decide only what needs a person, in as few sittings as possible.</p>

[Quickstart](getting-started/quickstart.md){ .md-button .md-button--primary }
[GDD in one page](explanation/method.md){ .md-button }

</div>

![A change moves from G0 to G4: the harness, the scenarios you approve, the machines' gates,
the Reviewer and your Release Gate. Green, yellow or red decides how strict each gate
is.](assets/flow.svg)

</div>

<div class="grid cards" markdown>

-   :material-rocket-launch-outline: **Quickstart**

    ---

    Install gdd, answer its setup form, then take your first change to a release by talking to your agent.

    [Start here →](getting-started/quickstart.md)

-   :material-school-outline: **Tutorial**

    ---

    One real change, from the idea to the release, with every screen on the way.

    [Follow the tutorial →](getting-started/tutorial.md)

-   :material-robot-outline: **Skills**

    ---

    What you run in your agent: `/gdd-setup`, `/gdd-change` and the rest, and what each one does.

    [Skills reference →](reference/skills/index.md)

-   :material-console: **CLI**

    ---

    Every `gdd` command, its options and what it prints, for the terminal and for CI.

    [CLI reference →](reference/cli/index.md)

</div>
"""


HERO_ES = """\
<div class="gd-hero" markdown>
<div markdown>

# gdd

<p class="gd-tagline">Construye software con agentes de IA, puerta a puerta. Hablas con tu
agente; el riesgo decide cuántas puertas pasa cada Change; las máquinas comprueban lo que se
puede comprobar, y tú solo decides lo que necesita a una persona, en las menos sentadas
posibles.</p>

[Inicio rápido](getting-started/quickstart.md){ .md-button .md-button--primary }
[GDD en una página](explanation/method.md){ .md-button }

</div>

![Un Change pasa de G0 a G4: el arnés, los escenarios que apruebas, las puertas de las
máquinas, el Reviewer y tu puerta de salida. Verde, amarillo o rojo deciden lo estricta que es
cada puerta.](assets/flow-es.svg)

</div>

<div class="grid cards" markdown>

-   :material-rocket-launch-outline: **Inicio rápido**

    ---

    Instala gdd, responde a su formulario de preparación y lleva tu primer Change a producción hablando con tu agente.

    [Empieza aquí →](getting-started/quickstart.md)

-   :material-school-outline: **Tutorial**

    ---

    Un Change real, de la idea a la salida a producción, con cada pantalla del camino.

    [Sigue el tutorial →](getting-started/tutorial.md)

-   :material-robot-outline: **Skills**

    ---

    Lo que ejecutas en tu agente: `/gdd-setup`, `/gdd-change` y las demás, y qué hace cada una.

    [Referencia de skills →](reference/skills/index.md)

-   :material-console: **CLI**

    ---

    Cada comando de `gdd`, sus opciones y lo que imprime, para la terminal y para la CI.

    [Referencia de la CLI →](reference/cli/index.md)

</div>
"""

HOME_TITLE = {"en": "Gate-Driven Development", "es": "Gate-Driven Development"}


def skipped(path: str) -> bool:
    return any(path == s or (s.endswith("/") and path.startswith(s)) for s in SKIP)


def rewrite(target: str, source: Path, lang: str) -> str:
    """The link `target`, written in `source` (relative to the language's folder), as the site
    needs it."""
    if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith(("#", "/")):
        return target
    path, _, anchor = target.partition("#")
    if not path:
        return target
    here = posixpath.dirname(source.as_posix())
    resolved = posixpath.normpath(posixpath.join(here, path))
    leaves = resolved.startswith("../") or resolved == ".."
    if leaves or (lang == "en" and skipped(resolved)):
        repo_path = posixpath.normpath(posixpath.join(LANGS[lang]["prefix"], here, path))
        kind = "tree" if (ROOT / repo_path).is_dir() else "blob"
        url = f"{REPO}/{kind}/main/{repo_path}"
        return f"{url}#{anchor}" if anchor else url
    if posixpath.basename(path) == "README.md":
        path = posixpath.join(posixpath.dirname(path), "index.md")
    return f"{path}#{anchor}" if anchor else path


def stage_page(text: str, source: Path, lang: str) -> str:
    out, fenced = [], False
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
        if not fenced:

            def fix(m: re.Match[str]) -> str:
                if m.group(1):
                    return m.group(1) + rewrite(m.group(2), source, lang) + m.group(3)
                return m.group(4) + rewrite(m.group(5), source, lang) + m.group(6)

            line = LINK.sub(fix, line)
        out.append(line)
    return "\n".join(out) + "\n"


def home_page(text: str, lang: str) -> str:
    """A README.md with the hero in place of its title and opening paragraphs."""
    body = text.split("\n## ", 1)[1]
    front = f"---\ntitle: {HOME_TITLE[lang]}\nhide:\n  - navigation\n  - toc\n---\n"
    hero = HERO_EN if lang == "en" else HERO_ES
    return front + "\n" + hero + "\n## " + body


def stage(lang: str) -> None:
    source, dest_root = LANGS[lang]["source"], LANGS[lang]["stage"]
    if dest_root.exists():
        shutil.rmtree(dest_root)
    for src in sorted(source.rglob("*")):
        rel = src.relative_to(source)
        if src.is_dir() or (lang == "en" and skipped(rel.as_posix())):
            continue
        dest = dest_root / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        if src.suffix != ".md":
            shutil.copy2(src, dest)
            continue
        text = stage_page(src.read_text(encoding="utf-8"), rel, lang)
        if rel.name == "README.md":
            dest = dest.with_name("index.md")
            if rel.parent == Path("."):
                text = home_page(text, lang)
        dest.write_text(text, encoding="utf-8")
    shutil.copytree(ASSETS, dest_root / "assets")
    if DOMAIN and lang == "en":
        (dest_root / "CNAME").write_text(DOMAIN + "\n", encoding="utf-8")


def config(lang: str, base: str) -> Path:
    """The configuration of `lang`, with the language switcher pointing at `base`."""
    text = CONFIG.read_text(encoding="utf-8")
    if lang == "es":
        for old, new in SPANISH:
            if text.count(old) != 1:
                sys.exit(f"zensical.toml changed: update SPANISH in {Path(__file__).name} ({old})")
            text = text.replace(old, new)
    # The configuration is written in build/, and Zensical reads its folders from there.
    text = text.replace('docs_dir = "build/', 'docs_dir = "')
    text = text.replace('site_dir = "build/', 'site_dir = "')
    text += (
        "\n[[project.extra.alternate]]\n"
        f'name = "English"\nlink = "{base}"\nlang = "en"\n'
        "\n[[project.extra.alternate]]\n"
        f'name = "Español"\nlink = "{base}es/"\nlang = "es"\n'
    )
    path = BUILD / f"zensical-{lang}.toml"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def nav_pages(nav: list) -> list[str]:
    """The pages the menu `nav` names, without its links to other sites."""
    pages = []
    for item in nav:
        if isinstance(item, dict):
            for value in item.values():
                pages += nav_pages(value if isinstance(value, list) else [value])
        elif not re.match(r"^[a-z][a-z0-9+.-]*:", item):
            pages.append(item)
    return pages


def missing_pages(lang: str, path: Path) -> list[str]:
    """The pages the menu of the configuration `path` names that the staged copy of `lang` does
    not have. Zensical takes such a path for a link to another site and builds without a word,
    so the menu would carry a broken entry."""
    with path.open("rb") as file:
        nav = tomllib.load(file)["project"]["nav"]
    stage = LANGS[lang]["stage"]
    return [page for page in nav_pages(nav) if not (stage / page).is_file()]


def checked(lang: str, path: Path) -> bool:
    """Whether the menu of `lang` has a page behind every entry. A missing English page stops the
    build; a missing Spanish one is only a warning, since the translation follows the English."""
    missing = missing_pages(lang, path)
    for page in missing:
        kind = "error" if lang == "en" else "warning"
        print(f"{kind}: the {lang} menu names {page}, which has no page", file=sys.stderr)
    return lang != "en" or not missing


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build the documentation website from docs/.")
    parser.add_argument("--serve", action="store_true", help="preview the English site with live reload")
    parser.add_argument("--stage-only", action="store_true", help="only write the staged copies")
    parser.add_argument("--base", default="/", help="the path the site is served under (default /)")
    args = parser.parse_args(argv)
    base = args.base if args.base.endswith("/") else args.base + "/"
    for lang in LANGS:
        stage(lang)
    if args.stage_only:
        print("staged build/site-src and build/site-src-es")
        return 0
    zensical = ["zensical"] if shutil.which("zensical") else ["uvx", "zensical@0.0.71"]
    if args.serve:
        path = config("en", base)
        checked("en", path)  # the preview goes on: the pages may still be on their way
        return subprocess.run([*zensical, "serve", "-f", str(path)], cwd=ROOT).returncode
    for lang in LANGS:
        path = config(lang, base)
        if not checked(lang, path):
            return 1
        command = [*zensical, "build", "--clean", "--strict", "-f", str(path)]
        code = subprocess.run(command, cwd=ROOT).returncode
        if code:
            return code
    return 0


if __name__ == "__main__":
    sys.exit(main())
