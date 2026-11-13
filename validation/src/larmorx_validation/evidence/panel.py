# SPDX-License-Identifier: Apache-2.0
"""The documentation site's Data panel page: what each dataset of larmorx's data panel is, why it
is there, its licence and citation, and the defacing check of its anatomical images, from
larmorx-testdata (``panel.toml`` and the catalog; ``docs/panel.md`` there).

``python -m larmorx_validation evidence panel --site <site>`` writes ``static/evidence/panel.json``
(``all`` does it too); the page ``content/validation/data-panel.md`` draws it with
``{{ ev.panel_table() }}`` and its kin. The catalog's texts are shown in the writing guide's words
(``_words``: the original's runs, programs)."""

from __future__ import annotations

import tomllib
from pathlib import Path

from larmorx_validation.evidence import core, credits

#: The catalog's words (larmorx-testdata's panel.toml, written before the writing guide) and the
#: guide's (docs/writing-guide.md, section 1), applied where the page shows them.
_WORDS = (
    ("An fMRIPrep 25.2.6 oracle run", "An fMRIPrep 25.2.6 run"),
    ("the oracle run reads", "fMRIPrep's run reads"),
    ("every tool's reference case", "every program's reference case"),
)


def _words(text: str) -> str:
    for old, new in _WORDS:
        text = text.replace(old, new)
    return text


def _testdata_root() -> Path:
    from larmorx_testdata.catalog import REPO_ROOT

    return Path(REPO_ROOT)


def run_status(run: str) -> str:
    """An fMRIPrep oracle run's state, from its ``RUN_INFO.txt``: ``finished`` (exit status 0),
    ``failed``, ``running`` or ``not run yet``."""
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    info = (ws / "oracles" / "fmriprep-25.2.6" / run / "RUN_INFO.txt") if ws else None
    if info is None or not info.is_file():
        return "not run yet"
    text = info.read_text(encoding="utf-8")
    if "exit status: 0" in text:
        return "finished"
    return "failed" if "exit status:" in text else "running"


def export(ctx: core.Context) -> dict:
    import larmorx_testdata as td

    root = _testdata_root()
    doc = tomllib.loads((root / "panel.toml").read_text(encoding="utf-8"))
    cat = td.catalog()
    datasets = []
    for d in doc["dataset"]:
        cid = d["collection"]
        files = cat.select(tier="full", tags={"panel"}, collection=cid)
        images = [e for e in files if e.path.endswith((".nii", ".nii.gz"))]
        datasets.append(
            {
                "id": d["id"],
                "collection": cid,
                "subject": d["subject"],
                "session": d.get("session", ""),
                "age": str(d.get("age", "")),
                "sex": d.get("sex", ""),
                "scanner": d.get("scanner", ""),
                "role": _words(d["role"]),
                "covers": d["covers"],
                "fmriprep_run": d.get("fmriprep_run", ""),
                "fmriprep_options": d.get("fmriprep_options", "--fs-no-reconall"),
                "fmriprep_status": run_status(d.get("fmriprep_run", "")),
                "note": _words(d.get("note", "")),
                "files": len(files),
                "images": len(images),
                "bytes": sum(e.size for e in files),
                "megabytes": sum(e.size for e in files) / 1e6,
                "anat": [
                    {
                        "file": a["path"].split("/", 2)[2],
                        "defacing": a["defacing"],
                        "show": a["show"],
                    }
                    for a in d.get("anat", [])
                ],
                "credit": credits.credit(cid),
            }
        )
    return {
        "approved": doc["panel"]["approved"],
        "summary": doc["panel"]["summary"],
        "criteria": [
            {"id": c[0], "label": _words(c[1]), "group": c[2]} for c in doc["panel"]["criteria"]
        ],
        "datasets": datasets,
        "totals": {
            "files": sum(d["files"] for d in datasets),
            "bytes": sum(d["bytes"] for d in datasets),
            "gigabytes": sum(d["bytes"] for d in datasets) / 1e9,
        },
        "source": {
            "repository": "https://github.com/karellopez/larmorx-testdata",
            "definition": "panel.toml",
            "document": "docs/panel.md",
            "fetch": "python -m larmorx_testdata fetch --tier full --tag panel",
        },
    }


__all__ = ["export"]
