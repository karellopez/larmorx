# SPDX-License-Identifier: Apache-2.0
"""Random testing on the site: each evidence page's "Random testing" panel and the Validation
page's summary across programs (``static/evidence/difftest.json``), from the latest long run of
differential random testing (``<workspace>/oracles/difftest/<date>/summary.json``,
``docs/validation/difftest.md``) and what was done about each difference
(``validation/difftest/triage.toml``: fixed with its permanent case, a register entry, a clean
room's open difference, unclear); and the runs of half an hour since then (``RUNS``: the parts'
random-testing runs, each as its evidence page shows and triages it), from their recorded
summaries (``<workspace>/oracles/difftest/<run>/summary.json``), never run again here.

``python -m larmorx_validation evidence difftest --site <site>`` (``all`` includes it). The long
run can be named with ``LARMORX_DIFFTEST_NIGHT`` (its directory); otherwise the newest one with a
``summary.json``. The JSON never says ``verdict``: random testing's outcomes are its own (agree,
explained, notice, undefined, differ, unclear), not the site's verdict taxonomy. Its texts are in
the writing guide's words (``docs/writing-guide.md``): the triage's explanations through
:func:`words.reword`, each implementation by its port's name (``ports``).
"""

from __future__ import annotations

import json
import os
import re
import tomllib
from pathlib import Path

from larmorx_validation.evidence import core, words

#: Each program's evidence page and how the page names it.
PAGES: dict[str, tuple[str, str]] = {
    "nifti-io": ("/evidence/nifti-io/", "NIfTI I/O"),
    "itk-transforms": ("/evidence/itk-hdf5/", "ITK transform files"),
    "ants-apply-transforms": ("/evidence/ants-apply-transforms/", "antsApplyTransforms"),
    "ants-image-math": ("/evidence/ants-programs/", "ImageMath"),
    "ants-threshold-image": ("/evidence/ants-programs/", "ThresholdImage"),
    "ants-multiply-images": ("/evidence/ants-programs/", "MultiplyImages"),
    "ants-smooth-image": ("/evidence/ants-programs/", "SmoothImage"),
    "ants-resample-image": ("/evidence/ants-programs/", "ResampleImage"),
    "ants-resample-image-by-spacing": ("/evidence/ants-programs/", "ResampleImageBySpacing"),
    "ants-n4": ("/evidence/ants-n4/", "N4BiasFieldCorrection"),
    "ants-denoise": ("/evidence/ants-denoise/", "DenoiseImage"),
    "ants-registration": ("/evidence/ants-registration/", "antsRegistration"),
    "ants-ai": ("/evidence/ants-ai/", "antsAI"),
    "ants-affine-initializer": ("/evidence/ants-ai/", "antsAffineInitializer"),
    "ants-atropos": ("/evidence/ants-atropos/", "Atropos"),
    "afni-tshift": ("/evidence/afni-tshift/", "3dTshift"),
    "afni-automask": ("/evidence/afni-automask/", "3dAutomask"),
    "afni-unifize": ("/evidence/afni-unifize/", "3dUnifize"),
    "mri-hmc": ("/evidence/mri-hmc/", "head-motion correction (larmorx mri hmc)"),
    "fsl-mcflirt": ("/evidence/mri-hmc/", "mcflirt"),
    "resample-series": ("/evidence/resample-series/", "one-shot resampler"),
}

#: Each implementation of the harness by its port's name (the writing guide, section 1): the
#: harness calls a clean room "original" (its descriptors' names, kept as they are) and a
#: program with one port "port".
PORT_NAMES: dict[str, str] = {"original": "clean room", "replica": "replica", "port": "replica"}
#: Where a tool's single port is not a replica.
TOOL_PORTS: dict[str, dict[str, str]] = {"nifti-io": {"port": "LARMORX's own"}}

#: The runs of half an hour since the long run, as each evidence page shows them (the run each
#: page's evidence module reads: ``TRIAL``, ``TRIALS``, ``TRIAL_CATCHUP``): the run's folder,
#: the tool in its summary, the implementation, the page, its random-testing section, the
#: program as the page names it.
RUNS: tuple[tuple[str, str, str, str, str, str], ...] = (
    (
        "2026-11-03-fsl-maths-replica",
        "fsl-maths",
        "replica",
        "/evidence/fsl-maths/",
        "random",
        "fslmaths",
    ),
    (
        "2026-11-03-fsl-maths-original",
        "fsl-maths-original",
        "original",
        "/evidence/fsl-maths/",
        "random",
        "fslmaths",
    ),
    (
        "2026-11-03-fsl-maths-replica",
        "fsl-stats",
        "replica",
        "/evidence/fsl-stats/",
        "random",
        "fslstats",
    ),
    (
        "2026-11-03-fsl-maths-original",
        "fsl-stats-original",
        "original",
        "/evidence/fsl-stats/",
        "random",
        "fslstats",
    ),
    (
        "2026-11-03-topup-replica",
        "fsl-topup",
        "replica",
        "/evidence/fsl-topup/",
        "random",
        "topup and applytopup",
    ),
    (
        "2026-11-03-topup-original",
        "fsl-topup-original",
        "original",
        "/evidence/fsl-topup/",
        "random",
        "topup and applytopup",
    ),
    (
        "2026-11-05-catchup-topup",
        "fsl-topup-original",
        "original",
        "/evidence/fsl-topup/",
        "random",
        "topup and applytopup",
    ),
    ("2026-11-05-catchup-bet", "fsl-bet", "original", "/evidence/fsl-bet/", "trial", "BET"),
    ("2026-11-05-catchup-bet", "fsl-bet", "replica", "/evidence/fsl-bet/", "trial", "BET"),
    (
        "2026-11-05-catchup-fast",
        "fsl-fast-original",
        "original",
        "/evidence/fsl-fast/",
        "random",
        "FAST",
    ),
    (
        "2026-11-05-catchup-flirt",
        "fsl-flirt-original",
        "original",
        "/evidence/fsl-flirt/",
        "random",
        "FLIRT",
    ),
    (
        "2026-11-05-catchup-hmc",
        "mri-hmc",
        "original",
        "/evidence/mri-hmc/",
        "random-testing",
        "head-motion correction (larmorx mri hmc)",
    ),
    (
        "2026-11-06-prelude-replica",
        "fsl-prelude",
        "replica",
        "/evidence/fsl-prelude/",
        "random",
        "prelude",
    ),
    (
        "2026-11-06-prelude-original",
        "fsl-prelude-original",
        "original",
        "/evidence/fsl-prelude/",
        "random",
        "prelude",
    ),
    (
        "2026-11-08-volreg-original-trial",
        "afni-volreg-original",
        "original",
        "/evidence/afni-volreg/",
        "random",
        "3dvolreg",
    ),
    (
        "2026-11-08-sdcflows-fieldmap",
        "sdcflows-fieldmap",
        "port",
        "/evidence/sdcflows-fieldmap/",
        "random-testing",
        "sdcflows' field-map estimation",
    ),
    (
        "2026-11-08-t2smap-replica",
        "tedana-t2smap",
        "replica",
        "/evidence/tedana-t2smap/",
        "random-testing",
        "t2smap",
    ),
    (
        "2026-11-08-t2smap-replica-check",
        "tedana-t2smap",
        "replica",
        "/evidence/tedana-t2smap/",
        "random-testing",
        "t2smap",
    ),
    (
        "2026-11-09-fieldmaps-b",
        "sdcflows-estimation",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "sdcflows' PEPOLAR, direct and fieldmap-less estimation",
    ),
    (
        "2026-11-09-fieldmaps-b",
        "niworkflows-helpers",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "niworkflows' helpers",
    ),
    (
        "2026-11-09-fieldmaps-b",
        "ndimage-skimage",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "SciPy's ndimage and scikit-image",
    ),
    (
        "2026-11-09-fieldmaps-b-noavx512",
        "sdcflows-estimation",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "sdcflows' PEPOLAR, direct and fieldmap-less estimation",
    ),
    (
        "2026-11-09-fieldmaps-b-noavx512",
        "niworkflows-helpers",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "niworkflows' helpers",
    ),
    (
        "2026-11-09-fieldmaps-b-noavx512",
        "ndimage-skimage",
        "port",
        "/evidence/sdcflows-estimation/",
        "random-testing",
        "SciPy's ndimage and scikit-image",
    ),
    (
        "2026-11-11-t2smap-original-2",
        "tedana-t2smap-original",
        "original",
        "/evidence/tedana-t2smap/",
        "random-testing",
        "t2smap",
    ),
)
#: A run's own words, where its folder's name alone does not say it.
RUN_NOTES: dict[str, str] = {
    "2026-11-03-fsl-maths-replica": "fslmaths and fslstats in one run",
    "2026-11-03-fsl-maths-original": "fslmaths and fslstats in one run",
    "2026-11-08-t2smap-replica-check": "the check of the final build",
    "2026-11-09-fieldmaps-b-noavx512": "numpy's AVX-512 kernels off",
}

#: "larmorx" in a sentence names LARMORX (the project): not in code, a path, a package's or a
#: crate's name (larmorx-nc, larmorx_io) or a command.
_LARMORX = re.compile(r"(?<![\w`/.-])larmorx(?![\w`/:-]|\.\w)")


#: The comparisons' runner by what it is (the writing guide: "parity" on pages users read).
_RUNNER = re.compile(r"\bthe parity runner\b")

#: The triage's development words on a page users read (the reviewer's round 25, I): a "trial"
#: is a random-testing run, a clean room's work is its own, from its specification, and a run is
#: named by its date, not its folder.
_TRIAL = [
    (re.compile(r"\bthe trial of (\d{4}-\d{2}-\d{2})"), r"the random-testing run of \1"),
    (re.compile(r"\bthe trial against\b"), "the random-testing run against"),
    (
        re.compile(r"\bthat part's (?:30-minute )?trial\b"),
        "that part's random-testing run of half an hour",
    ),
    (
        re.compile(r"\bthe version trial of (\d{4}-\d{2}-\d{2})"),
        r"the random-testing run of \1 with the newest versions",
    ),
    (re.compile(r"\bthe clean-room agent's\b"), "the clean room's own, from its specification"),
    (re.compile(r" \(oracles/difftest/(\d{4}-\d{2}-\d{2})-[\w-]+\)"), r" (its run of \1)"),
]


#: The harness's own names of a few checks, in words, where a page shows them (the reviewer's
#: round 25, B; the site's renderer shows difftest.json's with the same words, its
#: ``DIFFTEST_CHECKS``): "DN" marks the checks of the defects' notices, and the harness calls the
#: clean room "original".
CHECK_WORDS = {
    "DN defaults: replica = original": "the replica's default equal to the clean room's",
    "DN fix pass": "the `fix` pass",
    "DN notices in both modes": "the notices in `fix` and in `reproduce`",
    "larmorx panicked": "LARMORX stopped with an internal error",
}


def _trial(text: str) -> str:
    for pat, rep in _TRIAL:
        text = pat.sub(rep, text)
    return text


def _prose(text: str) -> str:
    """A triage explanation in the guide's words, its code spans kept as they are."""
    parts = re.split(r"(`[^`]*`)", text)
    return "".join(
        part
        if i % 2
        else _LARMORX.sub(
            "LARMORX", _RUNNER.sub("the comparison's runner", _trial(words.reword(part) or ""))
        )
        for i, part in enumerate(parts)
    )


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
    # A night's folder is named by its date alone; a part's short trial (``<date>-<topic>``)
    # covers only its own tools and never replaces the night the site shows.
    nights = sorted(
        p.parent
        for p in root.glob("*/summary.json")
        if re.fullmatch(r"\d{4}-\d{2}-\d{2}", p.parent.name)
    )
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
    out = {"status": best["status"], "explanation": _prose(best["explanation"])}
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
        impls = sorted(t.get("implementations", {}))
        names = {**PORT_NAMES, **TOOL_PORTS.get(name, {})}
        pages.setdefault(page, []).append(
            {
                "tool": name,
                "label": label,
                "implementations": impls,
                # Each implementation by its port's name (the writing guide's words).
                "ports": {i: names.get(i, i) for i in impls},
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
        "runs": runs_since(night),
        "provenance": {
            "summary": f"oracles/difftest/{night.name}/summary.json",
            "runs": sorted({f"oracles/difftest/{r[0]}/summary.json" for r in RUNS}),
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
        },
    }


def runs_since(night: Path) -> list[dict]:
    """The runs of half an hour since the long run (``RUNS``), one row per program, port and
    run, from each run's recorded summary: its date, length, seed and LARMORX's commit, and the
    implementation's cases by outcome. Their differences are triaged on the program's evidence
    page, which the row links."""
    rows = []
    for run, tool, impl, page, anchor, label in RUNS:
        path = night.parent / run / "summary.json"
        if not path.is_file():
            raise FileNotFoundError(
                f"random testing's run {run} (oracles/difftest/{run}/summary.json)"
            )
        s = json.loads(path.read_text(encoding="utf-8"))
        t = s["tools"][tool]
        outcomes = t.get("implementations", {}).get(impl)
        if outcomes is None:
            raise KeyError(f"{run}: {tool} has no implementation {impl}")
        rows.append(
            {
                "page": page,
                "section": anchor,
                "label": label,
                "port": {**PORT_NAMES, **TOOL_PORTS.get(tool, {})}.get(impl, impl),
                "run": run,
                "note": RUN_NOTES.get(run, ""),
                "date": s["date"],
                "minutes": round((s.get("elapsed_s") or 0) / 60, 1),
                "seed": s["seed"],
                "larmorx_commit": s.get("larmorx_commit"),
                "cases": sum(outcomes.values()),
                "outcomes": {k: outcomes[k] for k in (*OUTCOMES,) if outcomes.get(k)},
                "unclear": sum(outcomes.get(k, 0) for k in UNCLEAR),
            }
        )
    return rows


__all__ = [
    "OUTCOMES",
    "PAGES",
    "PORT_NAMES",
    "RUNS",
    "export",
    "latest_night",
    "runs_since",
    "triage",
]
