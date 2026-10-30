# SPDX-License-Identifier: Apache-2.0
"""The tables that repeat the records' facts, generated (decision log 2026-10-23, point 2).

One description per tool and per record (``validation/tools.toml``) and the records' JSON
(``docs/validation/<record>.json``) give three tables, each written between its markers:

- the README's "Supported tools" table (``README.md``);
- the validation index's table (``docs/validation/README.md``);
- the site's status table (``data/status.yml`` of ``larmorx_documentation``, its
  ``implemented:`` list), with ``--site``.

``python -m larmorx_validation tables [--site DIR]`` writes them; ``--check`` writes nothing and
fails when a committed table differs from its generated form (``validation/tests/test_tables.py``
runs it for the two tables of this repository).

The numbers: :data:`METRICS` computes, from each record's JSON, the counts its texts cite
(``{record.metric}``). Every record has ``cases`` (every case), ``pass``, ``both`` (rejected by
both), ``expected`` (expected divergences), ``fail``, ``compared`` (``pass`` + ``fail``); the
functions below add what a suite's own checks report (bit-identity as built and with
correctly rounded maths, headers).
"""

from __future__ import annotations

import json
import os
import re
import sys
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

REPO = Path(__file__).resolve().parents[3]
DESCRIPTIONS = REPO / "validation" / "tools.toml"
RECORDS = REPO / "docs" / "validation"

BEGIN = "<!-- BEGIN GENERATED: {name} (python -m larmorx_validation tables; edit validation/tools.toml) -->"
END = "<!-- END GENERATED: {name} -->"
YAML_BEGIN = "# BEGIN GENERATED: {name} (python -m larmorx_validation tables in the main repository; edit its validation/tools.toml)"
YAML_END = "# END GENERATED: {name}"


# ------------------------------------------------------------------------------------------------
# Metrics


def _reported(r: dict, name: str):
    return (r.get("reported") or {}).get(name)


def _zero(v) -> bool:
    """A reported check that says "identical": the count 0, or ``True`` (``1 == True`` in
    Python, so the types are told apart)."""
    if isinstance(v, bool):
        return v
    if isinstance(v, int | float):
        return v == 0
    return v in ("0", "True")


def _common(doc: dict) -> dict[str, int]:
    s = doc.get("summary", {})
    res = doc.get("results", [])
    return {
        "cases": len(res),
        "pass": s.get("pass", 0),
        "both": s.get("both-error", 0),
        "expected": s.get("expected-divergence", 0),
        "fail": s.get("fail", 0),
        "skipped": s.get("skipped", 0),
        "compared": s.get("pass", 0) + s.get("fail", 0),
    }


def _bit_identical(check: str = "bit-identical"):
    """Compared = the cases that report ``check``; identical = those where it is 0."""

    def metric(doc: dict) -> dict[str, int]:
        compared = [r for r in doc["results"] if _reported(r, check) is not None]
        return {
            "compared": len(compared),
            "identical": sum(_zero(_reported(r, check)) for r in compared),
        }

    return metric


def _as_built(first: str, suffix: str = ": bit-identical to ANTs as built"):
    """N4, DenoiseImage, Atropos: compared = the cases that report the first output's
    bit-identity with ANTs as built; identical = every output's is 0."""

    def metric(doc: dict) -> dict[str, int]:
        compared = [r for r in doc["results"] if _reported(r, first + suffix) is not None]
        identical = [
            r
            for r in compared
            if all(_zero(v) for k, v in (r.get("reported") or {}).items() if k.endswith(suffix))
        ]
        return {"compared": len(compared), "identical": len(identical)}

    return metric


def _unmodified(doc: dict) -> dict[str, int]:
    """antsRegistration, antsAI, antsAffineInitializer: a passing case identical to the
    unmodified ANTs reports nothing about it; one identical only with CORE-MATH reports the
    unmodified program's checks (``unmodified ANTs: ...``)."""
    passing = [r for r in doc["results"] if r["status"] == "pass"]
    as_built = [
        r
        for r in passing
        if not any(k.startswith("unmodified ANTs") for k in (r.get("reported") or {}))
    ]
    return {"as_built": len(as_built), "with_crmath": len(passing) - len(as_built)}


def _image_math(doc: dict) -> dict[str, int]:
    m = _bit_identical()(doc)
    compared = [r for r in doc["results"] if _reported(r, "bit-identical") is not None]
    m["headers"] = sum(_zero(_reported(r, "header bytes identical")) for r in compared)
    return m


def _unifize(doc: dict) -> dict[str, int]:
    """3dUnifize's clean-room original: the cases that write outputs (a measured ``<output>:
    data`` check), those whose every output is bit-identical to AFNI's (no ``values
    bit-identical`` report above 0), and the others, bit-identical once AFNI uses correctly
    rounded maths."""
    written = identical = crmath = 0
    for r in doc["results"]:
        if r["status"] != "pass" or not any(
            k.endswith(": data") for k in (r.get("measured") or {})
        ):
            continue
        written += 1
        rep = r.get("reported") or {}
        values = [v for k, v in rep.items() if k.endswith(": values bit-identical to AFNI's")]
        if all(_zero(v) for v in values):
            identical += 1
        elif _zero(rep.get("values bit-identical to AFNI's with correctly rounded maths")):
            crmath += 1
    return {"written": written, "identical": identical, "with_crmath": crmath}


def _bet(doc: dict) -> dict[str, int]:
    """BET's replica: fMRIPrep's calls (the category ``fmriprep``) and the three kinds of
    expected divergence: FSL crashes and ``reproduce`` fills what it leaves undefined
    (``filled``), options that run FSL programs not replicated yet (``pending``), FSL's fill
    reading memory beyond the volume (``memory``, FSL-43)."""
    res = doc["results"]
    expected = [r for r in res if r["status"] == "expected-divergence"]
    return {
        "fmriprep": sum(r["category"] == "fmriprep" for r in res),
        "filled": sum(r["reason"].startswith("FSL crashes (status") for r in expected),
        "pending": sum(r["reason"].startswith("needs FSL programs") for r in expected),
        # FSL-43's fill reading memory beyond the volume (2026-10-29): FSL's values undefined
        "memory": sum(r["reason"].startswith("FSL-43: ") for r in expected),
    }


#: record -> its metrics beyond the common ones.
METRICS: dict[str, Callable[[dict], dict[str, int]]] = {
    "ants-apply-transforms": _bit_identical(),
    "ants-n4": _as_built("corrected"),
    "ants-denoise": _as_built("denoised"),
    "ants-image-math": _image_math,
    "ants-resample-image-by-spacing": _bit_identical(),
    "ants-registration": _unmodified,
    "ants-registration-velocity-full": _unmodified,
    "ants-ai": _unmodified,
    "ants-ai-full": _unmodified,
    "ants-affine-initializer": _unmodified,
    "ants-atropos": _as_built("seg"),
    "afni-tshift": _bit_identical(),
    "afni-tshift-replica": _bit_identical(),
    "afni-unifize": _unifize,
    # The GPL replicas of 3dAutomask and 3dUnifize: the same counts from their own records.
    "afni-automask-replica": _unifize,
    "afni-unifize-replica": _unifize,
    "fsl-bet": _bet,
    "fsl-bet-replica": _bet,
    "fsl-bet-replica-full": _bet,
    # The data panel's records of the ANTs tools (2026-10-26): the same metrics as their
    # standard records.
    "ants-apply-transforms-panel": _bit_identical(),
    "ants-n4-panel": _as_built("corrected"),
    "ants-denoise-panel": _as_built("denoised"),
    "ants-image-math-panel": _image_math,
    "ants-threshold-image-panel": _bit_identical(),
    "ants-multiply-images-panel": _bit_identical(),
    "ants-smooth-image-panel": _bit_identical(),
    "ants-resample-image-panel": _bit_identical(),
    "ants-resample-image-by-spacing-panel": _bit_identical(),
    "ants-registration-panel": _unmodified,
    # The fourteen whole template registrations, run on their own (2026-10-28).
    "ants-registration-panel-template": _unmodified,
    "ants-ai-panel": _unmodified,
    "ants-affine-initializer-panel": _unmodified,
    "ants-atropos-panel": _as_built("seg"),
}


def record_metrics(record: str, records: Path = RECORDS) -> SimpleNamespace:
    path = records / f"{record}.json"
    doc = json.loads(path.read_text(encoding="utf-8"))
    m = _common(doc)
    if record in METRICS:
        m.update(METRICS[record](doc))
    return SimpleNamespace(**m)


# ------------------------------------------------------------------------------------------------
# Rendering


@dataclass(frozen=True)
class Place:
    """Where a table goes: how a record and a repository file are linked from it."""

    name: str
    record: Callable[[str], str]
    repo: Callable[[str], str]


README = Place("README.md", lambda r: f"docs/validation/{r}.md", lambda p: p)
INDEX = Place(
    "docs/validation/README.md",
    lambda r: f"{r}.md",
    lambda p: os.path.relpath(p, "docs/validation").replace(os.sep, "/"),
)
SITE = Place("data/status.yml", lambda r: f"repo:docs/validation/{r}.md", lambda p: f"repo:{p}")


class _Records(dict):
    def __init__(self, records: Path):
        super().__init__()
        self.records = records

    def __missing__(self, key: str):
        value = record_metrics(key, self.records)
        self[key] = value
        return value


def render_text(template: str, place: Place, metrics: _Records) -> str:
    text = template.format_map(metrics)
    text = re.sub(r"\]\(record:([^)]+)\)", lambda m: "](" + place.record(m.group(1)) + ")", text)
    return re.sub(r"\]\(repo:([^)]+)\)", lambda m: "](" + place.repo(m.group(1)) + ")", text)


def descriptions(path: Path = DESCRIPTIONS) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def readme_table(desc: dict, metrics: _Records) -> str:
    rows = [
        "| Tool | Command / Python | How | Validation (standard tier) |",
        "|---|---|---|---|",
    ]
    for t in desc["tool"]:
        use = ", ".join(f"`{u}`" for u in t["use"])
        rows.append(
            f"| {t['title']} | {use} | {t['how']} | {render_text(t['validation'], README, metrics)} |"
        )
    return "\n".join(rows)


def index_table(desc: dict, metrics: _Records) -> str:
    rows = ["| Record | larmorx | Reference | Status |", "|---|---|---|---|"]
    for r in desc["record"]:
        rows.append(
            f"| [{r['id']}]({r['id']}.md) | {render_text(r['larmorx'], INDEX, metrics)} | "
            f"{render_text(r['reference'], INDEX, metrics)} | {render_text(r['status'], INDEX, metrics)} |"
        )
    return "\n".join(rows)


def _yaml_string(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def site_table(desc: dict, metrics: _Records) -> str:
    lines = ["implemented:"]
    for t in desc["tool"]:
        lines += [
            f"  - eco: {t['eco']}",
            f"    tool: {_yaml_string(t['title'])}",
            f"    page: {t['page']}",
            "    use: [" + ", ".join(_yaml_string(u) for u in t["use"]) + "]",
            f"    how: {_yaml_string(t['how'])}",
            f"    validation: {_yaml_string(render_text(t['validation'], SITE, metrics))}",
        ]
    return "\n".join(lines)


def replace_block(text: str, name: str, body: str, *, yaml: bool = False) -> str:
    begin = (YAML_BEGIN if yaml else BEGIN).format(name=name)
    end = (YAML_END if yaml else END).format(name=name)
    if begin not in text or end not in text:
        raise ValueError(f"the markers of {name!r} are missing")
    head, rest = text.split(begin, 1)
    _, tail = rest.split(end, 1)
    return f"{head}{begin}\n{body}\n{end}{tail}"


TARGETS = (
    (REPO / "README.md", "tools table", readme_table, False),
    (REPO / "docs" / "validation" / "README.md", "records table", index_table, False),
)


def generate(site: Path | None = None) -> dict[Path, str]:
    desc = descriptions()
    metrics = _Records(RECORDS)
    out = {}
    for path, name, fn, yaml in TARGETS:
        out[path] = replace_block(
            path.read_text(encoding="utf-8"), name, fn(desc, metrics), yaml=yaml
        )
    if site is not None:
        path = site / "data" / "status.yml"
        out[path] = replace_block(
            path.read_text(encoding="utf-8"),
            "implemented tools",
            site_table(desc, metrics),
            yaml=True,
        )
    return out


def main(args) -> int:
    site = Path(args.site) if args.site else None
    generated = generate(site)
    differ = [p for p, text in generated.items() if p.read_text(encoding="utf-8") != text]
    if args.check:
        for p in differ:
            print(
                f"{p}: differs from its generated form (python -m larmorx_validation tables)",
                file=sys.stderr,
            )
        return 1 if differ else 0
    for p, text in generated.items():
        if p in differ:
            p.write_text(text, encoding="utf-8")
            print(f"wrote {p}")
    return 0


def add_arguments(p) -> None:
    p.add_argument(
        "--check",
        action="store_true",
        help="fail when a committed table differs from its generated form",
    )
    p.add_argument("--site", help="the documentation site's checkout: also its data/status.yml")


__all__ = [
    "INDEX",
    "METRICS",
    "README",
    "SITE",
    "add_arguments",
    "generate",
    "index_table",
    "main",
    "readme_table",
    "record_metrics",
    "render_text",
    "site_table",
]
