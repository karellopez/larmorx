# SPDX-License-Identifier: Apache-2.0
"""The clean-room originals of fslmaths and fslstats on their evidence pages (``larmorx fsl
fslmaths`` and ``fsl fslstats`` with ``LARMORX_IMPLEMENTATION=original``, crate larmorx-fsl,
Apache-2.0, written from ``specs/fslmaths.md`` and ``specs/fslstats.md``): the same real calls
the replicas' examples show (fMRIPrep's combine_masks, the goodvoxels mask's fourteen fslmaths
and four fslstats calls, one fslmaths call per operation family, glibc's double functions,
every fslstats option on a real image), run by the original in its reproduce mode at 14
threads and at one and in its default mode, against FSL's runs of the replicas' cache entries
(no FSL run of its own: FSL's outputs are those entries'); every case of the originals'
records (``fsl-maths``, ``fsl-stats``) with its verdict, read from the record's measured values,
and the original's default mode on each case (no FSL run); random testing's trial of the
originals with its triage; the speed and memory of the benchmark report.

The original runs as the parity suites run it (``parity.fsl_maths.original_command``: a
``larmorx`` binary named by ``LARMORX_MATHS_BIN``, else this Python's ``larmorx`` package).
"""

from __future__ import annotations

import hashlib
import json
import re
import tempfile
import types
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

#: The originals' records, per program.
RECORD = {"fslmaths": "fsl-maths", "fslstats": "fsl-stats"}
#: The originals' random-testing trial (both programs) and its triage (findings 24 to 30).
TRIAL = "2026-11-03-fsl-maths-original"
TRIAGE = "validation/investigations/cleanroom-maths/trial_triage.json"
FINDINGS = "docs/findings/fsl-maths-original.md"
REGISTER = "docs/findings/upstream-defects.md"
BENCH = "docs/benchmarks/fsl-maths.md"
#: The original's runs of an example: label, D13 mode, threads (its result is the same bits at
#: any thread count).
RUNS = [("original", "reproduce", 14), ("original-1", "reproduce", 1), ("original-fix", "fix", 14)]
PANELS = [
    ("original", "larmorx", "LARMORX clean-room original", "reproduce mode, 14 threads"),
    ("original-1", "larmorx", "LARMORX clean-room original", "reproduce mode, 1 thread"),
    ("original-fix", "larmorx", "LARMORX clean-room original",
     "default mode (D13 corrections), 14 threads"),
]  # fmt: skip
#: specs/fslmaths.md §21's metric of the sums: the largest difference relative to FSL's largest
#: magnitude, and the share of voxels within 4 units in the last place.
SUM_RELATIVE, SUM_SHARE, SUM_ULPS = 1e-5, 0.99, 4


def _suite(program: str):
    if program == "fslmaths":
        from larmorx_validation.parity import fsl_maths as s
    else:
        from larmorx_validation.parity import fsl_stats as s
    return s


def versions() -> dict[str, str]:
    import larmorx
    from larmorx_validation.evidence import fsl_maths as fm

    return {
        "fsl": fm.versions()["fsl"],
        "larmorx": f"larmorx {larmorx.__version__} (the clean-room originals, crate larmorx-fsl, "
        "Apache-2.0)",
    }


# --------------------------------------------------------------------- running the original


def run(e: fcache.Entry, label: str, mode: str, n: int, args: list[str], put,
        program: str = "fslmaths") -> None:  # fmt: skip
    """The original on ``args`` in ``mode`` at ``n`` threads, in ``<entry>/<label>`` (it writes
    where FSL does: its working directory); its standard output into ``<label>.stdout.txt``, the
    register entries its notices name kept; the inputs removed after."""
    from larmorx_validation.evidence import fsl_maths as fm
    from larmorx_validation.parity import defects
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put(cwd)
    given = list(cwd.iterdir())
    env = {**_environment({}), defects.ENV: mode, "OMP_NUM_THREADS": str(n),
           "LARMORX_IMPLEMENTATION": "original"}  # fmt: skip
    argv = [*_suite(program).original_command(), "fsl", program, *args]
    e.run(label, argv, env=env, unset=fm.unset_env(), cwd=cwd)
    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    stdout, stderr = log.split("--- stdout\n", 1)[1].split("\n--- stderr\n", 1)
    e.extra.setdefault("named", {})[label] = sorted(set(defects.notices_in(stderr)))
    (e.dir / f"{label}.stdout.txt").write_text(stdout, encoding="utf-8")
    for p in given:
        if p.is_file():
            p.unlink()


def _all_runs(e: fcache.Entry, prefix: str, args: list[str], put, program: str) -> None:
    for label, mode, n in RUNS:
        run(e, f"{label}{prefix}", mode, n, args, put, program)


def _noop(d: Path) -> None:
    del d


# ------------------------------------------------------------------- the examples' entries


def _compute_combine(ctx: core.Context, e: fcache.Entry) -> None:
    del ctx
    from larmorx_validation.evidence import fsl_maths as fm

    node = fm._combine_node()
    args = fm._suite().fmriprep_args(fm.RUN, node)
    _all_runs(e, "", args, _noop, "fslmaths")
    e.finish(versions=versions(), args=args, settings="fMRIPrep 25.2.6's combine_masks node "
             "of ds005040's head-motion reference, its masks read from the run's working "
             "directory: LARMORX's clean-room original in its reproduce mode at 14 threads and at "
             "one and in its default mode; FSL 6.0.7.23 and the replica are the entry "
             "fsl-maths/fmriprep-combine-masks's runs of the same call")  # fmt: skip


def _compute_goodvoxels(ctx: core.Context, e: fcache.Entry) -> None:
    del ctx
    from larmorx_validation.evidence import fsl_maths as fm

    fs = fm._suite()
    base = fm.chain()
    values = json.loads((base / "values.json").read_text(encoding="utf-8"))
    files = fm._goodvoxels_files(base)
    calls = {}
    for node, prog, template in fs.GOODVOXELS_STEPS:
        if prog != "fslmaths":
            continue

        def put(d: Path, template=template) -> None:
            fs._goodvoxels_args(template, files, values, d)

        with tempfile.TemporaryDirectory(prefix="lx-maths-ev-orig-gv_") as t:
            args = [*fs._goodvoxels_args(template, files, values, Path(t)), f"{node}.nii.gz"]
        calls[node] = args
        _all_runs(e, f"-{node}", args, put, "fslmaths")
    e.finish(versions=versions(), calls=calls, settings="fMRIPrep 25.2.6's goodvoxels "
             "workflow's fourteen fslmaths calls on ds005040's BOLD series in T1w space, each on "
             "FSL's outputs of the calls before it (the replica record's chain; the ribbon a "
             "stand-in: the segmentation's grey matter): LARMORX's clean-room original in its "
             "reproduce mode at 14 threads and at one and in its default mode; FSL 6.0.7.23 and "
             "the replica are the entry fsl-maths/fmriprep-goodvoxels's runs of the same calls")  # fmt: skip


def _compute_families(ctx: core.Context, e: fcache.Entry) -> None:
    del ctx
    from larmorx_validation.evidence import fsl_maths as fm

    calls = {}
    for oid in fm.FAMILIES:
        args, used = fm.family_args(oid)
        calls[oid] = args
        _all_runs(e, f"-{oid}", args, fm._copier(used), "fslmaths")
    e.finish(versions=versions(), calls=calls, settings="One fslmaths call per operation family "
             "on ds005040's BOLD series in T1w space, FSL's -Tmean and -Tstd of it and fMRIPrep's "
             "brain mask: LARMORX's clean-room original in its reproduce mode at 14 threads and at "
             "one and in its default mode; FSL 6.0.7.23 and the replica are the entry "
             "fsl-maths/families's runs of the same calls")  # fmt: skip


def _compute_glibc(ctx: core.Context, e: fcache.Entry) -> None:
    del ctx
    from larmorx_validation.evidence import fsl_maths as fm

    mean = fm._family_files()["mean"]
    put = fm._copier({"mean.nii.gz": mean})
    for rid in fm.GLIBC_CALLS:
        _all_runs(e, f"-{rid}", fm.glibc_args(rid), put, "fslmaths")
    e.finish(versions=versions(), settings="fslmaths' double-precision functions (-dt double ... "
             "-odt double) on FSL's -Tmean of ds005040's BOLD series in T1w space, scaled into each "
             "function's range: LARMORX's clean-room original in its reproduce mode at 14 threads "
             "and at one and in its default mode; FSL 6.0.7.23 and the replica are the entry "
             "fsl-maths/glibc-double's runs of the same calls")  # fmt: skip


def _compute_stats_goodvoxels(ctx: core.Context, e: fcache.Entry) -> None:
    del ctx
    from larmorx_validation.evidence import fsl_maths as fm

    base = fm.chain()
    printed = {}
    for node, prog, template in fm._suite().GOODVOXELS_STEPS:
        if prog != "fslstats":
            continue
        src = template[0][1:-1]
        image = base / src / f"{src}.nii.gz"
        args = [image.name, *template[1:]]
        _all_runs(e, f"-{node}", args, fm._copier({image.name: image}), "fslstats")
        printed[node] = {label: (e.dir / f"{label}-{node}.stdout.txt").read_text(encoding="utf-8")
                         for label, _m, _n in RUNS}  # fmt: skip
    e.finish(versions=versions(), printed=printed, settings="fMRIPrep 25.2.6's goodvoxels "
             "workflow's four fslstats nodes on FSL's outputs of the nodes before them (the "
             "replica record's chain on ds005040, the ribbon a stand-in): LARMORX's clean-room "
             "original in its reproduce mode at 14 threads and at one and in its default mode; "
             "FSL 6.0.7.23 and the replica are the entry fsl-stats/fmriprep-goodvoxels's runs")  # fmt: skip


def _compute_stats_options(ctx: core.Context, e: fcache.Entry) -> None:
    from larmorx_validation.evidence import fsl_maths as fm
    from larmorx_validation.evidence import fsl_stats as fst

    files = fst._files(fst.options_entry(ctx))
    printed = {}
    for rid in fst.CALLS:
        args, used = fst.call_args(rid, files)
        _all_runs(e, f"-{rid}", args, fm._copier(used), "fslstats")
        printed[rid] = {label: (e.dir / f"{label}-{rid}.stdout.txt").read_text(encoding="utf-8")
                        for label, _m, _n in RUNS}  # fmt: skip
    e.finish(versions=versions(), printed=printed, settings="fslstats' options on the real "
             "images of the entry fsl-stats/options-real (FSL's -Tmean and -Tstd of ds005040's "
             "BOLD series in T1w space, the series, fMRIPrep's brain mask, a label image of its "
             "segmentation, fMRIPrep's preprocessed T1w): LARMORX's clean-room original in its "
             "reproduce mode at 14 threads and at one and in its default mode; FSL 6.0.7.23 and "
             "the replica are that entry's runs of the same calls")  # fmt: skip


_COMPUTE = {
    ("fsl-maths", "combine"): ("fmriprep-combine-masks", _compute_combine),
    ("fsl-maths", "goodvoxels"): ("fmriprep-goodvoxels", _compute_goodvoxels),
    ("fsl-maths", "families"): ("families", _compute_families),
    ("fsl-maths", "glibc"): ("glibc-double", _compute_glibc),
    ("fsl-stats", "goodvoxels"): ("fmriprep-goodvoxels", _compute_stats_goodvoxels),
    ("fsl-stats", "options"): ("options-real", _compute_stats_options),
}


def entry(ctx: core.Context, tool: str, which: str) -> fcache.Entry:
    """The original's runs of the calls of the replica's entry ``<tool>/<name>`` (``which``)."""
    name, compute = _COMPUTE[(tool, which)]

    def go(e: fcache.Entry) -> None:
        compute(ctx, e)

    return fcache.entry(
        ctx.figures_context(), tool, f"original-{name}",
        f"The calls of {tool}/{name}: LARMORX's clean-room original, reproduce mode at 14 "
        "threads and at one, the default mode", go,
    )  # fmt: skip


# ----------------------------------------------------------------- the examples' verdicts


def sums_metric(fsl: np.ndarray, ours: np.ndarray) -> dict:
    """§21's metric of the sums over every voxel: the largest difference over FSL's largest
    magnitude (finite voxels), the share within 4 units in the last place, the non-finite
    voxels identical."""
    from larmorx_validation.parity.fsl_maths import _bits, _ulps

    x, y = np.asarray(fsl).ravel(), np.asarray(ours).ravel()
    if x.dtype != y.dtype or x.dtype.kind != "f":
        x64, y64 = x.astype(np.float64), y.astype(np.float64)
        d = np.abs(x64 - y64)
        scale = float(np.abs(x64).max()) or 1.0
        return {"relative": float(d.max()) / scale, "share": float(np.mean(d == 0)),
                "nonfinite_same": True, "ulps": None}  # fmt: skip
    fx, fy = np.isfinite(x), np.isfinite(y)
    same = bool(np.array_equal(fx, fy) and np.array_equal(_bits(x[~fx]), _bits(y[~fy])))
    both = fx & fy
    scale = float(np.abs(x[both]).max()) if both.any() else 0.0
    worst = float(np.abs(x[both].astype(np.float64) - y[both].astype(np.float64)).max())
    ulp = _ulps(x, y)
    return {
        "relative": worst / scale if scale else worst,
        "share": float(np.count_nonzero(ulp <= SUM_ULPS)) / max(1, ulp.size),
        "nonfinite_same": same,
        "ulps": int(ulp[both].max()) if both.any() else 0,
    }


def within(m: dict) -> bool:
    return m["nonfinite_same"] and m["relative"] <= SUM_RELATIVE and m["share"] >= SUM_SHARE


def metric_words(label: str, m: dict) -> str:
    return (
        f"{label}: max |Δ| {m['relative']:.2g} of FSL's largest value (§21: at most 1e-5), "
        f"{m['share']:.1%} of the voxels within 4 units in the last place (§21: at least 99 %)"
    )


#: What the original does differently on an output whose values are not FSL's (example
#: outputs' operations), in plain words.
CAUSES = {
    "fft": "FSL convolves this kernel by its FFT, the original by a direct sum in single "
    "precision (§10.2's arithmetic of FSL's direct path): their roundings differ (findings 8 "
    "and 31; spec question 14)",
    "sums": "the original adds in its own order, which §21 leaves open (bit-identity welcome, "
    "not required)",
}


def sphere_kernel(r: float, d) -> np.ndarray:
    """§9's sphere: 2 round(r / d) + 1 voxels per axis, 1 where the squared distance (single
    precision) is at most r^2."""
    d = np.asarray(d, dtype=np.float32)
    half = [int(np.floor(np.float32(r) / x + np.float32(0.5))) for x in d]
    ax = [np.arange(-h, h + 1, dtype=np.float32) for h in half]
    i, j, k = np.meshgrid(*ax, indexing="ij")
    s = (i * d[0]) ** 2 + (j * d[1]) ** 2 + (k * d[2]) ** 2
    return (s <= np.float32(r) * np.float32(r)).astype(np.float64)


def exact_fmean(x: np.ndarray, kernel: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """-fmean's exact result in float64 from §10.2's definition: the image convolved with the
    kernel, zero outside, divided by the same convolution of ones (for a symmetric kernel the
    orientation does not matter); and that count of kernel voxels inside the image."""
    from scipy import ndimage

    x = np.asarray(x, dtype=np.float64)
    num = ndimage.correlate(x, kernel, mode="constant", cval=0.0)
    cnt = ndimage.correlate(np.ones_like(x), kernel, mode="constant", cval=0.0)
    return num / cnt, cnt


def against_exact(exact: np.ndarray, *outs: np.ndarray) -> list[dict]:
    """Each output's largest and median difference from the exact result, relative to the exact
    result's largest magnitude (§21's measure)."""
    scale = float(np.abs(exact).max())
    res = []
    for y in outs:
        d = np.abs(np.asarray(y, dtype=np.float64) - exact)
        res.append({"max": float(d.max()) / scale, "median": float(np.median(d)) / scale,
                    "p99": float(np.percentile(d, 99)) / scale})  # fmt: skip
    return res


def verdicts(diffs: list[dict], same: dict[str, bool], named: list[str], outcome: tuple[str, str],
             own_same: bool | None = None) -> None:  # fmt: skip
    """The original's comparisons of an example: ``outcome`` is its verdict and reason against
    FSL (computed by the example from §21's metric); the others with itself."""
    from larmorx_validation.evidence import fsl_maths as fm

    fix = {k: v.replace("the default", "the original's default") for k, v in fm.FIX_REASONS.items()}
    for x in diffs:
        i = x["id"]
        if not i.startswith(("original", "ofix")):
            continue
        x["links"] = []
        if i == "original-fsl":
            x["verdict"], x["reason"] = outcome
            if outcome[0] != "bit-identical":
                x["links"].append({"label": "findings", "href": FINDINGS})
        elif i == "original-own":
            if same.get(i):
                x["verdict"] = "bit-identical"
                x["reason"] = "fMRIPrep's own file (fslmaths in fMRIPrep 25.2.6's image) holds the "
                "same values."
            elif own_same is False:
                x["verdict"] = "open"
                x["reason"] = "fMRIPrep's own output is not FSL's here (see the replica's row)."
            else:
                x["verdict"], x["reason"] = outcome
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "original1-original": "The thread count changes nothing: the clean-room "
                "original's parallel loops (per voxel, column, row or block, every sum in a fixed "
                "order) give its one-thread result bit for bit at 14 threads.",
                "ofix-original": "No correction of the register applies to these calls: both "
                "modes give the same values."
                if not named
                else "No correction of the register changes these values: the default mode "
                f"names {', '.join(named)}, with the same values in both modes.",
            }.get(i, "The same values, every one.")
        elif i == "ofix-original" and named:
            x["verdict"] = "d13"
            x["reason"] = " ".join(fix.get(n, f"{n}, corrected by default.") for n in named)
            x["links"] += [{"label": n, "href": REGISTER} for n in named]
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def outcome(fsl: dict[str, np.ndarray], ours: dict[str, np.ndarray], labels: dict[str, str],
            cause: dict[str, str], what: str) -> tuple[tuple[str, str], dict[str, dict]]:  # fmt: skip
    """The original against FSL over an example's outputs: bit-identical where every output's
    values are FSL's; otherwise each differing output's §21 metric, its cause, and *open*
    (within the thresholds or not, as stated)."""
    metrics = {}
    for oid, a in fsl.items():
        b = ours[oid]
        if a.shape == b.shape and a.dtype == b.dtype and a.tobytes() == b.tobytes():
            continue
        metrics[oid] = sums_metric(a, b)
    if not metrics:
        return ("bit-identical", f"The same values, every one, in every output {what} (the "
                "record compares every file, the header, the messages and the exit status "
                "too)."), {}  # fmt: skip
    parts = []
    for oid, m in metrics.items():
        ok = "within §21's thresholds" if within(m) else "beyond §21's thresholds"
        parts.append(f"{metric_words(labels[oid], m)}: {ok}, as {CAUSES[cause.get(oid, 'sums')]}")
    n = len(fsl) - len(metrics)
    head = f"{n} of {len(fsl)} outputs FSL's bit for bit; " if len(fsl) > 1 else ""
    return ("open", common.clip(head + "; ".join(parts) + ".", 900)), metrics


def programs(oinfo: dict) -> str:
    return (
        f"{oinfo.get('versions', {}).get('larmorx', '?')}, from larmorx at commit "
        f"`{oinfo['larmorx_commit'][:12]}`"
    )


def provenance(oinfo: dict, e: fcache.Entry, data: list) -> list[str]:
    return [
        x for x in common.provenance(oinfo, f"{e.tool}/{e.case}", data) if x.startswith("Runs:")
    ]


def note() -> str:
    return (
        " The clean-room original's record (fsl-maths) holds it to specs/fslmaths.md §21: FSL's "
        "exit status, messages, file names and headers identical, the values bit for bit "
        "(double-precision functions within one unit in the last place per function, GLIBC-07), "
        "the sums (-s, -fmean, -fmeanu, the means and standard deviations, -bptf, ...) within 1e-5 "
        "of FSL's largest value with 99 % of the voxels within 4 units in the last place."
    )


# ------------------------------------------------------------------- the cases' default mode


def case_defs(program: str) -> dict[str, tuple[str, tuple]]:
    """Every case of the original's suite (its permanent cases of random testing attached):
    id -> (description, payload)."""
    from larmorx_validation.difftest import permanent

    s = _suite(program).suite()
    permanent.attach(s)
    out: dict[str, tuple[str, tuple]] = {}
    for c in s.cases("full"):
        out.setdefault(c.id, (c.description, c.payload))
    return out


def _case_run(program: str, kind: str, payload):
    """The suite's ``Run`` of a case of kind run, own or direct."""
    su = _suite(program)
    if kind == "own":
        return su._own_cases()[payload][2]
    if kind == "direct":
        return payload
    if program == "fslmaths":
        return su.rep._cases()[payload][2]
    return su.replica_cases()[payload][2]


def default_args(program: str, kind: str, payload, d: Path) -> tuple[list[str], dict]:
    """A case's arguments with its inputs in ``d``, and its settings."""
    from larmorx_validation.evidence import fsl_maths as fm

    if kind in ("fmriprep", "goodvoxels", "bet"):
        return fm._default_args(kind, payload, d, program), {}
    run_ = _case_run(program, kind, payload)
    return _suite(program)._prepare(run_, d), dict(run_.settings)


def _compute_default(program: str):
    def compute(e: fcache.Entry) -> None:
        from concurrent.futures import ThreadPoolExecutor

        from larmorx_validation.parity import defects

        rec = records.load(e.ctx.repo, RECORD[program])
        defs = case_defs(program)
        su = _suite(program)

        def one(r: dict) -> tuple[str, dict]:
            case = r["case"]
            kind, payload, *_ = defs[case][1]
            with tempfile.TemporaryDirectory(prefix="lx-maths-ev-orig-fix_") as t:
                d = Path(t)
                try:
                    args, settings = default_args(program, kind, payload, d)
                except Exception as x:  # an input the case cannot make
                    return case, {"exit": None, "named": [], "note": str(x)[:200]}
                p = su.run_original(args, cwd=d, settings=settings, mode=defects.FIX, threads=1)
            named = defects.notices_in(p.stderr)
            named += [n for n in re.findall(r"\b((?:FSL|GLIBC)-\d+)\b", p.stderr)
                      if n not in named]  # fmt: skip
            return case, {"exit": p.code, "named": named}

        with ThreadPoolExecutor(12) as pool:
            done = dict(pool.map(one, rec.results))
        e.finish(
            versions=versions(),
            default_mode={k: done[k] for k in sorted(done)},
            settings=f"LARMORX's clean-room {program} in its default mode on every case of its "
            "record, at one thread, in a fresh directory with the case's inputs: its exit "
            "status and the register entries it names (no FSL run; the record's fix pass "
            "checks that a name appears exactly when the default's outputs differ)",
        )

    return compute


def default_entry(ctx: core.Context, program: str) -> fcache.Entry:
    tool = RECORD[program]
    return fcache.entry(
        ctx.figures_context(), tool, "original-cases-default-mode",
        f"LARMORX's clean-room {program} in its default mode on every case of its record: its "
        "exit status and the entries it names", _compute_default(program),
    )  # fmt: skip


# ------------------------------------------------------------------------------ the cases


#: The record's passing cases whose values are not FSL's bit for bit, and why (the record's
#: measured values give the numbers).
OPEN_REASONS = {
    "conv/fft-box": CAUSES["fft"],
    "conv/fft-even": CAUSES["fft"],
    "sub/subsamp2": "FSL's subsampling arithmetic under the floating types is not known: no "
    "order of a single or double sum of the voxels gives FSL's last bits (findings 13 and 29; "
    "spec questions 13 and 38)",
    "sub/subsamp2-noform": "as sub/subsamp2: FSL's subsampling arithmetic under the floating "
    "types (spec questions 13 and 38)",
    "sub/subsamp2-nan": "as sub/subsamp2: FSL's subsampling arithmetic under the floating types "
    "(spec questions 13 and 38)",
    "d13/subsamp2offc-4d-even": "FSL's arithmetic of -subsamp2offc's 2 × 2 × 2 means (findings "
    "13 and 29; spec question 13)",
    "difftest/subsamp2-qform-quaternion": "FSL's subsampling arithmetic under the floating types "
    "(spec questions 13 and 38)",
    "stat/ztop-double": "the last bits of Φ in double precision (the original's Φ is the "
    "specification's form with libm's erf and erfc; spec question 35)",
    "stat/ranknorm-double": "the last bits of Φ⁻¹ in double precision (spec questions 35 and 36)",
    "tensor/decomp": "the eigenvalues, FA, MD and mode are FSL's bit for bit; the eigenvectors are "
    "the same up to their sign (|cos| = 1.000000): FSL's signs are LAPACK's, which the "
    "specification does not define (spec question 20), the original's each one's largest "
    "component positive",
    "tensor/chain": "as tensor/decomp: the eigenvectors' signs (spec question 20)",
    "tensor/double": "as tensor/decomp: the eigenvectors' signs (spec question 20)",
    "d13/roc-noise-larger": "-roc's printed area 1e-7 from FSL's (0.01 of §21's allowance of "
    "1e-5), in its last printed digit; the curve file is FSL's; the cause is not established",
    "bet/ds005040-sub-002-noreconall/T1w": "BET's call: the x of the centre of gravity 6.1e-8 "
    "relative from FSL's (97.862248 against 97.862242), within §10's 1e-6: the mapping of the "
    "sums back to millimetres in a neurological image, whose arithmetic in FSL is not known "
    "(findings 21; spec question 24)",
}
TAN = (
    "GLIBC-07: with -dt double, glibc 2.35's tan (IBM's code in glibc, LGPL) is not always "
    "correctly rounded, and the clean-room original's is (CORE-MATH's, in both modes): one unit "
    "in the last place on a few of the 210 double values, within §21's one unit per function. "
    "No permissive code gives glibc's bits; an exact reproduction is planned by a clean-room "
    "implementation of glibc's behaviour."
)


def _zero(v) -> bool:
    return v in (0, 0.0, "0", "0.0", "100.0%")


def _differs(measured: dict) -> dict:
    """The measured values that show a difference from FSL (a sums' share counts only beside a
    bound that is not 0)."""
    out = {}
    for k, v in measured.items():
        if k == "exit status":
            continue
        if k.endswith("share of voxels within 4 ulp"):
            bound = measured.get(k.replace("the sums' share of voxels within 4 ulp",
                                           "values within the sums' bound"))  # fmt: skip
            if _zero(bound):
                continue
        if k.endswith("eigenvectors up to their sign") or not _zero(v):
            out[k] = v
    return out


def case_verdict(r: dict, dm: dict, program: str) -> tuple[str, str]:
    """The original's verdict on a case of its record, from the record's measured values and the
    original's default mode on the case (``dm``)."""
    named, code = dm.get("named", []), dm.get("exit")
    v = common.outcome_verdict(r)
    if v is not None:
        verdict, why = v
        if verdict == "both-reject" and named:
            why += (
                f" The clean-room original's default mode finishes instead (exit status 0), "
                f"naming {', '.join(named)}."
                if code == 0
                else f" The clean-room original's default mode ends with exit status {code}, "
                f"naming {', '.join(named)}."
            )
        return verdict, common.clip(why, 700)
    diff = _differs(r.get("measured", {}))
    notes = []
    fix = r.get("reported", {}).get("D13 fix pass")
    if isinstance(fix, int) and fix:
        notes.append(
            f"in its default mode the clean-room original corrects "
            f"{', '.join(named) or 'a register entry'} and names it: {fix:,} values change"
        )
    elif named and code not in (0, None):
        notes.append(f"its default mode refuses the call (exit {code}), naming {', '.join(named)}")
    elif named:
        notes.append(f"its default mode names {', '.join(named)}, with the same values")
    tail = ("; " + "; ".join(notes)) if notes else ""
    if not diff:
        what = "printed text" if program == "fslstats" else "file fslmaths writes"
        return "bit-identical", common.clip(
            f"Every {what} with FSL's values (the record measures every value), with FSL's exit "
            f"status and messages{tail}.", 600)  # fmt: skip
    if r["case"] == "fn/double-tan":
        return "divergence", common.clip(TAN + (tail[2:].capitalize() + "." if tail else ""), 700)
    measured = "; ".join(f"{k}: {v}" for k, v in diff.items())
    why = OPEN_REASONS.get(r["case"], "the original computes in its own order, within the "
                           "record's thresholds")  # fmt: skip
    return "open", common.clip(
        f"Within the record's thresholds, not bit for bit ({measured}): {why}{tail}.", 900)  # fmt: skip


def _input(name: str) -> str:
    if name.startswith("orig:"):
        return f"<{name[5:]}: a synthetic input of the original's suite>"
    if name.startswith("dt:"):
        return f"<random testing's input {name.split(':', 1)[1][:12]}>"
    return f"<{name}: a synthetic input>"


def command(case: str, defs: dict, program: str) -> str:
    """A case's command line as a reader needs it (inputs named, not their paths)."""
    import shlex

    from larmorx_validation.evidence import fsl_maths as fm

    kind, payload, *_ = defs[case][1]
    if kind in ("fmriprep", "goodvoxels", "bet"):
        return fm.command(case, {case: ("", (kind, payload))}, program)
    run_ = _case_run(program, kind, payload)

    def arg(a: str) -> str:
        if a.startswith("@"):
            return _input(a[1:])
        return shlex.quote(a) if re.search(r"\s", a) or a == "" else a

    parts = [program, *(arg(a) for a in run_.args)]
    env = " ".join(f"{k}={v}" for k, v in run_.settings if v is not None)
    unset = " ".join(f"{k} unset" for k, v in run_.settings if v is None)
    return " ".join(x for x in (env, " ".join(parts), f"({unset})" if unset else "") if x)


def add_cases(ctx: core.Context, cs: list[dict], program: str, examples: dict[str, str]) -> tuple:
    """The original's results joined to the page's cases (a case of its own where the replica's
    record has none: its ``orig/`` cases and its permanent cases of random testing); returns the
    record and the default-mode entry."""
    rec = records.load(ctx.repo, RECORD[program])
    de = default_entry(ctx, program)
    default = de.info().get("default_mode", {})
    defs = case_defs(program)
    by = {c["id"]: c for c in cs}
    for r in rec.results:
        v, why = case_verdict(r, default.get(r["case"], {}), program)
        result = {
            "outcome": r["status"],
            "verdict": v,
            "checks": f"{r['checks_passed']}/{r['checks_total']}",
            "reason": why,
        }
        if r["case"] in by:
            if r["case"].startswith("difftest/"):
                raise ValueError(f"{r['case']}: a permanent case in both records")
            by[r["case"]]["results"]["original"] = result
            continue
        cs.append({
            "id": r["case"],
            "category": r["category"],
            "description": defs[r["case"]][0],
            "command": command(r["case"], defs, program),
            "results": {"original": result},
            **({"example": examples[r["case"]]} if r["case"] in examples else {}),
        })  # fmt: skip
    return rec, de


def implementation(cs: list[dict], program: str) -> dict:
    return common.implementation(
        cs, "original", "LARMORX clean-room original, against FSL 6.0.7.23 (standard tier)",
        "original", f"docs/validation/{RECORD[program]}.md", "experimental",
    )  # fmt: skip


# ------------------------------------------------------------------------------ the trial


#: The triage's categories in plain words: the category (without its tool) -> (text, verdict).
TRIAGE_TEXT: dict[str, tuple[str, str | None]] = {
    "larmorx mistake, fixed: the geometry of a new grid along x (findings 24)": (
        "a LARMORX mistake, fixed: the geometry of a new grid along x (-subsamp2, the reductions "
        "along X of an image FSL holds mirrored or without matrices)", None),
    "larmorx mistake, fixed: -roc's false-positive candidates (findings 29)": (
        "a LARMORX mistake, fixed: -roc's false-positive candidates (the mask's voxels below the "
        "true voxels' bound)", None),
    "larmorx mistake, fixed: the morphology's messages with a kernel image (findings 25)": (
        "a LARMORX mistake, fixed: the morphological filters' messages with a kernel image",
        None),
    "larmorx mistake, fixed: a subsampled qform's quaternion (findings 24)": (
        "a LARMORX mistake, fixed: a subsampled image's qform quaternion", None),
    "larmorx mistake, fixed: TFCE's step tested before the sizes (findings 26)": (
        "a LARMORX mistake, fixed: TFCE's checks in FSL's order", None),
    "larmorx mistake, fixed: FSL-157 named for every int8 -odt input (findings 29)": (
        "a LARMORX mistake, fixed: FSL-157 named for every int8 image written with -odt input",
        None),
    "larmorx mistake, fixed: FSL-165 on every image with an axis of size 1 (findings 27)": (
        "a LARMORX mistake, fixed: FSL-165 for -fillh of every image with an axis of size 1",
        None),
    "larmorx mistake, fixed: TFCE of a double image in single precision (findings 26)": (
        "a LARMORX mistake, fixed: TFCE of a double image in single precision", None),
    "larmorx mistake, fixed: FSL-154 named for every uint8 image computed unsigned": (
        "a LARMORX mistake, fixed: FSL-154 named for every uint8 image computed unsigned", None),
    "larmorx mistake, fixed: -ptoz's and -ztop's NaN (findings 30)": (
        "a LARMORX mistake, fixed: -ptoz's and -ztop's NaN", None),
    "larmorx mistake, fixed: the subsampling's sums in double (findings 29)": (
        "a LARMORX mistake, fixed: the subsampling's sums under the integer types", None),
    "larmorx mistake, fixed: -edge's integer differences (findings 29)": (
        "a LARMORX mistake, fixed: -edge's differences under the integer types", None),
    "larmorx mistake, fixed: FSL-75 named in -subsamp2 (findings 24: FSL's heap abort)": (
        "a LARMORX mistake, fixed: FSL-75 named in -subsamp2", None),
    "fixed: a chain the fixes above reach": (
        "fixed: a chain that one of the fixes above reaches", None),
    "harness, fixed: the suite's tensor check (single slices) and NIfTI-2 quaternions": (
        "the suite's own mistake, fixed: its tensor check on single slices, NIfTI-2 quaternions",
        None),
    "harness, fixed: the suite's tensor check (compressed names, single slices)": (
        "the suite's own mistake, fixed: its tensor check (compressed names, single slices)",
        None),
    "harness, fixed: §21's TFCE metric in the suite": (
        "the suite's own mistake, fixed: §21's TFCE metric", None),
    "explained: -tensor_decomp of degenerate tensors (eigenvalues at the rounding level)": (
        "explained: -tensor_decomp of degenerate tensors (eigenvalues at the rounding level: "
        "their eigenvectors are not defined)", None),
    "explained: the header's offsets after a grid change (at most 6 ulp of one offset)": (
        "explained: the header's offsets after a grid change, at most 6 units in the last place "
        "of one offset (spec question 31)", None),
    "explained: -subsamp2's last bits under the floating types (spec question 13)": (
        "explained: -subsamp2's last bits under the floating types (spec questions 13 and 38)",
        None),
    "explained: -ztop's and -ptoz's last bits (Φ, Φ⁻¹), within the sums' bound": (
        "explained: -ztop's and -ptoz's last bits (Φ, Φ⁻¹), within the sums' bound (spec "
        "question 35)", None),
    "explained: -dog_edge's crossings at exact ties (within the sums' bound)": (
        "explained: -dog_edge's zero crossings at exact ties, within the sums' bound", None),
    "explained: sums beyond 4 ulp where they cancel to near 0 (spec question 14)": (
        "explained: sums beyond 4 units in the last place where they cancel to near 0 (spec "
        "question 14)", None),
    "explained: -randn in double: Φ⁻¹'s last bits": (
        "explained: -randn in double precision: Φ⁻¹'s last bits (spec question 36)", None),
    "explained: the median's and mode's signed zero (§18: ±0's order)": (
        "explained: the median's and the mode's signed zero (§18: ±0's order)", None),
    "explained: TFCE's last bits through a discontinuous operation (-rem)": (
        "explained: TFCE's last bits through a discontinuous operation (-rem)", None),
    "expected: -subsamp2offc's means (findings 13 and 29)": (
        "expected: -subsamp2offc's means (as the record's two expected divergences)",
        "divergence"),
    "register: FSL-75 (FSL's heap corruption crashes it before it writes; named)": (
        "FSL-75: FSL's heap corruption ends it before it writes; the original finishes and names "
        "it (rule A)", "d13"),
    "larmorx mistake, fixed: fslstats' histograms without a histogram (findings 28)": (
        "a LARMORX mistake, fixed: fslstats' histograms without a histogram (the output and the "
        "JSON of -h and -H when no histogram is made)", None),
    "harness, fixed: §10's bound over a histogram's lines and an output cut short": (
        "the suite's own mistake, fixed: §10's bound over a histogram's lines and an output cut "
        "short", None),
    "explained: fslstats -c/-C of images holding NaN and infinities: the NaN's sign": (
        "explained: -c and -C of images holding NaN and infinities: the NaN's sign (spec "
        "question 34)", None),
    "explained: fslstats' last printed digit at §10's bound (or bounded words unattributed)": (
        "explained: the last printed digit at §10's bound", None),
}  # fmt: skip
#: The open categories (their names cut short in the triage file): prefix -> text.
OPEN_TEXT = (
    ("open: roc.txt", "open: -roc's curve numbers 2.4e-7 and 5e-7 from FSL's, beyond half a unit "
     "of the 5th significant digit"),
    ("open: out", "open: the display range (cal_min, cal_max) after -range, in its last bits, or "
     "-2147483648 in FSL against NaN"),
)  # fmt: skip


def _triage_text(cat: str) -> tuple[str, str | None]:
    if cat in TRIAGE_TEXT:
        return TRIAGE_TEXT[cat]
    for prefix, text in OPEN_TEXT:
        if cat.startswith(prefix):
            return text, "open"
    raise KeyError(f"no text for the triage category {cat!r}")


def trial(ctx: core.Context, tool: str) -> tuple[list[dict], dict]:
    """The originals' random-testing trial (its ``summary.json``) for ``tool`` (fsl-maths or
    fsl-stats) and its triage (``trial_triage.json``): two tables, and their provenance."""
    path = common.oracles() / "difftest" / TRIAL / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    name = f"{tool}-original"
    t = s["tools"][name]
    impl = t["implementations"]["original"]
    o = t["options"]
    program = "fslmaths" if tool == "fsl-maths" else "fslstats"
    rows = [
        (f"Random cases of {program}, each compared with FSL 6.0.7.23", t["cases"]),
        ("within the record's thresholds, every exact check passed", impl.get("agree", 0)),
        ("differing only by a correction the default mode names", impl.get("notice", 0)),
        ("differing at the trial (each one triaged below)", impl.get("differ", 0)),
        ("distinct options and settings drawn", t["options_covered"]),
    ]
    if tool == "fsl-maths":
        rows += [
            ("chains with -s, -fmean or -fmeanu (the convolutions)",
             o.get("-s", 0) + o.get("-fmean", 0) + o.get("-fmeanu", 0)),
            ("-dt double, -dt input", [o.get("-dt double", 0), o.get("-dt input", 0)]),
        ]  # fmt: skip
    else:
        rows += [("-t (per volume), -K (per label), -json",
                  [o.get("-t", 0), o.get("-K", 0), o.get("-json", 0)])]  # fmt: skip
    rows.append((f"length (minutes; {s['jobs']} jobs at one thread each, both programs)",
                 round(s["elapsed_s"] / 60, 1)))  # fmt: skip
    out_rows = []
    for head, v in rows:
        if isinstance(v, list):
            cell = common.cell(" / ".join(f"{x:,}" for x in v))
        elif isinstance(v, float):
            cell = common.cell(v, "sig")
        else:
            cell = common.cell(int(v), "count")
        out_rows.append({"head": head, "cells": [cell]})
    peaks = max(float(p.split()[0]) for p in s["memory"]["peaks"])
    summary = common.table(
        "trial-original",
        f"Random testing of the clean-room original: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of the clean-room originals (seed "
        f"`{s['seed']}`, larmorx `{s['larmorx_commit']}`, both programs at once): the replica "
        "trial's streams drawn again, so the replica trial's runs of FSL were served by the "
        "oracle cache and the rest stored there; realistic cases only, each compared as the "
        f"record compares. Every case in a memory box of {s['memory']['case']} (peaks below "
        f"{peaks:.2f} GiB). The differing cases were re-run with the part's final build and "
        "each put in one category (below).",
    )
    tpath = ctx.repo / TRIAGE
    traw = tpath.read_bytes()
    tri = json.loads(traw)
    counts = {k.split(": ", 1)[1]: n for k, n in tri["counts"].items() if k.startswith(name + ":")}
    repro = {k.split(": ", 1)[1]: v for k, v in tri["reproducers"].items()
             if k.startswith(name + ":")}  # fmt: skip
    if sum(counts.values()) != impl.get("differ", 0):
        raise ValueError(f"{TRIAGE}: {sum(counts.values())} findings, the trial {impl['differ']}")
    # categories the triage file keeps apart by their cut names (the open display ranges, one
    # per output name) are one row here: their counts added, the largest one's reproducer
    merged: dict[str, list] = {}
    for cat, n in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0])):
        text, verdict = _triage_text(cat)
        if text in merged:
            merged[text][0] += n
        else:
            merged[text] = [n, verdict, repro[cat].split("/")[-1]]
    trows = []
    for text, (n, verdict, case) in sorted(merged.items(), key=lambda kv: -kv[1][0]):
        row = {"head": text, "cells": [common.cell(int(n), "count"), common.cell(case)]}
        if verdict:
            row["verdict"] = verdict
        trows.append(row)
    triage = common.table(
        "trial-original-triage",
        "The trial's differing cases, each in one category",
        ["Category", {"label": "Cases", "num": True}, "Reproducer"],
        trows,
        note="Every differing case of the trial re-run with the part's final build and put in one "
        "category by its recipe and its first failed check "
        "(`validation/investigations/cleanroom-maths/triage_counts.py`, its output "
        f"`{TRIAGE}`), each category's cause established by probes and replays "
        f"(`{FINDINGS}`, findings 24 to 30); LARMORX's mistakes and the suite's are fixed, with "
        "17 permanent cases of fslmaths and 4 of fslstats in the records; the reproducer is a "
        "finding of the trial (`difftest --replay`).",
    )
    prov = {
        "trial": f"oracles/difftest/{TRIAL}/summary.json",
        "sha256": hashlib.sha256(raw).hexdigest(),
        "triage": TRIAGE,
        "triage_sha256": hashlib.sha256(traw).hexdigest(),
    }
    return [summary, triage], prov


# ------------------------------------------------------------------------------- speed


#: The benchmark report's rows, per program (the case column's first words).
_STATS_ROWS = ("BET's", "`fslstats")
SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23 (single-threaded)", "mark": "original"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica (single-threaded)",
     "mark": "replica"},
    {"id": "original (larmorx)|1 thread", "label": "clean-room original, 1 thread",
     "mark": "larmorx"},
    {"id": "original (larmorx)|14 threads", "label": "clean-room original, 14 threads",
     "mark": "larmorx-open"},
]  # fmt: skip
_TOOLS = ["FSL 6.0.7.23", "replica (larmorx-nc)", "original (larmorx)", "original (larmorx)"]
_OPS = ["1 thread", "1 thread", "1 thread", "14 threads"]


def _bench_rows(ctx: core.Context) -> tuple[list[dict], str, str]:
    """The report's results table: each row's case, seconds and MiB per implementation and
    checks; the report's text and its path."""
    p = ctx.repo / BENCH
    md = p.read_text(encoding="utf-8")
    body = md.split("## Results", 1)[1].split("\n\n", 2)[1]
    rows = []
    for line in body.splitlines()[2:]:
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) != 6:
            continue
        runs = []
        for c in cells[1:5]:
            m = re.fullmatch(r"([\d.]+) \((\d+)\)", c)
            if m is None:
                raise ValueError(f"{BENCH}: {c!r}")
            runs.append((float(m.group(1)), int(m.group(2))))
        program = "fslstats" if cells[0].startswith(_STATS_ROWS) else "fslmaths"
        rows.append({"case": cells[0], "program": program, "runs": runs, "checks": cells[5]})
    return rows, md, p.relative_to(ctx.repo).as_posix()


def _plain(case: str) -> str:
    return case.replace("`", "").replace("\\|", "|")


def speed(ctx: core.Context, program: str) -> tuple[list[dict], dict | None, dict]:
    """The speed chart (seconds, from the benchmark report: informal, one run each) and the
    table of every row with its peak memory, for ``program``'s rows."""
    rows, md, rel = _bench_rows(ctx)
    rows = [r for r in rows if r["program"] == program]
    m = re.search(r"main machine \(([^)]*)\)", md)
    machine = m.group(1) if m else "?"
    results = []
    charted = []
    for r in rows:
        if min(s for s, _ in r["runs"]) <= 0:  # below the report's 0.01 s: not on a log axis
            continue
        charted.append(r["case"])
        for tool, op, (sec, _mib) in zip(_TOOLS, _OPS, r["runs"], strict=True):
            results.append({"tool": tool, "operation": op, "input": r["case"], "median": sec})
    bench = types.SimpleNamespace(results=results, data={"environment": {}})
    chart = common.speed(
        bench,
        SPEED_SERIES,
        {c: _plain(c) for c in charted},
        title="Seconds for the whole call (read the images, compute, write every output), one run "
        "each, log scale; the factor is the clean-room original's speed-up over FSL, both on "
        "one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[2]["id"]),
        note=f"From the benchmark report `{rel}` (the main machine: {machine}; informal, one run "
        "each, while another agent worked beside it). FSL and the replica are single-threaded; "
        "the original's outputs are the same bits at 1 and 14 threads.",
        inputs=charted,
    )
    trows = []
    for r in rows:
        cells = [common.cell(f"{s:.2f} s ({mib} MiB)") for s, mib in r["runs"]]
        trows.append({"head": _plain(r["case"]), "cells": [*cells, common.cell(r["checks"])]})
    table = common.table(
        "benchmark",
        "The benchmark: seconds and peak memory",
        ["Call", "FSL 6.0.7.23", "Replica", "Original, 1 thread", "Original, 14 threads",
         "Against FSL"],
        trows,
        note=f"The benchmark report `{rel}`: wall-clock seconds of the whole program and its peak "
        "resident memory (MiB), one run each on the main machine with another agent's work "
        "beside it (informal). The memory rule (at most 10 % above FSL's peak plus 8 MiB) is "
        "checked at one thread by tests/parity/test_memory_peak.py; on 14 threads a .nii.gz "
        "output adds about 3 MiB per worker (the shared parallel gzip writer).",
    )  # fmt: skip
    source = {"path": rel, "sha256": hashlib.sha256((ctx.repo / BENCH).read_bytes()).hexdigest()}
    return [source], chart, table
