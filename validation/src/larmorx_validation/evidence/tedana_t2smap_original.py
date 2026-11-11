# SPDX-License-Identifier: Apache-2.0
"""tedana's ``t2smap``: the clean-room original's evidence (crate larmorx-tedana, Apache-2.0,
written from ``specs/t2smap.md``; through its own binding ``larmorx._core.tedana_t2smap_cleanroom``
until it is wired into ``larmorx tedana t2smap``), for the t2smap page beside tedana's and the
replica's evidence.

Written by the clean-room implementer: it reads no tedana source and none of the replica's files,
records, evidence or page. The replica (``larmorx-gpl``, from ``LARMORX_GPL_BIN`` or the
``PATH``) and tedana 26.0.3 (through the oracle-output cache, the launcher of the original's
suite) are run as black boxes.

:func:`extend` adds the original's part to a page document (its ``examples``, ``cases``,
``implementations``, ``tables`` and ``provenance``), as the replica-side relay places it on the
page; :func:`export` returns that part on its own (an otherwise empty page document), for its
checks. The part:

- a worked example: fMRIPrep 25.2.6's ``t2smap`` call on OpenNeuro ds003592 sub-280's rest run
  (three echoes, the call fMRIPrep makes, with ``--fittype curvefit``), run by tedana 26.0.3,
  the replica's default and the original in its reproduce mode at one thread and its default at
  eight and at one: the T2* map, the S0 map, the RMSE map and the adaptive mask on the run's
  dilated brain mask, the comparisons judged by the spec's ``check.py`` (§17.3's thresholds);
- every case of the original's record (``tedana-t2smap-original``) with its verdict, read from
  the record's measured and reported values;
- the table of fMRIPrep's calls, the random-testing trial with its triage, the speed and memory
  against tedana and the replica.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import sys
import tomllib
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "tedana-t2smap"
RECORD = "tedana-t2smap-original"
BENCH = "docs/benchmarks/tedana-t2smap.md"
BENCH_JSON = "docs/benchmarks/tedana-t2smap-original.json"
FINDINGS = "docs/findings/tedana-t2smap-original.md"
SPEC = "specs/t2smap.md"
#: The original's random-testing trial (``oracles/difftest/<TRIAL>``) and its triage (the
#: entries of ``DIFFTEST_TOOL`` in ``validation/difftest/triage.toml``).
TRIAL = "2026-11-11-t2smap-original-2"
DIFFTEST_TOOL = "tedana-t2smap-original"
TRIAGE = "validation/difftest/triage.toml"
#: The memory call's log (tests/parity/test_memory_peak.py, the original's test).
MEMORY_LOG = "logs/t2smap-cr-r3a2-memory.log"
LABEL = "LARMORX clean-room original"
CASE = "fmriprep-ds003592-curvefit-original"
CALL = "ds003592"
EXTRA = ["--fittype", "curvefit"]
THREADS = 8
#: The runs of the example: label, program, D13 mode, threads, the panel's subtitle.
RUNS = [
    ("tedana", "tedana", None, 1, "tedana 26.0.3 (its newest environment), through the "
     "oracle-output cache"),
    ("replica", "replica", "fix", None, "larmorx-gpl's replica, default mode (a black box)"),
    ("original-reproduce", "original", "reproduce", 1, "reproduce mode, 1 thread"),
    ("original", "original", "fix", THREADS, f"default mode (D13 corrections), {THREADS} threads"),
    ("original-1", "original", "fix", 1, "default mode, 1 thread"),
]  # fmt: skip
#: The outputs shown: id, file name (BIDS convention), label, unit.
OUTPUTS = [
    ("t2s", "T2starmap.nii.gz", "T2* map (s)", "s"),
    ("s0", "S0map.nii.gz", "S0 map", ""),
    ("rmse", "desc-rmse_statmap.nii.gz", "RMSE map", ""),
    ("am", "desc-adaptiveGoodSignal_mask.nii.gz", "adaptive mask (good echoes)", "echoes"),
]
#: The original through its own binding, as a program (until the wiring).
RUNNER = (
    "import os, sys\n"
    "from larmorx import _core\n"
    "code, out, err = _core.tedana_t2smap_cleanroom(sys.argv[1:], list(os.environ.items()), "
    "os.getcwd())\n"
    "sys.stdout.write(out)\nsys.stderr.write(err)\nsys.exit(code)\n"
)
_NOTICE = re.compile(r"^larmorx(?:-gpl)?: upstream defect (\S+) \((\w+)\) (corrected|reproduced)")


def _suite():
    from larmorx_validation.parity import tedana_t2smap_original as suite

    return suite


def _settings(mode: str | None) -> dict[str, str]:
    """The spec's reference setup (§17.1), and the D13 mode."""
    suite = _suite()
    blackbox, _, _, _ = suite.tools()
    full = blackbox.environment()
    keep = ("COLUMNS", "TQDM_DISABLE", "OPENBLAS_NUM_THREADS", "OPENBLAS_CORETYPE")
    env = {k: full[k] for k in keep if k in full}
    env.update({k: v for k, v in full.items() if k.startswith("NPY_")})
    if mode is not None:
        env["LARMORX_UPSTREAM_DEFECTS"] = mode
    return env


def _args() -> tuple[list[str], list[Path], Path]:
    suite = _suite()
    _, _, _, fmriprep = suite.tools()
    run, task, tes = fmriprep.CALLS[CALL]
    if not (fmriprep.RUNS / run).is_dir():
        raise FileNotFoundError(f"the oracle run {run}")
    echoes, mask = fmriprep.inputs(run, task)
    args = ["-d", *map(str, echoes), "-e", *tes, "--mask", str(mask), *EXTRA]
    return args, echoes, mask


def _compute(e: fcache.Entry) -> None:
    suite = _suite()
    args, echoes, mask = _args()
    for i, p in enumerate(echoes, 1):
        e.input(f"echo {i} (fMRIPrep's boldref-space series)", p)
    e.input("fMRIPrep's dilated brain mask (t2smap's --mask)", mask)
    unset = [k for k in os.environ if k.startswith(("NPY_", "OPENBLAS_", "OMP_"))]
    notices: dict[str, list[str]] = {}
    for label, program, mode, n, _sub in RUNS:
        d = e.dir / label
        if d.is_dir():
            shutil.rmtree(d)
        d.mkdir()
        words = [*args] + (["--n-threads", str(n)] if program == "original" else [])
        words += ["--out-dir", "out"]
        if program == "tedana":
            blackbox, _, _, _ = suite.tools()
            env = _settings(None)
            code, _out, err = suite.run_tedana(words, d, blackbox.environment())
            run = fcache.Run(
                label, [str(suite.LAUNCHER), *words], {**env, "cache": "oracle-output"}
            )
            run.exit, run.stderr_tail = code, err[-2000:]
            e.runs.append(run)
            (e.dir / f"{label}.log").write_text(f"exit {code}\n--- stderr\n{err}", encoding="utf-8")
            if code != 0:
                raise RuntimeError(f"tedana exited with {code}: {err[-1000:]}")
            continue
        if program == "replica":
            replica = suite.replica_program()
            if replica is None:
                raise FileNotFoundError("larmorx-gpl (LARMORX_GPL_BIN), the replica's program")
            argv = [*replica, *words]
        else:
            argv = [sys.executable, "-c", RUNNER, *words]
        r = e.run(label, argv, env=_settings(mode), unset=unset, cwd=d)
        del r
        log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
        notices[label] = sorted({m.group(1) for x in log.splitlines() if (m := _NOTICE.match(x))})
    e.extra.update(
        call=CALL,
        command="t2smap " + " ".join(core.short_path(w) for w in args),
        notices=notices,
        versions={
            "tedana": "tedana 26.0.3 (oracles/python-newest, the launcher of the spec's probe; "
            "through the oracle-output cache)",
            "replica": "larmorx-gpl 0.1.0rc1 (PyPI), its program as a black box",
            "larmorx": "the clean-room original (crate larmorx-tedana) through "
            "larmorx._core.tedana_t2smap_cleanroom",
        },
        threads={"tedana": ["1"], "original": [str(THREADS), "1"]},
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep 25.2.6's t2smap call on ds003592 sub-280's rest run with --fittype curvefit: "
        "tedana 26.0.3, the replica's default and the clean-room original in both modes",
        _compute,
    )


# ------------------------------------------------------------------------------ the example


def _load(path: Path) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(str(path)).dataobj)


def _check(ref: Path, other: Path) -> tuple[bool, str, tuple[int, int] | None]:
    """The spec's check.py (§17.3) between two runs' folders, through the original's suite."""
    suite = _suite()
    ok, detail = suite.judge_values(ref, other, False)
    counts = suite.value_counts(detail) if hasattr(suite, "value_counts") else None
    return ok, detail, counts


def _share(detail: str) -> str:
    m = re.search(r"\(b\)/\(c\) share ([0-9.]+%)", detail)
    return m.group(1) if m else "0%"


def _example(ctx: core.Context) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import ants_panel
    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    d = e.dir
    inputs = {k.split(" (")[0]: Path(v.split(" (sha256 ", 1)[0]) for k, v in info["inputs"].items()}
    mask_path = inputs["fMRIPrep's dilated brain mask"]
    echo1 = inputs["echo 1"]
    img = nib.load(str(echo1))
    brain = _load(mask_path) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    first = np.asanyarray(img.dataobj[..., 0]).astype(np.float32)
    ex.add(exm.Panel("echo1", "input", "Echo 1, the first volume",
                     "fMRIPrep's boldref-space series of the first echo (float32), one of the "
                     "three echoes t2smap combines", {"all": first}))  # fmt: skip
    texts = {
        "tedana": ("original", "tedana 26.0.3, t2smap", RUNS[0][4]),
        "replica": ("larmorx", "larmorx replica (larmorx-gpl)", "default mode, a black box"),
        "original-reproduce": ("larmorx", LABEL, RUNS[2][4]),
        "original": ("larmorx", LABEL, RUNS[3][4]),
        "original-1": ("larmorx", LABEL, RUNS[4][4]),
    }
    for lab, (role, label, sub) in texts.items():
        full = {oid: _load(d / lab / "out" / name) for oid, name, _l, _u in OUTPUTS}
        ex.add(exm.Panel(lab, role, label, sub, full))
    dd = common.d
    diffs = [
        dd("orepro-tedana", "original-reproduce", "tedana", "main",
           "clean-room original (reproduce) − tedana 26.0.3", common.ROLE_LX),
        dd("original-tedana", "original", "tedana", "main",
           "clean-room original (default) − tedana 26.0.3", common.ROLE_LX),
        dd("replica-tedana", "replica", "tedana", "main", "replica (default) − tedana 26.0.3",
           common.ROLE_LX),
        dd("original-replica", "original", "replica", "check",
           "clean-room original − replica, both default", common.ROLE_SELF),
        dd("original1-original", "original-1", "original", "check",
           f"clean-room original, 1 thread − {THREADS} threads", common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": oid, "label": label, "unit": unit} for oid, _n, label, unit in OUTPUTS]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = info.get("notices", {})
    ok_a, det_a, cnt_a = _check(d / "tedana", d / "original-reproduce")
    ok_b, det_b, cnt_b = _check(d / "replica", d / "original")

    def counted(c: tuple[int, int] | None) -> str:
        return f"{c[0]:,} of {c[1]:,} compared values identical" if c else "values compared"

    for x in diffs:
        i = x["id"]
        if i == "orepro-tedana":
            x["verdict"] = (
                "bit-identical" if same[i] else ("within-variability" if ok_a else "open")
            )
            x["reason"] = common.clip(
                f"specs/t2smap.md §17.3 by its check.py: {counted(cnt_a)}; the curve fit's "
                f"(b)/(c) share {_share(det_a)} of the fit voxels (the original's own trust-region "
                "solver, its log-linear fit in closed form: the fitted numbers within the "
                "thresholds, not bit for bit, decided 2026-11-11)",
                420,
            )
            x["links"] = [{"label": "§17.3", "href": SPEC}]
        elif i in ("original-tedana", "replica-tedana"):
            ids = named.get("original" if i == "original-tedana" else "replica", [])
            x["verdict"] = "bit-identical" if same[i] else "d13"
            x["reason"] = common.clip(
                "The default's corrections, each named by its notice: "
                + (", ".join(ids) if ids else "none named")
                + " (TED-05 caps the full T2* map as the limited one; NP-03 and GLIBC-07 take "
                "the correctly rounded logarithm and exponential).",
                400,
            )
        elif i == "original-replica":
            x["verdict"] = (
                "bit-identical" if same[i] else ("within-variability" if ok_b else "open")
            )
            x["reason"] = common.clip(
                f"The two implementations' defaults by §17.3 (check.py): {counted(cnt_b)}; the "
                f"(b)/(c) share {_share(det_b)}.",
                400,
            )
        else:
            x["verdict"], x["reason"] = (
                ("bit-identical", f"The same bits at 1 and {THREADS} threads.") if same[i]
                else ("open", f"Values differ between 1 and {THREADS} threads."))  # fmt: skip
    at, _k = common.largest_in_slice(ex, "original-reproduce", "tedana", "t2s", "axial")
    default = {"output": "t2s", "plane": "axial", "voxel": at,
               "panels": ["echo1", "tedana", "original-reproduce", "original"],
               "diffs": ["orepro-tedana", "original-tedana"],
               "compare": {"a": "tedana", "b": "original-reproduce"}}  # fmt: skip
    out = common.finish(ctx, ex, diffs, outputs, default=default, window_panel="tedana",
                        caption="the T2* map in seconds; each difference on its own symmetric "
                        "scale.")  # fmt: skip
    ds = ants_panel.data("ds003592")
    return {
        "id": CASE,
        "title": "fMRIPrep's multi-echo call: the clean-room original beside tedana and the "
        "replica",
        "summary": (
            "fMRIPrep 25.2.6's t2smap call on OpenNeuro ds003592 sub-280's rest run (three "
            "echoes), with the monoexponential curve fit, given to tedana 26.0.3, to the "
            "replica's default and to LARMORX's clean-room original. In its reproduce mode the "
            "original holds tedana's maps within specs/t2smap.md §17.3's thresholds (its own "
            "solvers, not tedana's bits); by default it corrects tedana's defects, as the "
            "replica's default does, and agrees with it by the same thresholds."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{ds['dataset']}, {ds['subject']}, task rest "
                f"({ds['licence']}); shown inside fMRIPrep's dilated brain mask, dilated by one "
                "voxel",
            },
            {"label": "Node", "text": "fMRIPrep 25.2.6, `bold_t2smap_wf` (its echoes and mask)"},
            {"label": "Command", "text": f"`{info['command']}`"},
            {
                "label": "Programs",
                "text": "tedana 26.0.3's t2smap (through the oracle-output "
                "cache); larmorx-gpl 0.1.0rc1's replica (a black box); LARMORX's clean-room original "
                f"(crate larmorx-tedana, experimental) from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, tuple(grid.shape))},
        ],
        **out,
        "chain": None,
        "metrics_note": "Δ is A − B over every value of the files (the whole grid); the verdicts "
        "of the comparisons with tedana and between the defaults are check.py's (§17.3).",
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [ds]),
    }


# -------------------------------------------------------------------------------- the cases


def _reported(r: dict, name: str):
    return (r.get("reported") or {}).get(name)


def case_verdict(r: dict) -> tuple[str, str]:
    """A record case's verdict from its outcome, exit statuses and reported values."""
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    measured = r.get("measured") or {}
    if r["category"] == "threads":
        return (
            "bit-identical",
            "every output the same at 1, 4 and 8 threads (LARMORX against itself)",
        )
    exits = [x for k, x in measured.items() if k.startswith("(A)") and k.endswith("exit status")]
    if exits and all(x not in (0, None) for x in exits):
        return "both-reject", (
            f"tedana and the original's reproduce stop the same way (exit status {exits[0]}, the "
            "same messages and error); the default against the replica's the same"
        )
    same = [
        x for k, x in (r.get("reported") or {}).items() if "(A)" in k and k.endswith("identical")
    ]
    share = [x for k, x in (r.get("reported") or {}).items() if "(A)" in k and "share" in k]
    if same and all(x == "every file" or (isinstance(x, str) and _all(x)) for x in same):
        return "bit-identical", (
            "tedana's values bit for bit, its messages, files and exit status; the default "
            "against the replica's the same by §17.3"
        )
    if not same:
        return "bit-identical", "the same exit status, output and messages (no file to compare)"
    words = ", ".join(str(x) for x in same)
    return "within-variability", common.clip(
        f"specs/t2smap.md §17.3 by its check.py: {words} values identical"
        + (f"; the curve fit's (b)/(c) share {share[0]} %" if share else "")
        + "; exit status, files and messages tedana's",
        400,
    )


def _all(text: str) -> bool:
    m = re.fullmatch(r"(\d+)/(\d+)", text)
    return bool(m) and m.group(1) == m.group(2)


def _add_cases(ctx: core.Context, doc: dict) -> records.Record:
    rec = records.load(ctx.repo, RECORD)
    suite = _suite()
    defs = {c.id: c for c in suite.cases("full")}
    have = {c["id"] for c in doc["cases"]}
    for r in rec.results:
        v, why = case_verdict(r)
        cid = f"original/{r['case']}"
        if cid in have:
            raise ValueError(f"case id {cid} taken")
        c = defs.get(r["case"])
        entry_ = {"id": cid, "category": r["category"],
                  "description": c.description if c else r["case"],
                  "command": f"t2smap {c.description}" if c and c.category == "probe" else "",
                  "results": {"original": {"outcome": r["status"], "verdict": v,
                                           "checks": f"{r['checks_passed']}/{r['checks_total']}",
                                           "reason": why}}}  # fmt: skip
        if r["case"] == "ds003592-curvefit":
            entry_["example"] = CASE
        doc["cases"].append(entry_)
    doc["cases_total"] = len(doc["cases"])
    doc["cases_per_impl"] = True
    return rec


def _fmriprep_table(rec: records.Record) -> dict:
    rows = []
    for r in rec.results:
        if r["category"] != "fmriprep":
            continue
        rp = r.get("reported") or {}
        verdict = "within-variability" if r["status"] == "pass" else "open"
        rows.append({"head": r["case"], "verdict": verdict, "cells": [
            common.cell(rp.get("(A) values identical", "–")),
            common.cell(rp.get("(A) curve fit (b)/(c) share (%)")),
            common.cell(rp.get("(B) values identical", "–")),
            common.cell(rp.get("(B) curve fit (b)/(c) share (%)")),
            common.cell(f"{r['checks_passed']}/{r['checks_total']}")]})  # fmt: skip
    return common.table(
        "fmriprep-calls-original",
        "fMRIPrep's multi-echo calls: the clean-room original against tedana 26.0.3 (A, its "
        "reproduce mode) and against the replica (B, both defaults)",
        ["Call", "(A) values identical", {"label": "(A) (b)/(c) share (%)", "num": True},
         "(B) values identical", {"label": "(B) (b)/(c) share (%)", "num": True}, "checks"],
        rows,
        note="specs/t2smap.md §17.3's thresholds by its check.py; (b)/(c): the curve-fit voxels "
        "whose fit the other run's is no better than (b), or whose start lies on the feasibility "
        "boundary (c), at most 3 % allowed. The log-linear call's values are tedana's bit for bit "
        "but for one float32 unit in the combined series.",
    )  # fmt: skip


# ----------------------------------------------------------------- random testing, speed


def _trial(ctx: core.Context) -> tuple[list[dict], list[dict]]:
    night = common.oracles() / "difftest" / TRIAL
    summary = json.loads((night / "summary.json").read_text(encoding="utf-8"))
    tool = summary["tools"][DIFFTEST_TOOL]
    n_cases = int(tool["cases"])
    n_diff = sum(int(x.get("cases", 0)) for x in tool.get("differences", []))
    entries = [
        e
        for e in tomllib.loads((ctx.repo / TRIAGE).read_text(encoding="utf-8")).get(
            "difference", []
        )
        if e.get("tool") == DIFFTEST_TOOL and TRIAL in e.get("explanation", "")
    ]
    rows = [{"head": f"{n_cases:,} random cases (30 minutes, seed {summary['seed']})",
             "verdict": "within-variability" if not n_diff else "open",
             "cells": [common.cell(n_cases - n_diff, "count"), common.cell(n_diff, "count"),
                       common.cell(f"{len(entries)} triage entries")]}]  # fmt: skip
    for e in entries:
        rows.append({"head": ", ".join(e["signature"]), "sub": common.clip(e["explanation"], 400),
                     "verdict": {
                    "fixed": "within-variability",
                    "harness": "within-variability",
                    "register": "d13",
                    "explained": "divergence",
                }.get(
                         e["status"], "open"),
                     "cells": [common.cell("–"), common.cell("–"), common.cell(e["status"])]})  # fmt: skip
    table = common.table(
        "difftest-original",
        "Random testing: the clean-room original against tedana 26.0.3 and the replica's default",
        ["Trial", {"label": "agree", "num": True}, {"label": "differ", "num": True}, "triage"],
        rows,
        note="Each case one command line on multi-echo files drawn from a recipe (two to six "
        "echoes, every image form and data type, every option); (A) the original's reproduce "
        "against tedana, (B) its default against the replica's, both by the original's suite and "
        "specs/t2smap.md §17.3; every difference triaged in validation/difftest/triage.toml.",
    )
    source = [{"night": TRIAL, "tool": DIFFTEST_TOOL, "cases": n_cases, "differ": n_diff}]
    return [table], source


def _memory(ctx: core.Context) -> list[tuple[str, float, float]]:
    path = common.oracles() / MEMORY_LOG
    if not path.is_file():
        return []
    rx = re.compile(r"t2smap ds003592 \((\w+), (\d+) thread\(s\)\): the original ([0-9.]+) MiB, "
                    r"tedana ([0-9.]+) MiB")  # fmt: skip
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = rx.search(line)
        if m:
            out.append((f"{m.group(1)}, {m.group(2)} thread(s)", float(m.group(3)),
                        float(m.group(4))))  # fmt: skip
    return out


def _speed(ctx: core.Context) -> list[dict]:
    bench = json.loads((ctx.repo / BENCH_JSON).read_text(encoding="utf-8"))
    rows = []
    for call, t in bench["calls"].items():
        o1, o8 = t.get("original fix (1)"), t.get("original fix (8)")
        rows.append({"head": call, "verdict": "bit-identical" if all(
            v for k, v in t.items() if k.endswith("identical to (1)")) else "open", "cells": [
            common.cell(t.get("tedana")), common.cell(t.get("replica default (1)")),
            common.cell(t.get("replica default (8)")), common.cell(t.get("original reproduce (1)")),
            common.cell(o1), common.cell(o8),
            common.cell(t["tedana"] / o1 if o1 else None),
            common.cell(t["tedana"] / o8 if o8 else None)]})  # fmt: skip
    speed = common.table(
        "speed-original",
        "Speed: wall time (s) of fMRIPrep's multi-echo calls, best of two runs",
        ["Call", {"label": "tedana", "num": True}, {"label": "replica, 1 thread", "num": True},
         {"label": "replica, 8", "num": True}, {"label": "original reproduce, 1", "num": True},
         {"label": "original default, 1", "num": True}, {"label": "original default, 8",
                                                          "num": True},
         {"label": "× tedana's speed, 1", "num": True},
         {"label": "× tedana's speed, 8", "num": True}],
        rows,
        note=f"{bench['about']} {bench['machine'].get('cpus', '?')} logical CPUs, a machine "
        "shared with other work; the verdict: the original's files the same at 1 and 8 threads "
        "in both modes. The report: " + BENCH + ".",
    )  # fmt: skip
    out = [speed]
    mem = _memory(ctx)
    if mem:
        out.append(common.table(
            "memory-original",
            "Peak memory of fMRIPrep's ds003592 curve-fit call (MiB)",
            ["Run", {"label": "the original", "num": True}, {"label": "tedana", "num": True},
             {"label": "allowed", "num": True}],
            [{"head": f"the original, {w}", "verdict": "within-variability" if o <= 1.1 * t + 8
              else "open", "cells": [common.cell(o), common.cell(t),
                                     common.cell(1.1 * t + 8)]} for w, o, t in mem],
            note="tests/parity/test_memory_peak.py (the original's test): at most 10 % above "
            "tedana's peak plus 8 MiB, tedana at one thread through the oracle-output cache.",
        ))  # fmt: skip
    return out


# ----------------------------------------------------------------------------------- export


def extend(ctx: core.Context, doc: dict) -> dict:
    """The original's part added to a page document ``doc`` (its lists and provenance)."""
    doc.setdefault("examples", []).append(_example(ctx))
    doc.setdefault("cases", [])
    rec = _add_cases(ctx, doc)
    doc.setdefault("implementations", []).append(common.implementation(
        doc["cases"], "original",
        "LARMORX clean-room original, against tedana 26.0.3 (its reproduce mode; full tier)",
        "original", f"docs/validation/{RECORD}.md", "experimental"))  # fmt: skip
    doc.setdefault("tables", [])
    doc["tables"].append(_fmriprep_table(rec))
    trial, trial_source = _trial(ctx)
    doc["tables"] += trial
    doc["tables"] += _speed(ctx)
    prov = doc.setdefault("provenance", {})
    prov["records"] = [*prov.get("records", []), *core.record_source(rec)]
    prov["benchmarks"] = [*prov.get("benchmarks", []), {
        "path": BENCH_JSON,
        "sha256": hashlib.sha256((ctx.repo / BENCH_JSON).read_bytes()).hexdigest()}]  # fmt: skip
    cache_prov = prov.get("cache", [])
    if isinstance(cache_prov, dict):
        cache_prov = [cache_prov]
    prov["cache"] = [*cache_prov, fcache.provenance(entry(ctx))]
    prov["difftest"] = [*prov.get("difftest", []), *trial_source]
    prov["original_exporter"] = core.tree_hash(
        ctx.repo, "validation/src/larmorx_validation/evidence/tedana_t2smap_original.py"
    )
    return doc


def export(ctx: core.Context) -> dict:
    """The original's part on its own (a page document holding only it)."""
    return extend(ctx, {"tool": TOOL, "examples": [], "cases": [], "implementations": [],
                        "tables": [], "provenance": {}})  # fmt: skip


if __name__ == "__main__":  # the example's cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir)
