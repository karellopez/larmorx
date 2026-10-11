# SPDX-License-Identifier: Apache-2.0
"""The record of a stage-by-stage comparison: ``<name>.md`` and ``<name>.json``.

The Markdown record opens with one row per stage, against fMRIPrep's output as it ran; then
the stages larmorx cannot run yet (the roadmap), the details of every node, and how to
reproduce the record. The JSON record has every number.
"""

from __future__ import annotations

import json
import math
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

from larmorx_validation.report import redact, table
from larmorx_validation.stages.catalogue import MISSING, STAGES, THRESHOLDS, Stage
from larmorx_validation.stages.compare import ImageDiff

#: FD correlation is required only where the largest FD reaches this (specs/mcflirt.md §14,
#: as in the mri-hmc record); below it, it is reported with mcflirt's own band.
FD_MIN_MM = 0.5
FD_R = 0.99
MEDIAN_MM = MEDIAN_DEG = 0.05


def _plain(x: Any) -> Any:
    if isinstance(x, ImageDiff):
        return _plain(x.to_dict())
    if isinstance(x, dict):
        return {str(k): _plain(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [_plain(v) for v in x]
    if isinstance(x, Path):
        return str(x)
    if isinstance(x, float):
        return None if math.isnan(x) else (str(x) if math.isinf(x) else x)
    if hasattr(x, "item") and callable(x.item):
        return _plain(x.item())
    if isinstance(x, str):
        return redact(x)
    return x


def _diff(d: Any) -> ImageDiff | None:
    if isinstance(d, ImageDiff) or d is None:
        return d
    return ImageDiff(**{**d, "shape": tuple(d["shape"])})


def _sec(x: float | None) -> str:
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return "–"
    if x < 10:
        return f"{x:.2f} s"
    if x < 100:
        return f"{x:.1f} s"
    return f"{x:.0f} s"


def _g(x: float, digits: int = 2) -> str:
    return f"{x:.{digits}g}"


# ------------------------------------------------------------------------------------------------
# Thresholds (PLAN.md §11.3)


def assess_output(stage: Stage, d: ImageDiff) -> tuple[bool, str]:
    """Whether one output passes PLAN.md §11.3's threshold, and the measure."""
    if d.identical:
        return True, "bit-identical"
    if d.note:
        return False, d.note
    if d.kind == "labels":
        return d.dice >= THRESHOLDS["masks"][2], f"Dice {d.dice:.6f}"
    name = stage.threshold
    if name == "n4-denoise":
        return d.ncc > THRESHOLDS[name][2], f"NCC {d.ncc:.7f}"
    if name == "bold":
        b = d.bold or {}
        ok = (
            b.get("mean_ncc", 0) > 0.99
            and b.get("tsnr_r", 0) > 0.98
            and b.get("timeseries_r_median", 0) > 0.98
        )
        return ok, ", ".join(f"{k} {v:.6f}" for k, v in b.items() if k != "mask_voxels")
    limit = THRESHOLDS.get(name, THRESHOLDS["itk-interpolation"])[2]
    if name == "scipy-interpolation" and d.bold:
        ok, text = assess_output(Stage("", "", "", "", "", "", "bold"), d)
        return d.max_rel <= limit or ok, f"max rel {_g(d.max_rel)}; {text}"
    text = f"max rel {_g(d.max_rel)}"
    over = (d.over or {}).get(f"{limit:g}")
    if over:
        text += f" ({over:,} of {d.voxels:,} voxels above {limit:g})"
    return d.max_rel <= limit, text


def assess_hmc(r: dict) -> tuple[bool, str]:
    m = r.get("matrices", {})
    ok = m.get("median_mm", 1) <= MEDIAN_MM and m.get("median_deg", 1) <= MEDIAN_DEG
    text = f"median Δ {m.get('median_mm', float('nan')):.4f} mm / {m.get('median_deg', float('nan')):.4f}°"
    c = r.get("confounds", {}).get("larmorx")
    if c:
        fd_max = r.get("fd_max_fmriprep", 0.0)
        if fd_max >= FD_MIN_MM:
            ok = ok and c["fd_r"] > FD_R
            text += f"; FD r {c['fd_r']:.4f}"
        else:
            text += f"; FD r {c['fd_r']:.4f} (reported: max FD {fd_max:.2f} mm < {FD_MIN_MM})"
    return ok, text


# ------------------------------------------------------------------------------------------------
# Summaries


def _node_label(r: dict) -> str:
    keys = r.get("keys", {})
    parts = []
    if run := keys.get("run"):
        parts.append(
            run.split("_run_")[-1] and f"run-{run.split('_run_')[-1]}" if "_run_" in run else run
        )
    if space := keys.get("space"):
        parts.append(space)
    name = r.get("name", "")
    if item := keys.get("item"):
        name = item.lstrip("_")
    parts.append(f"`{name}`")
    return " ".join(parts)


def _outputs(r: dict) -> list[dict]:
    return r.get("outputs", [])


def stage_summary(stage: Stage, results: list[dict]) -> dict[str, Any]:
    """One row of the first table."""
    if stage.kind == "hmc":
        return _hmc_summary(stage, results)
    n_out = 0
    identical = 0
    worst: ImageDiff | None = None
    passes = True
    measures = []
    causes: dict[str, int] = defaultdict(int)
    unexplained = 0
    known = 0
    errors = [r for r in results if r.get("status") in ("larmorx-error", "harness-error", "error")]
    for r in results:
        for o in _outputs(r):
            d = _diff(o["vs_fmriprep"])
            n_out += 1
            ok, measure = assess_output(stage, d)
            passes = passes and ok
            if d.identical:
                identical += 1
                continue
            measures.append(measure)
            if (
                worst is None
                or (d.ncc < worst.ncc)
                or (d.kind == "labels" and (d.dice or 1) < (worst.dice or 1))
            ):
                worst = d
            cause = o.get("cause")
            if cause:
                for c in cause["causes"]:
                    causes[c] += 1
                if cause.get("known"):
                    known += 1
                elif not cause["explained"]:
                    unexplained += 1
            elif stage.kind == "resample":
                unexplained += 1
    if errors:
        passes = False
    return {
        "outputs": n_out,
        "identical": identical,
        "worst": worst,
        "passes": passes,
        "measures": measures,
        "causes": dict(causes),
        "unexplained": unexplained,
        "known": known,
        "errors": len(errors),
    }


def _hmc_summary(stage: Stage, results: list[dict]) -> dict[str, Any]:
    passes = True
    measures = []
    for r in results:
        if r.get("status") != "compared":
            passes = False
            continue
        ok, text = assess_hmc(r)
        passes = passes and ok
        measures.append(text)
    return {
        "outputs": len(results),
        "identical": 0,
        "passes": passes,
        "measures": measures,
        "causes": {},
        "unexplained": 0,
        "known": 0,
        "errors": sum(r.get("status") != "compared" for r in results),
    }


def _result_text(stage: Stage, s: dict, results: list[dict]) -> str:
    if stage.kind == "hmc":
        rms = [r["matrices"] for r in results if "matrices" in r]
        if not rms:
            return "not compared"
        p95 = max(m["rms_p95_mm"] for m in rms)
        med = max(m["median_mm"] for m in rms)
        deg = max(m["median_deg"] for m in rms)
        return (
            f"not bit-exact (clean-room); RMS deviation 95th pct ≤ {p95:.3f} mm, "
            f"median Δparam ≤ {med:.4f} mm / {deg:.4f}°"
        )
    if s["errors"]:
        return f"{s['errors']} error(s)"
    if s["identical"] == s["outputs"]:
        return f"**bit-identical**: {s['outputs']} of {s['outputs']} outputs"
    w = s["worst"]
    text = f"{s['identical']} of {s['outputs']} outputs bit-identical"
    if w is not None:
        text += "; worst: " + w.summary()
    return text


CAUSE_TEXT = {
    "threads": "fMRIPrep's thread count",
    "run-to-run": "not reproducible run to run",
    "version": "the ANTs build (fMRIPrep's 2.6.2 vs the oracle's 2.6.5)",
    "glibc maths": "glibc's maths",
    "larmorx": "**larmorx**",
    "float pipeline": "**larmorx computes `--float 1` in double** (known; ANTs in double "
    "precision, rounded to float32, is larmorx bit for bit)",
}


def _cause_text(stage: Stage, s: dict, results: list[dict]) -> str:
    if stage.kind == "hmc":
        return "clean-room algorithm; mcflirt reproduces itself exactly"
    if s["identical"] == s["outputs"]:
        return "–"
    if stage.kind == "resample":
        return "**unexplained**" if s["unexplained"] else "–"
    parts = [CAUSE_TEXT.get(c, c) for c in s["causes"]]
    text = ", ".join(dict.fromkeys(parts)) or "–"
    if s["unexplained"]:
        text += f"; **{s['unexplained']} unexplained**"
    elif not s["known"]:
        text += "; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit"
    return text


def _times(results: list[dict], key: str, threads_key: str) -> str:
    vals = [r.get(key) for r in results if r.get(key) is not None]
    vals = [v for v in vals if not (isinstance(v, float) and math.isnan(v))]
    if not vals:
        return "–"
    threads = sorted({r.get(threads_key) for r in results if r.get(threads_key)})
    t = "/".join(str(x) for x in threads)
    return f"{_sec(sum(vals))} ({t}t)"


def _threshold_text(stage: Stage, s: dict) -> str:
    value = THRESHOLDS[stage.threshold][2]
    mark = "pass" if s["passes"] else "**fail**"
    if stage.kind == "hmc":
        return f"{mark} (≤ {MEDIAN_MM} mm / {MEDIAN_DEG}°; FD r > {FD_R} where max FD ≥ {FD_MIN_MM} mm)"
    if s["identical"] == s["outputs"]:
        return f"{mark} (bit-identical)"
    label = {"n4-denoise": f"NCC > {value}", "masks": f"Dice ≥ {value}"}.get(
        stage.threshold, f"max rel ≤ {_g(value)}" if isinstance(value, float) else str(value)
    )
    worst = s["worst"]
    if worst is not None and worst.kind == "labels":
        label = f"Dice ≥ {THRESHOLDS['masks'][2]}"
    return f"{mark} ({label})"


# ------------------------------------------------------------------------------------------------
# Markdown


def render(s, payload: dict, versions: dict, env: dict, command: str) -> str:
    run = s.run
    results = payload["results"]
    by_stage: dict[str, list[dict]] = defaultdict(list)
    for r in results:
        by_stage[r["stage"]].append(r)
    present = [st for st in STAGES if st.id in by_stage]
    absent = [st for st in STAGES if st.id not in by_stage]
    busy = payload.get("busy") or []
    load = payload.get("load_before", [0, 0, 0])
    quiet = not busy and load[0] < 1.5
    timing_note = (
        "on a quiet machine"
        if quiet
        else f"**on a shared machine** (load average {load[0]:.1f} at the start; other jobs: "
        f"{len(busy)}), so they are indicative only"
    )

    lines = [
        f"# Stage by stage: larmorx against fMRIPrep {run.version}",
        "",
        "larmorx's tools run on fMRIPrep's **own inputs and parameters** for each step it already "
        "implements, read from nipype's working directory (`command.txt`, `_report/report.rst`), and "
        "their outputs are compared with what fMRIPrep produced at that step. So every stage is "
        "judged on its own, with no error carried over from earlier stages. **The headline numbers "
        "are against fMRIPrep as it ran**; where they differ, the step is run again in fMRIPrep's "
        "own container and with the repository's oracle builds to find out why.",
        "",
        f"- **Oracle:** fMRIPrep {run.version}, image `nipreps/fmriprep@{run.digest}` "
        f"(Apptainer), run `{run.name}`: "
        f"`{redact(run.arguments)}`. Input: ds000005 sub-01 (one T1w, three BOLD runs of 240 "
        "volumes, no field maps, no slice timing).",
        f"- **Tools inside fMRIPrep's image:** {_tool_versions(versions)}.",
        f"- **larmorx:** {env['larmorx']} ({env['larmorx_commit']}), on {env['platform']}, "
        f"{env['cpu']}, {env['logical_cpus']} logical CPUs.",
        "- **References for the causes:** fMRIPrep's own tools re-run in its image, at its "
        "thread count and at one thread, as built and with CORE-MATH's correctly rounded "
        "functions preloaded (`scripts/build_crmath_preload.sh`; the image has this machine's "
        "glibc 2.35); ANTs 2.6.5 on ITK 5.4.5 built by `scripts/build_ants_oracle.sh` (one "
        "thread, as built and with CORE-MATH preloaded); FSL 6.0.7's `mcflirt` on this machine.",
        f"- **Generated:** {payload['started'][:10]}, with `{command}`. Times are {timing_note}; "
        "fMRIPrep's own times come from its run (8 jobs at once on a machine shared with a "
        "development agent, see `RUN_INFO.txt`), the re-runs of fMRIPrep's tools from this "
        "invocation, at fMRIPrep's thread counts.",
        "",
        "## Results",
        "",
    ]
    total_nodes = sum(len(v) for v in by_stage.values())
    lines += _headline(present, by_stage, total_nodes)
    rows = []
    for st in present:
        rs = by_stage[st.id]
        sm = stage_summary(st, rs)
        lx_key = "larmorx_seconds"
        if st.kind == "resample":
            # fMRIPrep's node also writes its output: compare like with like
            lx_key = "larmorx_total_seconds"
            for r in rs:
                if r.get("larmorx_seconds") is not None:
                    r[lx_key] = r["larmorx_seconds"] + (r.get("larmorx_write_seconds") or 0.0)
        rows.append(
            (
                st.id,
                st.title,
                str(len(rs)),
                f"`{st.tool}`",
                _result_text(st, sm, rs),
                _cause_text(st, sm, rs),
                _threshold_text(st, sm),
                _times(rs, "fmriprep_seconds", "fmriprep_threads"),
                _times(rs, "container_seconds", "fmriprep_threads"),
                _times(rs, lx_key, "larmorx_threads"),
            )
        )
    lines += [
        table(
            (
                "Stage",
                "Step",
                "Nodes",
                "larmorx",
                "Against fMRIPrep as it ran",
                "Why they differ",
                "PLAN.md §11.3",
                "fMRIPrep (its run)",
                "fMRIPrep's tool re-run",
                "larmorx",
            ),
            rows,
        ),
        "",
        "Times are sums over the stage's nodes, with the thread counts used. fMRIPrep's run "
        "times include nipype's own overhead for each node; the re-runs time the program alone "
        "(inside the image, without Apptainer's start-up), and larmorx's the call (the command "
        "line in-process for the ANTs programs and head-motion correction; computing and writing "
        "the output for the resampling).",
        "",
    ]
    if absent:
        lines += ["**Not run in this oracle run:**", ""]
        lines += [f"- {st.id} {st.title}: {st.absent or 'no such node'}." for st in absent]
        lines.append("")
    lines += _roadmap(payload)
    for st in present:
        lines += _details(st, by_stage[st.id])
    lines += [
        "## Reproduce",
        "",
        "```bash",
        "scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds000005 01 "
        "ds000005-sub-01-noreconall --fs-no-reconall   # the oracle run (about an hour)",
        "scripts/build_ants_oracle.sh && scripts/build_crmath_preload.sh   # the references",
        f"{command}",
        "```",
        "",
        "larmorx's outputs and the reference runs are kept in "
        f"`<workspace>/oracles/compare/fmriprep-{run.version}/{run.name}/<node path>/<run>/` "
        "(`larmorx`, `container-<n>t`, `ants-2.6.5-1t`, `ants-2.6.5-1t-cr`, `fsl-6.0.7`), each with "
        "`run.json` (arguments, environment, exit status, time). `--reuse` reuses the one-thread "
        "reference runs of an earlier invocation; `--stage A01 F05` limits the comparison.",
        "",
    ]
    return "\n".join(redact(x) for x in lines)


def _version(versions: dict, key: str) -> str:
    """``2.6.2 (build h5f726e6_2)`` -> ``2.6.2``."""
    return str(versions.get(key, "?")).split(" (build")[0]


def _tool_versions(v: dict) -> str:
    import re

    ants = re.sub(r"^ANTs Version:\s*", "", str(v.get("ants_version", "?")))
    afni = re.search(r"AFNI_([\d.]+)", str(v.get("afni", "")))
    glibc = re.search(r"GLIBC ([^)]+)\) ([\d.]+)", str(v.get("glibc", "")))
    return (
        f"ANTs {ants} (conda-forge `ants` {_version(v, 'ants')}) on ITK {_version(v, 'libitk')}; "
        f"FSL `mcflirt` {_version(v, 'fsl-mcflirt')} (`newimage` {_version(v, 'fsl-newimage')}); "
        f"AFNI {afni.group(1) if afni else '?'}; nitransforms {v.get('nitransforms', '?')}, "
        f"SciPy {v.get('scipy', '?')}, numpy {v.get('numpy', '?')} on OpenBLAS "
        f"{_version(v, 'libopenblas')}; glibc {glibc.group(2) if glibc else '?'}"
    )


def _headline(present: list[Stage], by_stage: dict[str, list[dict]], total: int) -> list[str]:
    exact_nodes = 0
    compared = 0
    unexplained = []
    known: list[str] = []
    for st in present:
        for r in by_stage[st.id]:
            if st.kind == "hmc":
                continue
            compared += 1
            outs = _outputs(r)
            if outs and all(_diff(o["vs_fmriprep"]).identical for o in outs):
                exact_nodes += 1
            for o in outs:
                c = o.get("cause")
                if c and c.get("known"):
                    known.append(r["node"])
                elif c and not c["explained"]:
                    unexplained.append(r["node"])
            if st.kind == "resample" and r.get("status") == "different":
                unexplained.append(r["node"])
    hmc = [r for st in present if st.kind == "hmc" for r in by_stage[st.id]]
    known_nodes = sorted(set(known))
    text = [
        f"**{exact_nodes} of {compared} compared nodes are bit-identical to fMRIPrep's own "
        f"output** (plus {len(hmc)} head-motion corrections, which larmorx does with a clean-room "
        "algorithm and cannot match bit for bit). "
        + (
            "The other differences are explained by fMRIPrep's thread count, ANTs' version or "
            "glibc's maths (with those removed, larmorx is bit-identical)"
            if not unexplained
            else f"**{len(set(unexplained))} node(s) differ for reasons not found** (see the details)"
        )
        + (
            f", except {len(known_nodes)} node(s) where larmorx itself differs, for a known "
            "reason: " + "; ".join(sorted({KNOWN_TEXT[k] for k in _known_kinds(by_stage)})) + "."
            if known_nodes
            else "."
        ),
        "",
    ]
    return text


#: The known larmorx divergences (``nodes.KNOWN``), for the headline.
KNOWN_TEXT = {
    "float pipeline": (
        "`antsApplyTransforms --float 1`, which ANTs runs in single precision and larmorx in "
        "double (ANTs in double precision, rounded to float32, is larmorx's output bit for bit; "
        "the differences stay within PLAN.md §11.3's interpolation threshold)"
    ),
}


def _known_kinds(by_stage: dict[str, list[dict]]) -> set[str]:
    return {
        o["cause"]["known"]
        for rs in by_stage.values()
        for r in rs
        for o in _outputs(r)
        if o.get("cause", {}).get("known")
    }


def _roadmap(payload: dict) -> list[str]:
    missing = payload.get("missing", {})
    rows = []
    for m in MISSING:
        nodes = missing.get(m.tool, [])
        seconds = sum(n["seconds"] or 0 for n in nodes)
        rows.append(
            (
                m.tool,
                str(len(nodes)) if nodes else "–",
                _sec(seconds) if nodes else "–",
                m.needs,
                m.milestone,
            )
        )
    return [
        "## What larmorx cannot run yet",
        "",
        "Every other node of the run that runs a neuroimaging program, or a Python step larmorx "
        "will need, by the tool it needs. The time is fMRIPrep's (its run, 8 jobs at once). "
        'These are the next tools to port (CLAUDE.md, "Next").',
        "",
        table(
            (
                "Tool",
                "fMRIPrep nodes",
                "fMRIPrep time",
                "What fMRIPrep uses it for",
                "larmorx milestone",
            ),
            rows,
        ),
        "",
    ]


def _chain_text(link: dict) -> str:
    d = _diff(link["diff"])
    if d is None:
        return "not run"
    return "same" if d.identical else d.summary()


def _details(st: Stage, results: list[dict]) -> list[str]:
    lines = [f"## {st.id}: {st.title}", ""]
    if st.kind == "hmc":
        return lines + _hmc_details(results)
    if st.kind == "resample":
        return lines + _resample_details(st, results)
    rows = []
    chains = []
    for r in results:
        label = _node_label(r)
        if r.get("status") in ("skipped", "error", "larmorx-error", "harness-error"):
            rows.append((label, "–", r.get("status"), r.get("note", ""), "–", "–", "–", "–"))
            continue
        for o in _outputs(r):
            d = _diff(o["vs_fmriprep"])
            ok, measure = assess_output(st, d)
            rerun = _diff(o.get("rerun_vs_fmriprep"))
            rows.append(
                (
                    label,
                    o["output"],
                    d.summary(),
                    ("pass" if ok else "**fail**") + f": {measure}",
                    "–" if rerun is None else ("same" if rerun.identical else rerun.summary()),
                    _sec(r.get("fmriprep_seconds")) + f" ({r.get('fmriprep_threads')}t)",
                    _sec(r.get("container_seconds")),
                    _sec(r.get("larmorx_seconds")) + f" ({r.get('larmorx_threads')}t)",
                )
            )
            if o.get("chain"):
                chains.append((label, o))
    lines += [
        table(
            (
                "Node",
                "Output",
                "larmorx vs fMRIPrep",
                "PLAN.md §11.3",
                "fMRIPrep's tool re-run vs its run",
                "fMRIPrep",
                "Re-run",
                "larmorx",
            ),
            rows,
        ),
        "",
    ]
    if chains:
        lines += [
            "Where larmorx and fMRIPrep differ, each step of the chain changes one thing: "
            "fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's "
            "correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the "
            "repository's oracle build, one thread, correctly rounded (version and build) → "
            "larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), "
            "as the tool's own validation record does.",
            "",
        ]
        crow = []
        for label, o in chains:
            links = {link["to"]: link for link in o["chain"]}
            crow.append(
                (
                    label,
                    o["output"],
                    *(
                        _chain_text(links[k]) if k in links else "–"
                        for k in ("container-1t", "container-1t-cr", "ants-2.6.5-1t-cr", "larmorx")
                    ),
                    "explained"
                    if o["cause"]["explained"]
                    else (
                        "known: " + o["cause"]["known"]
                        if o["cause"].get("known")
                        else "**unexplained**"
                    ),
                    _chain_text({"diff": o.get("vs_ants_as_built")}),
                )
            )
        lines += [
            table(
                (
                    "Node",
                    "Output",
                    "fMRIPrep → 1 thread",
                    "→ correctly rounded",
                    "→ ANTs 2.6.5 (oracle build)",
                    "→ larmorx",
                    "",
                    "larmorx vs ANTs 2.6.5 as built",
                ),
                crow,
            ),
            "",
        ]
    derivs = [
        (_node_label(r), o["output"], o["derivative"], _diff(o["vs_derivative"]).summary())
        for r in results
        for o in _outputs(r)
        if o.get("derivative")
    ]
    if derivs:
        lines += [
            "Outputs that fMRIPrep also writes as derivatives, against the derivative:",
            "",
            table(("Node", "Output", "Derivative", "larmorx vs derivative"), derivs),
            "",
        ]
    return lines


def _hmc_details(results: list[dict]) -> list[str]:
    rows = []
    conf = []
    times = []
    for r in results:
        label = _node_label(r)
        if r.get("status") != "compared":
            rows.append((label, r.get("status"), r.get("note", ""), "", "", "", ""))
            continue
        m = r["matrices"]
        rerun = r.get("rerun", {})
        host = r.get("host_fsl", {})
        rows.append(
            (
                label,
                str(m["volumes"]),
                f"{m['rms_median_mm']:.4f} / {m['rms_p95_mm']:.4f} / {m['rms_max_mm']:.4f}",
                f"{m['median_mm']:.4f} / {m['median_deg']:.4f}",
                f"{m['max_mm']:.4f} / {m['max_deg']:.4f}",
                f"{rerun.get('identical_volumes', '–')} of {m['volumes']}",
                f"{host.get('identical_volumes', '–')} of {m['volumes']}" if host else "–",
            )
        )
        c = r.get("confounds", {})
        lx_c, check = c.get("larmorx"), c.get("check")
        itk = r.get("itk", {})
        if lx_c:
            band = r.get("band", {})
            conf.append(
                (
                    label,
                    f"{lx_c['median_mm']:.4f} / {lx_c['median_deg']:.4f}",
                    f"{lx_c['max_mm']:.4f} / {lx_c['max_deg']:.4f}",
                    f"{lx_c['fd_r']:.4f}"
                    + (f" (band {band['fd_r']:.4f})" if band.get("fd_r") else ""),
                    f"{r.get('fd_max_fmriprep', float('nan')):.3f}",
                    f"{lx_c['fd_mean_ref']:.4f} / {lx_c['fd_mean_test']:.4f}",
                    _g(lx_c["rmsd_max_diff"]),
                    _g(itk.get("larmorx_max_abs", float("nan"))),
                    "same"
                    if check and check["max_mm"] < 1e-12 and check["fd_max_diff"] < 1e-12
                    else "differs",
                )
            )
        times.append(
            (
                label,
                _sec(r.get("fmriprep_seconds")),
                _sec(r.get("container_seconds")),
                _sec(r.get("larmorx_seconds")) + f" ({r.get('larmorx_threads')}t)",
                _sec(r.get("api_seconds")) + f" ({r.get('api_threads')}t)",
                _sec(r.get("api_seconds_all")) + f" ({r.get('api_threads_all')}t)",
            )
        )
    lines = [
        "larmorx runs `larmorx mri hmc` with mcflirt's arguments from `command.txt` (the raw BOLD, "
        "fMRIPrep's HMC boldref as `-reffile`, `-mats`). Matrices against mcflirt's, per volume "
        "(RMS deviation over a sphere of 80 mm about the field of view's centre, Jenkinson 1999; "
        "parameter differences about the same centre); mcflirt again in fMRIPrep's image, and "
        "FSL 6.0.7's mcflirt on this machine, against fMRIPrep's run:",
        "",
        table(
            (
                "Run",
                "Volumes",
                "RMS deviation median / 95th pct / max (mm)",
                "Median Δparam (mm / °)",
                "Max Δparam (mm / °)",
                "mcflirt re-run: identical volumes",
                "FSL 6.0.7 mcflirt: identical volumes",
            ),
            rows,
        ),
        "",
    ]
    if conf:
        lines += [
            "**What fMRIPrep derives from them**, computed by fMRIPrep's own code in its image "
            "(niworkflows' `MCFLIRT2ITK`, fMRIPrep's `FSLMotionParams`, `FramewiseDisplacement`, "
            "`FSLRMSDeviation`) from larmorx's matrices, against the confounds file fMRIPrep wrote "
            "(`desc-confounds_timeseries.tsv`) and its `desc-hmc_xfm.txt`. The same code on "
            "mcflirt's matrices reproduces fMRIPrep's file (last column). mcflirt's band is its own "
            "lowest FD correlation when its input is perturbed slightly "
            "([mri-hmc](mri-hmc.md), from the testdata's ds000005 HMC reference):",
            "",
            table(
                (
                    "Run",
                    "Median Δ motion params (mm / °)",
                    "Max Δ (mm / °)",
                    "FD r",
                    "Max FD (mm)",
                    "Mean FD fMRIPrep / larmorx (mm)",
                    "Max Δ RMSD (mm)",
                    "Max Δ ITK matrix entry (mm for translations)",
                    "Check: mcflirt's matrices",
                ),
                conf,
            ),
            "",
        ]
    lines += [
        "Times: fMRIPrep's node (its run), mcflirt re-run in fMRIPrep's image on this machine "
        "(one thread, as mcflirt runs), `larmorx mri hmc` with the same arguments (it also "
        "resamples and writes the series, as mcflirt does), and the Python API with "
        "`resample=False` (the matrices only, all fMRIPrep uses):",
        "",
        table(
            (
                "Run",
                "fMRIPrep",
                "mcflirt re-run",
                "larmorx CLI",
                "larmorx API",
                "larmorx API, all CPUs",
            ),
            times,
        ),
        "",
    ]
    return lines


def _resample_details(st: Stage, results: list[dict]) -> list[str]:
    rows = []
    for r in results:
        label = _node_label(r)
        if r.get("status") in ("skipped", "harness-error"):
            rows.append((label, r.get("status"), r.get("note", ""), "", "", "", "", ""))
            continue
        o = _outputs(r)[0]
        d = _diff(o["vs_fmriprep"])
        dv = _diff(o.get("vs_derivative"))
        ok, measure = assess_output(st, d)
        rows.append(
            (
                label,
                "×".join(str(x) for x in d.shape),
                d.summary(),
                "–" if dv is None else dv.summary(),
                ("pass" if ok else "**fail**") + f": {measure}",
                "same"
                if r.get("rerun_identical")
                else ("–" if "rerun_identical" not in r else "differs"),
                _sec(r.get("fmriprep_seconds")) + f" ({r.get('fmriprep_threads')}t)",
                _sec(r.get("container_seconds")),
                f"{_sec(r.get('larmorx_seconds'))} + {_sec(r.get('larmorx_write_seconds'))} "
                f"({r.get('larmorx_threads')}t)",
            )
        )
    first = results[0].get("inputs", {}) if results else {}
    return [
        "`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: "
        f"the raw BOLD, the target grid, the transforms (`{len(first.get('transforms') or [])}` "
        f"files), order {first.get('order')}, mode `{first.get('mode')}`, Jacobian "
        f"{first.get('jacobian')}, {first.get('output_dtype')} output. fMRIPrep's time includes "
        "writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's "
        "re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets "
        "it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes "
        "([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend "
        "on any thread count.",
        "",
        table(
            (
                "Run",
                "Shape",
                "larmorx vs fMRIPrep's node",
                "larmorx vs the derivative",
                "PLAN.md §11.3",
                "fMRIPrep re-run vs its run",
                "fMRIPrep",
                "Re-run",
                "larmorx",
            ),
            rows,
        ),
        "",
    ]


def workspace_path(path: Path) -> str:
    """``path`` as ``<workspace>/oracles/...`` when it lies under the workspace's oracles."""
    parts = path.parts
    if "oracles" in parts:
        i = len(parts) - 1 - parts[::-1].index("oracles")
        return "<workspace>/" + "/".join(parts[i:])
    return redact(str(path))


def write_record(s, payload: dict, out: Path, name: str) -> tuple[Path, Path]:
    from larmorx_validation import environment
    from larmorx_validation.stages.runners import container_versions

    out.mkdir(parents=True, exist_ok=True)
    versions = container_versions(s) if s.run.image is not None else {}
    env = payload.get("environment") or environment.describe(("nibabel", "scipy", "pandas"))
    out_text = "docs/validation" if out.name == "validation" else redact(str(out))
    command = (
        f"python -m larmorx_validation pipeline-stages --oracle {workspace_path(s.run.root)} "
        f"--out {out_text}"
    )
    md = out / f"{name}.md"
    md.write_text(render(s, payload, versions, env, command) + "\n", encoding="utf-8")
    js = out / f"{name}.json"
    js.write_text(
        json.dumps(
            _plain(
                {
                    "record": name,
                    "generated": time.strftime("%Y-%m-%d"),
                    "command": command,
                    "oracle": {
                        "run": s.run.name,
                        "fmriprep": s.run.version,
                        "image": f"nipreps/fmriprep@{s.run.digest}",
                        "arguments": s.run.arguments,
                        "tools": versions,
                    },
                    "stages": [
                        {
                            "id": st.id,
                            "group": st.group,
                            "title": st.title,
                            "tool": st.tool,
                            "threshold": st.threshold,
                            "absent": st.absent,
                        }
                        for st in STAGES
                    ],
                    **payload,
                }
            ),
            indent=1,
        )
        + "\n",
        encoding="utf-8",
    )
    return md, js


__all__ = ["assess_hmc", "assess_output", "render", "stage_summary", "write_record"]
