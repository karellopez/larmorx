# SPDX-License-Identifier: Apache-2.0
"""Random testing on the site: each tool page's "Random testing" panel and the Validation page's
summary across tools (``static/evidence/difftest.json``), from the latest night of differential
random testing (``<workspace>/oracles/difftest/<date>/summary.json``,
``docs/validation/difftest.md``) and what was done about each difference
(``validation/difftest/triage.toml``: fixed with its permanent case, a register entry, a
clean-room original's open difference, unclear).

``python -m larmorx_validation evidence difftest --site <site>`` (``all`` includes it). The night
can be named with ``LARMORX_DIFFTEST_NIGHT`` (its directory); otherwise the newest one with a
``summary.json``. The JSON never says ``verdict``: random testing's outcomes are its own (agree,
explained, notice, undefined, differ, unclear), not the site's verdict taxonomy.
"""

from __future__ import annotations

import json
import os
import tomllib
from pathlib import Path

from larmorx_validation.evidence import core

#: Each tool's site page and how the page names it.
PAGES: dict[str, tuple[str, str]] = {
    "nifti-io": ("/tools/nifti-io/", "NIfTI I/O"),
    "itk-transforms": ("/tools/itk-hdf5/", "ITK transform files"),
    "ants-apply-transforms": ("/tools/ants-apply-transforms/", "antsApplyTransforms"),
    "ants-image-math": ("/tools/ants-programs/", "ImageMath"),
    "ants-threshold-image": ("/tools/ants-programs/", "ThresholdImage"),
    "ants-multiply-images": ("/tools/ants-programs/", "MultiplyImages"),
    "ants-smooth-image": ("/tools/ants-programs/", "SmoothImage"),
    "ants-resample-image": ("/tools/ants-programs/", "ResampleImage"),
    "ants-resample-image-by-spacing": ("/tools/ants-programs/", "ResampleImageBySpacing"),
    "ants-n4": ("/tools/ants-n4/", "N4BiasFieldCorrection"),
    "ants-denoise": ("/tools/ants-denoise/", "DenoiseImage"),
    "ants-registration": ("/tools/ants-registration/", "antsRegistration"),
    "ants-ai": ("/tools/ants-ai/", "antsAI"),
    "ants-affine-initializer": ("/tools/ants-ai/", "antsAffineInitializer"),
    "ants-atropos": ("/tools/ants-atropos/", "Atropos"),
    "afni-tshift": ("/tools/afni-tshift/", "3dTshift"),
    "afni-automask": ("/tools/afni-automask/", "3dAutomask"),
    "afni-unifize": ("/tools/afni-unifize/", "3dUnifize"),
    "mri-hmc": ("/tools/mri-hmc/", "head-motion correction, clean-room original"),
    "fsl-mcflirt": ("/tools/mri-hmc/", "mcflirt replica"),
    "resample-series": ("/tools/resample-series/", "one-shot resampler"),
}
OUTCOMES = (
    "agree",
    "explained",
    "notice",
    "undefined",
    "differ",
    "memory",
    "guard",
    "timeout",
    "crash",
    "skipped",
    "harness-error",
    "draw-error",
)
UNCLEAR = ("timeout", "crash", "harness-error", "draw-error")
TRIAGE = "validation/difftest/triage.toml"


def latest_night(repo: Path) -> Path | None:
    if env := os.environ.get("LARMORX_DIFFTEST_NIGHT"):
        return Path(env)
    from larmorx_validation.oracle_cache.store import workspace

    ws = workspace()
    root = None if ws is None else ws / "oracles" / "difftest"
    if root is None or not root.is_dir():
        return None
    nights = sorted(p.parent for p in root.glob("*/summary.json"))
    return nights[-1] if nights else None


def triage(repo: Path) -> list[dict]:
    path = repo / TRIAGE
    return (
        tomllib.loads(path.read_text(encoding="utf-8")).get("difference", [])
        if path.is_file()
        else []
    )


def _outcome_of(diff: dict, entries: list[dict]) -> dict:
    """What was done about a night's difference: the triage entry of its tool and
    implementation whose failed checks match its own best (an entry with the same checks
    first, then the most checks in common, then the first in the file); a difference no entry
    shares a check with is not yet triaged."""
    best, best_key = None, None
    for i, e in enumerate(entries):
        if (
            e["tool"] != diff["tool"]
            or e.get("implementation", diff["implementation"]) != diff["implementation"]
        ):
            continue
        common = set(e["signature"]) & set(diff["signature"])
        if not common:
            continue
        key = (set(e["signature"]) == set(diff["signature"]), len(common), -i)
        if best_key is None or key > best_key:
            best, best_key = e, key
    if best is None:
        return {"status": "unclear", "explanation": "not yet triaged"}
    out = {"status": best["status"], "explanation": best["explanation"]}
    if best.get("link"):
        out["link"] = best["link"]
    if best.get("defect"):
        out["defect"] = best["defect"]
    return out


def export(ctx: core.Context) -> dict:
    night = latest_night(ctx.repo)
    if night is None:
        raise FileNotFoundError("no night of random testing (oracles/difftest/<date>/summary.json)")
    s = json.loads((night / "summary.json").read_text(encoding="utf-8"))
    entries = triage(ctx.repo)
    pages: dict[str, list[dict]] = {}
    totals = {k: 0 for k in OUTCOMES}
    for name, t in s["tools"].items():
        page, label = PAGES.get(name, (None, name))
        if page is None:
            continue
        diffs = []
        for d in t["differences"]:
            o = _outcome_of({**d, "tool": name}, entries)
            diffs.append(
                {
                    "implementation": d["implementation"],
                    "checks": d["signature"],
                    "cases": d["cases"],
                    "reproducer": d["case"],
                    **o,
                }
            )
        for k, v in t["verdicts"].items():
            totals[k] = totals.get(k, 0) + v
        pages.setdefault(page, []).append(
            {
                "tool": name,
                "label": label,
                "implementations": sorted(t.get("implementations", {})),
                "cases": t["cases"],
                "outcomes": {k: t["verdicts"][k] for k in OUTCOMES if t["verdicts"].get(k)},
                # Cases that ended without a verdict of their own (for the orchestrator).
                "unclear": sum(t["verdicts"].get(k, 0) for k in UNCLEAR),
                "options_covered": t["options_covered"],
                # The cases drawn as the night's extreme share (gen.EXTREME_SHARE).
                "extreme": t.get("extreme", 0),
                "features_covered": t["features_covered"],
                "differences": diffs,
                "notices": [
                    {"defect": k, "cases": v["cases"]} for k, v in t.get("notices", {}).items()
                ],
                # Cases over the night's memory limit, run to their end later (``--heavy``).
                "heavy": [
                    {
                        "case": f"{h['tool']}:{h['seed']}:{h['n']}",
                        "result": (h.get("result") or {}).get("verdict", "waiting"),
                        "peak": (h.get("result") or {}).get("peak"),
                    }
                    for h in s.get("heavy", [])
                    if h["tool"] == name
                ],
            }
        )
    return {
        "schema": 1,
        "night": {
            "date": s["date"],
            "seed": s["seed"],
            "hours": round((s.get("elapsed_s") or 0) / 3600, 1),
            "jobs": s["jobs"],
            "threads": s["threads"],
            "larmorx_commit": s.get("larmorx_commit"),
            "cases": s["cases"],
            "outcomes": {k: v for k, v in totals.items() if v},
            "unclear": sum(totals.get(k, 0) for k in UNCLEAR),
            "tools": sum(len(v) for v in pages.values()),
        },
        # A list, not a mapping by URL: the site's JSON pointers stay plain.
        "pages": [{"page": page, "tools": tools_} for page, tools_ in pages.items()],
        "how": "docs/validation/difftest.md",
        "triage": TRIAGE,
        "provenance": {
            "summary": f"oracles/difftest/{night.name}/summary.json",
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
        },
    }


__all__ = ["OUTCOMES", "PAGES", "export", "latest_night", "triage"]
