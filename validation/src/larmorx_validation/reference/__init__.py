# SPDX-License-Identifier: Apache-2.0
"""The programs' reference pages of the documentation site, written from this repository.

``docs/api/<page>.md`` is the one source of a program's reference page (docs/writing-guide.md
§6, "One source"): one level-2 section per program, marked by a reference block
(:mod:`larmorx_validation.reference.apipage` says how it is written). The exporter joins it
with what the code says (:mod:`.code`: the options the command line accepts, the Python
function's signature, docstring and result, LARMORX's own options, the ports in the replica
registry) and writes ``content/programs/<group>/<program>.md`` in the site's checkout, laid
out by its ``reference`` template. Nobody edits those pages by hand.

    python -m larmorx_validation reference all --site <site>        write every page
    python -m larmorx_validation reference fsl-bet --site <site>    one API page's programs
    python -m larmorx_validation reference all --check              only the checks
    python -m larmorx_validation reference all --site <site> --check
                                                                   also fail on a stale page
    python -m larmorx_validation reference programs --site <site>   only the Programs page's data

``all`` also writes the Programs page's data, ``data/programs.yml``
(:mod:`larmorx_validation.reference.programs`, from ``validation/programs.toml``).

The checks (:func:`.page.build`) fail on any drift between an API page and the code; the
tests run them for every API page, and run every example of every reference page in a new
folder on synthetic inputs (``validation/tests/test_reference.py``). A page is byte-for-byte
the same when nothing it is made from changed: its front matter names each source with the
start of its SHA-256, never a commit.
"""

from __future__ import annotations

import sys
from pathlib import Path

from larmorx_validation.reference import apipage, programs
from larmorx_validation.reference.page import Reference, build

REPO = Path(__file__).resolve().parents[4]


def api_pages(repo: Path = REPO) -> list[str]:
    """The API pages that hold at least one program's section."""
    out = []
    for p in sorted((repo / "docs/api").glob("*.md")):
        rel = p.relative_to(repo).as_posix()
        if "<!-- reference" in p.read_text(encoding="utf-8"):
            out.append(rel)
    return out


def references(page: str, repo: Path = REPO) -> list[Reference]:
    """Every reference page an API page gives (``fsl-bet`` or ``docs/api/fsl-bet.md``)."""
    rel = page if page.endswith(".md") else f"docs/api/{page}.md"
    api = apipage.read(repo, rel)
    return [build(repo, p) for p in api.programs]


def main(args) -> int:
    repo = REPO
    site = Path(args.site) if args.site else None
    if args.page == "programs":
        return programs.main(repo, site, args.check)
    pages = api_pages(repo) if args.page == "all" else [args.page]
    refs = [r for page in pages for r in references(page, repo)]
    problems = [x for r in refs for x in r.problems]
    for x in problems:
        print(f"PROBLEM: {x}", file=sys.stderr)
    stale = []
    if site is not None:
        for r in refs:
            out = site / r.path
            old = out.read_text(encoding="utf-8") if out.is_file() else None
            if old == r.markdown:
                continue
            if args.check:
                stale.append(r.path)
                print(f"STALE: {out} differs from its export", file=sys.stderr)
            elif not problems:
                out.parent.mkdir(parents=True, exist_ok=True)
                out.write_text(r.markdown, encoding="utf-8")
                print(f"wrote {out}")
    print(
        f"{len(refs)} reference pages from {len(pages)} API pages; {len(problems)} problems"
        + (f"; {len(stale)} stale" if args.check and site else "")
    )
    status = 1 if problems or stale else 0
    if args.page == "all":
        status |= programs.main(repo, site, args.check)
    return status


def add_arguments(p) -> None:
    p.add_argument(
        "page", help="an API page (fsl-bet, docs/api/fsl-bet.md), 'programs' or 'all'"
    )
    p.add_argument("--site", help="the documentation site's checkout: write its reference pages")
    p.add_argument(
        "--check",
        action="store_true",
        help="write nothing; fail on a problem, or on a page that differs from its export",
    )
