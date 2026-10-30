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
    ('{ "Learn" = [', '{ "Aprender" = ['),
    ('{ "Do" = [', '{ "Hacer" = ['),
    ('{ "Day to day" = [', '{ "Día a día" = ['),
    ('{ "Shaping the work" = [', '{ "Dar forma al trabajo" = ['),
    ('{ "Setting up" = [', '{ "Preparar el proyecto" = ['),
    ('{ "Shipping and measuring" = [', '{ "Publicar y medir" = ['),
    ('{ "Look up" = [', '{ "Consultar" = ['),
    ('{ "Understand" = [', '{ "Entender" = ['),
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

<p class="gd-tagline">Build software with AI coding agents, one gate at a time. Risk decides how
many gates a change meets; machines check what can be checked; you decide only what needs a
person, in as few sittings as possible.</p>

[Your first change](getting-started/tutorial.md){ .md-button .md-button--primary }
[GDD in one page](explanation/method.md){ .md-button }

</div>

![A change moves from G0 to G4: the harness, the scenarios you approve, the machines' gates,
the Reviewer and your Release Gate. Green, yellow or red decides how strict each gate
is.](assets/flow.svg)

</div>

<div class="grid cards" markdown>

-   :material-school-outline: **Learn**

    ---

    From an empty folder to a released change in about half an hour, then the method in one page.

    [Start the tutorial →](getting-started/tutorial.md)

-   :material-wrench-outline: **Do**

    ---

    Run changes, write scenarios, answer the Release Gate, adopt an existing project, fix what breaks.

    [Run a change →](how-to/run-a-change.md)

-   :material-book-open-variant: **Look up**

    ---

    Every command, configuration key, gate, role, stack and file. Exact and complete.

    [Command reference →](reference/cli.md)

-   :material-lightbulb-outline: **Understand**

    ---

    Why gates, how risk is judged, what keeps agents safe, why you stop so rarely.

    [The life of a change →](explanation/life-of-a-change.md)

</div>
"""


HERO_ES = """\
<div class="gd-hero" markdown>
<div markdown>

# gdd

<p class="gd-tagline">Construye software con agentes de IA, puerta a puerta. El riesgo decide
cuántas puertas pasa cada Change; las máquinas comprueban lo que se puede comprobar, y tú solo
decides lo que necesita a una persona, en las menos sentadas posibles.</p>

[Tu primer Change](getting-started/tutorial.md){ .md-button .md-button--primary }
[GDD en una página](explanation/method.md){ .md-button }

</div>

![Un Change pasa de G0 a G4: el arnés, los escenarios que apruebas, las puertas de las
máquinas, el Reviewer y tu puerta de salida. Verde, amarillo o rojo deciden lo estricta que es
cada puerta.](assets/flow-es.svg)

</div>

<div class="grid cards" markdown>

-   :material-school-outline: **Aprender**

    ---

    De una carpeta vacía a un Change publicado en una media hora, y después el método en una página.

    [Empieza el tutorial →](getting-started/tutorial.md)

-   :material-wrench-outline: **Hacer**

    ---

    Lanzar Changes, escribir escenarios, responder a la puerta de salida, adoptar un proyecto existente, arreglar lo que falla.

    [Lanzar un Change →](how-to/run-a-change.md)

-   :material-book-open-variant: **Consultar**

    ---

    Cada comando, clave de configuración, puerta, rol, stack y fichero. Exacto y completo.

    [Referencia de comandos →](reference/cli.md)

-   :material-lightbulb-outline: **Entender**

    ---

    Por qué puertas, cómo se mide el riesgo, qué protege a los agentes, por qué paras tan poco.

    [La vida de un Change →](explanation/life-of-a-change.md)

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
        return subprocess.run([*zensical, "serve", "-f", str(config("en", base))], cwd=ROOT).returncode
    for lang in LANGS:
        command = [*zensical, "build", "--clean", "--strict", "-f", str(config(lang, base))]
        code = subprocess.run(command, cwd=ROOT).returncode
        if code:
            return code
    return 0


if __name__ == "__main__":
    sys.exit(main())
