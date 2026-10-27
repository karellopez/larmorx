# SPDX-License-Identifier: Apache-2.0
"""The published site's size, per page and in all, and the guard every export runs.

A page is one evidence file (``static/evidence/<tool>.json``) with the blobs it names; a blob
two pages share counts in each page and once in the site. The site is every file of ``static/``
it publishes (the blobs no JSON names are not published) plus the built HTML (``_site/``, when
it has been built). GitHub Pages publishes at most 1 GB; the guard keeps the site under
:data:`SITE_BUDGET` and each page under :data:`PAGE_BUDGET`, and names the largest items when
either is exceeded.

``python -m larmorx_validation evidence sizes --site <site>`` prints the table.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

#: The whole published site (GitHub Pages allows 1 GB; half of it leaves room).
SITE_BUDGET = 500_000_000
#: One page: its JSON and the images of its examples (about 25 tool pages fit the site budget
#: at 20 MB on average; a page holds two or three examples as full volumes, the rest as slices).
PAGE_BUDGET = 40_000_000
#: Pages whose budget is raised, with the reason. The guard never removes content (the user,
#: 2026-10-24: the site never shows less than it did): a page over the default budget gets a
#: raised one here, its reason printed with the sizes.
PAGE_BUDGETS: dict[str, tuple[int, str]] = {
    "ants-n4": (
        75_000_000,
        "the ANTS-40 example, fMRIPrep's final N4 of ds005454's 0.8 mm 7T T1w template, is a "
        "whole volume (172 x 214 x 172 voxels shown, about 40 MB) beside the ds000005 example "
        "(10 MB): an existing 3D view, kept at full resolution; and the data panel's six "
        "examples (2026-10-26: fMRIPrep's N4 of a multiband-8 BOLD reference as a whole "
        "volume, 4.8 MB, and five images as three full-resolution slices each, about 13 MB)",
    ),
}


def budget(page: str) -> int:
    """A page's budget: :data:`PAGE_BUDGET`, or its raised one."""
    return PAGE_BUDGETS.get(page, (PAGE_BUDGET, ""))[0]


BLOB_NAME = re.compile(r"\b[0-9a-f]{24}\.(?:bin|webp)\b")


def report(site: Path) -> dict:
    ev = site / "static" / "evidence"
    blobs = ev / "blobs"
    size = (
        {p.name: p.stat().st_size for p in blobs.iterdir() if p.is_file()} if blobs.is_dir() else {}
    )
    pages, used = {}, set()
    for f in sorted(ev.glob("*.json")):
        text = f.read_text(encoding="utf-8")
        names = set(BLOB_NAME.findall(text))
        used |= names
        missing = sorted(n for n in names if n not in size)

        doc = json.loads(text)
        examples = {}
        for e in doc.get("examples", []) if isinstance(doc, dict) else []:
            en = set(BLOB_NAME.findall(json.dumps(e)))
            examples[e["id"]] = sum(size.get(n, 0) for n in en)
        b = sum(size.get(n, 0) for n in names)
        pages[f.stem] = {
            "json": f.stat().st_size,
            "blobs": b,
            "total": f.stat().st_size + b,
            "n_blobs": len(names),
            "missing": missing,
            "examples": examples,
        }
    static = sum(
        p.stat().st_size
        for p in (site / "static").rglob("*")
        if p.is_file() and blobs not in p.parents
    )
    published_blobs = sum(size.get(n, 0) for n in used)
    html = (
        sum(p.stat().st_size for p in (site / "_site").rglob("*.html"))
        if (site / "_site").is_dir()
        else 0
    )
    return {
        "pages": pages,
        "static": static,
        "blobs": published_blobs,
        "n_blobs": len(used),
        "unused_blobs": sorted(set(size) - used),
        "html": html,
        "total": static + published_blobs + html,
    }


def problems(rep: dict, site_budget: int = SITE_BUDGET) -> list[str]:
    out = []
    if rep["total"] > site_budget:
        big = sorted(rep["pages"].items(), key=lambda kv: -kv[1]["total"])[:5]
        out.append(
            f"the published site would be {rep['total'] / 1e6:.1f} MB (budget {site_budget / 1e6:.0f} MB); "
            "largest pages: " + ", ".join(f"{k} {v['total'] / 1e6:.1f} MB" for k, v in big)
        )
    for name, p in rep["pages"].items():
        page_budget = budget(name)
        if p["total"] > page_budget:
            big = sorted(p["examples"].items(), key=lambda kv: -kv[1])[:4]
            out.append(
                f"page {name}: {p['total'] / 1e6:.1f} MB (budget {page_budget / 1e6:.0f} MB); largest "
                "examples: " + ", ".join(f"{k} {v / 1e6:.1f} MB" for k, v in big)
            )
        if p["missing"]:
            out.append(f"page {name}: {len(p['missing'])} blob(s) missing, e.g. {p['missing'][0]}")
    return out


def table(rep: dict) -> str:
    rows = [f"{'page':<24} {'JSON':>9} {'images':>9} {'total':>9} {'blobs':>6}  largest example"]
    for name, p in sorted(rep["pages"].items(), key=lambda kv: -kv[1]["total"]):
        big = max(p["examples"].items(), key=lambda kv: kv[1], default=("", 0))
        rows.append(
            f"{name:<24} {p['json'] / 1e6:>8.2f}M {p['blobs'] / 1e6:>8.2f}M {p['total'] / 1e6:>8.2f}M "
            f"{p['n_blobs']:>6}  {big[0]} {big[1] / 1e6:.2f}M" if big[0] else
            f"{name:<24} {p['json'] / 1e6:>8.2f}M {p['blobs'] / 1e6:>8.2f}M {p['total'] / 1e6:>8.2f}M {p['n_blobs']:>6}"
        )  # fmt: skip
    rows.append(
        f"site: {rep['total'] / 1e6:.1f} MB = static files {rep['static'] / 1e6:.1f} MB + "
        f"{rep['n_blobs']} published blobs {rep['blobs'] / 1e6:.1f} MB + built HTML "
        f"{rep['html'] / 1e6:.1f} MB"
        + ("" if rep["html"] else " (not built)")
        + (
            f"; {len(rep['unused_blobs'])} unused blob file(s) not published"
            if rep["unused_blobs"]
            else ""
        )
        + f"; budgets: site {SITE_BUDGET / 1e6:.0f} MB, page {PAGE_BUDGET / 1e6:.0f} MB"
    )
    for name, (b, why) in PAGE_BUDGETS.items():
        rows.append(f"budget of {name} raised to {b / 1e6:.0f} MB: {why}")
    return "\n".join(rows)


def write_manifest(site: Path) -> Path:
    """``static/evidence/blobs.sha256``: every blob a JSON names, with its full SHA-256
    (``sha256sum`` format, sorted by name). The site's ``tools/blobs.py`` verifies the blob
    branch against it before publishing."""
    ev = site / "static" / "evidence"
    names: set[str] = set()
    for f in ev.glob("*.json"):
        names |= set(BLOB_NAME.findall(f.read_text(encoding="utf-8")))
    lines = []
    for n in sorted(names):
        h = hashlib.sha256((ev / "blobs" / n).read_bytes()).hexdigest()
        if not h.startswith(n.split(".")[0]):
            raise ValueError(f"blob {n}: its content's SHA-256 is {h}")
        lines.append(f"{h}  {n}\n")
    path = ev / "blobs.sha256"
    text = "".join(lines)
    if not path.is_file() or path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8")
    return path


__all__ = [
    "PAGE_BUDGET",
    "PAGE_BUDGETS",
    "SITE_BUDGET",
    "budget",
    "problems",
    "report",
    "table",
    "write_manifest",
]
