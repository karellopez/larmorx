# SPDX-License-Identifier: Apache-2.0
"""BET's variations ``-S``, ``-B``, ``-A``, ``-A2`` and standard_space_roi by LARMORX's clean-room
original (written in the clean-room catch-up of 2026-11-05 from ``specs/bet.md`` §5.3 and
``specs/standard_space_roi.md``), on the heads of the replica's worked examples
(:mod:`fsl_bet_options`, used as a black box): each of those examples gains the original's runs in
its default and ``reproduce`` modes (the same call on the same head as FSL 6.0.7.23's) and their
comparisons with FSL's run and the replica's.

The original's runs live in cache entries of their own (``options-<case>-original``); FSL's and
the replica's runs stay in the replica's entries and are never computed again. The panels are
added to the replica's examples, by the example each is, before the example is finished
(``common.finish``), so they are encoded on the same display grid and in the same three slices.
Each output is matched with its file by the replica's own panel: the replica's file of that name
must give the replica's panel exactly, else the export stops."""

from __future__ import annotations

import contextlib
import os
import re
import sys
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core, fsl_step
from larmorx_validation.evidence.example import Panel
from larmorx_validation.figures import cache as fcache

TOOL = "fsl-bet"
#: The replica's examples of the variations: example id -> its cache entry (the same heads and
#: calls; ``RUN_INFO.json`` names the call, its inputs and its record case).
EXAMPLES = {
    "bet-S-adult": "options-adult-ds000005-S",
    "bet-S-tumour": "options-tumour-S",
    "bet-B-child": "options-child-3y-B",
    "bet-B-older": "options-older-82y-B",
    "bet-A-older": "options-older-75y-A",
    "bet-A2-child": "options-child-7y-A2",
    "ssroi-child": "options-ssroi-child-b",
}
#: The output each panel array is, by file.
FILES = {
    "brain": "out.nii.gz",
    "mask": "out_mask.nii.gz",
    "skull": "out_skull.nii.gz",
    "inskull": "out_inskull_mask.nii.gz",
    "outskull": "out_outskull_mask.nii.gz",
    "outskin": "out_outskin_mask.nii.gz",
    "skullmask": "out_skull_mask.nii.gz",
    "roi": "out.nii.gz",
}
#: The original's runs: label, D13 mode, the panel's subtitle.
RUNS = [
    ("orig", "fix", "default mode (Apache-2.0, crate larmorx-fsl)"),
    ("orig-reproduce", "reproduce", "reproduce mode"),
]
LABEL = "larmorx's clean-room original"
FINDINGS = "docs/findings/fsl-bet-clean-room.md"
_NOTICE = re.compile(
    r"upstream defect ((?:FSL|GLIBC)-\d+) \(([^)]*)\) (corrected|reproduced|refused)"
)
#: The corrections the original's default can name inside these scripts, what each corrects.
CORRECTED = {
    "FSL-09": "FLIRT's integer output inside standard_space_roi: rounded to the nearest and "
    "clamped where FLIRT truncates toward zero and wraps",
    "FSL-14": "FLIRT's costs leave NaN voxels out",
    "FSL-202": "FAST's class variances (the bias field of -B) with the square of the "
    "double-precision mean, where FAST squares a single-precision mean in single precision",
    "FSL-214": "standard_space_roi stops at the first program that fails",
    "FSL-216": "-S's eye threshold read as numbers, not as dc commands",
    "FSL-217": "an int8 image kept as int8 by -F's and -Z's fslroi",
    "FSL-182": "fslstats' -S (-S's eye threshold) without a mask: the masked forms' variance, "
    "clamped at 0 and summed in double precision",
    "FSL-51": "betsurf's profile searches stop at a profile's last sample, where FSL reads past "
    "it into memory it never set",
}
#: The default's maths functions (GLIBC-07, named nowhere: the user's decision of 2026-11-04).
GLIBC07 = (
    "the default computes the C library's maths functions correctly rounded, the same on every "
    "platform, where FSL takes glibc 2.35's (GLIBC-07, named nowhere by the user's decision of "
    "2026-11-04); reproduce computes them as glibc does. Inside the script's programs (FLIRT's "
    "rotations and windowed sinc, bet2's powf, betsurf's smoothing) their last bits move the "
    "registration's or the surfaces' path a little"
)


def _source(ctx: core.Context, case: str) -> fcache.Entry:
    e = fcache.Entry(ctx.figures_context(), TOOL, case, "")
    if not e.done:
        raise FileNotFoundError(f"figures cache entry {TOOL}/{case} (the replica's) is missing")
    return e


def _inputs(info: dict) -> dict[str, Path]:
    """The replica entry's inputs: label -> path (its ``RUN_INFO.json``: "path (sha256 …)")."""
    return {k: Path(v.rsplit(" (sha256 ", 1)[0]) for k, v in info["inputs"].items()}


def _compute(src: fcache.Entry):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.evidence import fsl_bet

        info = src.info()
        fsl = next(r for r in info["runs"] if r["label"] == "fsl23")
        argv = list(fsl["argv"])
        program, args = Path(argv[0]).name, argv[1:]
        for label, path in _inputs(info).items():
            e.input(label, path)
        home = e.dir / ".home"
        home.mkdir(exist_ok=True)
        add, unset = fsl_bet.environment(home, fsl_step.NEW)
        named = {}
        for label, mode, _sub in RUNS:
            d = e.dir / label
            d.mkdir(exist_ok=True)
            env = {
                **add,
                "LARMORX_IMPLEMENTATION": "original",
                "LARMORX_UPSTREAM_DEFECTS": mode,
                "OMP_NUM_THREADS": "2",
                "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
            }
            e.run(
                label,
                [sys.executable, "-m", "larmorx", "fsl", program, *args],
                env=env,
                unset=unset,
                cwd=d,
            )
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            named[label] = sorted({m.group(1) for m in _NOTICE.finditer(log)})
        e.finish(
            versions=fsl_bet.versions_orig(),
            command=info["command"],
            record_case=info.get("record_case"),
            replica_entry=src.case,
            named=named,
            settings=f"larmorx's clean-room original (python -m larmorx fsl {program}, "
            "LARMORX_IMPLEMENTATION=original) in its default and reproduce modes on the call of "
            f"the replica's entry {src.case} (FSL 6.0.7.23's run there), each in a directory of "
            "its own, in the parity suite's environment (FSLDIR set: the standard images read "
            "from FSL 6.0.7.23's data, as the data search finds them; an empty HOME, LC_ALL=C, "
            "OpenBLAS's Haswell kernels); OMP_NUM_THREADS=2 (the result does not depend on it)",
        )

    return compute


def entry(ctx: core.Context, example: str) -> fcache.Entry:
    src = _source(ctx, EXAMPLES[example])
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"{src.case}-original",
        f"the clean-room original on the call of {src.case} ({src.info()['command']}), default "
        "and reproduce",
        _compute(src),
    )


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [entry(ctx, x) for x in EXAMPLES]


def refresh(ctx: core.Context, why: str) -> list[fcache.Entry]:
    """The original's entries computed again (every run in them is the original's)."""
    out = []
    for x in EXAMPLES:
        src = _source(ctx, EXAMPLES[x])
        out.append(
            fcache.recompute(
                ctx.figures_context(),
                TOOL,
                f"{src.case}-original",
                f"the clean-room original on the call of {src.case} ({src.info()['command']}), "
                "default and reproduce",
                _compute(src),
                why,
            )
        )
    return out


# ------------------------------------------------------------------------------ the panels


def _which(ex) -> str | None:
    """The example the replica's module is finishing, by its FSL panel's command and its input
    panel's label (each pair names one example)."""
    labels = {p.id: p.label for p in ex.panels}
    if "fsl23" not in labels or "replica23" not in labels:
        return None
    shown = [w for w in labels["fsl23"].split(", ", 1)[-1].split() if w.startswith("-")]
    program = labels["fsl23"].split(", ", 1)[-1].split()[0]
    for x, case in EXAMPLES.items():
        info = fcache.Entry(ex.ctx.figures_context(), TOOL, case, "").info()
        cmd = info["command"].split()
        words = [w for w in cmd if w.startswith("-")]
        if cmd[0] == program and shown == words and _same_input(ex, info):
            return x
    return None


def _same_input(ex, info: dict) -> bool:
    """Whether the example's input panel is the entry's first input (its label's dataset and
    subject in the entry's input label)."""
    first = next(p for p in ex.panels if p.role == "input")
    words = first.label.split()
    key = next(iter(info["inputs"]))
    return len(words) >= 3 and words[1] in key and words[2].removeprefix("sub-") in key


def _load(e: fcache.Entry, label: str, out: str, like: Panel):
    """The original's file of ``out`` as the replica's panel holds its own (as stored)."""
    p = e.dir / label / FILES[out]
    a, scale, _inter = common.stored(p)
    if like.scale is None and scale is not None:
        return np.asarray(common.load(str(p)))
    return a


def _check_files(ex, src: fcache.Entry, outputs: list[dict]) -> None:
    """Each output's file gives the replica's own panel: the outputs are the files named here."""
    rep = ex.panel("replica23")
    for o in outputs:
        oid = o["id"]
        mine = _load(src, "replica23", oid, rep)
        theirs = rep.full[oid]
        if mine.dtype != theirs.dtype or not np.array_equal(mine, theirs):
            raise ValueError(f"{src.case}: {FILES[oid]} is not the replica's {oid} panel")


def _diffs() -> list[dict]:
    d = common.d
    new = fsl_step.NEW_LABEL
    return [
        d("orig-fsl23", "orig", "fsl23", "main", f"original − {new}", common.ROLE_LX),
        d("orig-repro-fsl23", "orig-reproduce", "fsl23", "main",
          f"original (reproduce) − {new}", common.ROLE_LX),
        d("orig-replica23", "orig", "replica23", "check",
          "original − replica, both defaults", common.ROLE_SELF),
        d("orig-repro", "orig", "orig-reproduce", "check", "original, default − reproduce mode",
          common.ROLE_SELF),
    ]  # fmt: skip


def _counts(x: dict, ex, outputs: list[dict]) -> str:
    parts = []
    for o in outputs:
        m = x["metrics"].get(o["id"])
        if m is None or m["identical"]:
            continue
        a = np.asarray(ex.panel(x["a"]).full[o["id"]])
        b = np.asarray(ex.panel(x["b"]).full[o["id"]])
        n = int(np.count_nonzero(a != b))
        ma, mb = a != 0, b != 0
        dice = 2 * np.count_nonzero(ma & mb) / max(1, np.count_nonzero(ma) + np.count_nonzero(mb))
        parts.append(
            f"{o['label'].lower()}: {n:,} of {a.size:,} values"
            + (f" (the non-zero voxels' Dice {dice:.5f})" if not np.array_equal(ma, mb) else "")
        )
    return "; ".join(parts)


#: What the record says of a case the original misses.
MISSES = {
    "real/child-3y-B": "the record's failure: -B's last step, bet -R with -c from the robust "
    "centre, starts bet2 a few bits from FSL's on this neurologically stored image (the "
    "specification's open question of random testing's cases 622 and 677: the conversion of a "
    "centre in voxels into FSL's radiological frame)",
}


def _record_note(rec_orig, case: str | None) -> str:
    r = next((x for x in rec_orig.results if x["case"] == case), None) if case else None
    if r is None:
        return ""
    miss = f": {MISSES[case]}" if case in MISSES and r["status"] == "fail" else ""
    return (
        f" Its record case `{case}` (the original against FSL 6.0.7.23): {r['status']}, "
        f"{r['checks_passed']} of {r['checks_total']} checks{miss}."
    )


def _verdicts(diffs: list[dict], ex, outputs: list[dict], info: dict, rec_orig) -> None:
    same = common.identical(diffs)
    named = info.get("named", {})
    fix_ids = [i for i in named.get("orig", []) if i.startswith("FSL-")]
    rec = _record_note(rec_orig, info.get("record_case"))
    files = ", ".join(f"`{FILES[o['id']]}`" for o in outputs)
    for x in diffs:
        x["links"] = [{"label": "findings", "href": FINDINGS}]
        i = x["id"]
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "orig-fsl23": f"The default mode gives FSL 6.0.7.23's values, every one, in {files}"
                + (
                    f": it names {', '.join(fix_ids)}, whose corrections change only "
                    "intermediate values here."
                    if fix_ids
                    else "."
                )
                + rec,
                "orig-repro-fsl23": f"The same values as FSL 6.0.7.23's, every one, in {files}."
                + rec,
                "orig-replica23": "The clean-room original's default gives the replica's default "
                "values, every one: the two implementations, one written from FSL's source and "
                "one from its specification, agree here.",
                "orig-repro": "Both modes give the same values"
                + (
                    f": the default names {', '.join(fix_ids)}, whose corrections change nothing "
                    "shown here."
                    if fix_ids
                    else "; the default names no correction on this head."
                ),
            }[i]
            continue
        counts = _counts(x, ex, outputs)
        named_text = "; ".join(f"{d} ({CORRECTED[d]})" for d in fix_ids if d in CORRECTED)
        why = (
            "the default's corrections, each named as it is made: " + named_text + "; and "
            if named_text
            else ""
        ) + GLIBC07
        if i == "orig-repro":
            x["verdict"] = "d13"
            x["reason"] = common.clip(
                f"{why[0].upper()}{why[1:]}. What they change: {counts}.",
                900,
            )
        elif i == "orig-fsl23" and same["orig-repro-fsl23"]:
            x["verdict"] = "d13"
            x["reason"] = common.clip(
                f"Deliberate in the default: {why}. The reproduce mode gives FSL's values, every "
                f"one. What the default changes: {counts}." + rec,
                900,
            )
        elif i == "orig-replica23":
            x["verdict"] = "d13" if same["orig-repro-fsl23"] else "open"
            x["reason"] = common.clip(
                (
                    "Both reproduce modes give FSL's values; the two defaults differ in their "
                    "corrections' arithmetic (the original names "
                    f"{', '.join(fix_ids) or 'none'}; the replica's are named in its run) and in "
                    "the maths functions: the original's default computes them correctly "
                    "rounded (GLIBC-07), the replica's as glibc does. "
                    if same["orig-repro-fsl23"]
                    else "The original's reproduce mode is not FSL's here (the comparison above); "
                    "the replica's is. "
                )
                + f"Where they differ: {counts}.",
                900,
            )
        else:
            surf = any(o["id"] in ("inskull", "outskull", "outskin", "skullmask") for o in outputs)
            x["verdict"] = "open"
            x["reason"] = common.clip(
                f"Not FSL 6.0.7.23's values: {counts}. "
                + (
                    "bet2's brain and every step before betsurf are FSL's; betsurf decides its "
                    "profiles as FSL does and its surfaces settle a little "
                    "elsewhere, the scalp most (docs/findings/fsl-bet-clean-room.md, betsurf), "
                    "held to its masks' thresholds (Dice >= 0.99, boundary p95 <= 1 voxel)."
                    if surf
                    else "The clean-room original runs the script's programs as its own "
                    "clean-room programs (FLIRT, FAST, fslmaths, fslstats, bet2), each held to "
                    "its record's thresholds where it is not bit-identical."
                )
                + (" In the default also: " + GLIBC07 + "." if i == "orig-fsl23" else "")
                + rec
                + " The difference is not closed: the original could still give FSL's files.",
                900,
            )


def _add(ex, diffs: list[dict], outputs: list[dict], kw: dict, e: fcache.Entry, src, rec_orig):
    """The original's two panels and four comparisons, with their verdicts, added to the replica's
    example before it is finished; the original joins the default view's panels."""
    _check_files(ex, src, outputs)
    rep = ex.panel("replica23")
    for label, _mode, sub in RUNS:
        full = {o["id"]: _load(e, label, o["id"], rep) for o in outputs}
        ex.add(Panel(label, "larmorx", LABEL, sub, full, scale=rep.scale, inter=rep.inter,
                     overlay=rep.overlay))  # fmt: skip
    mine = _diffs()
    common.compare_all(ex, mine, outputs)
    _verdicts(mine, ex, outputs, e.info(), rec_orig)
    # after the replica's comparisons of the same kind
    for x in mine:
        at = max((k for k, y in enumerate(diffs) if y["kind"] == x["kind"]), default=len(diffs) - 1)
        diffs.insert(at + 1, x)
    default = kw["default"]
    if "orig" not in default["panels"]:
        at = default["panels"].index("replica23") + 1 if "replica23" in default["panels"] else 2
        default["panels"].insert(at, "orig")
    if "orig-fsl23" not in default["diffs"]:
        default["diffs"].insert(1, "orig-fsl23")


@contextlib.contextmanager
def _finishing(ctx: core.Context, rec_orig, found: dict):
    """While the replica's module builds its examples: each, at ``common.finish``, gains the
    original's panels (the example it is: :func:`_which`)."""
    real = common.finish

    def finish(c, ex, diffs, outputs, **kw):
        x = _which(ex)
        if x is not None:
            src = _source(ctx, EXAMPLES[x])
            _add(ex, diffs, outputs, kw, entry(ctx, x), src, rec_orig)
            found[x] = True
        return real(c, ex, diffs, outputs, **kw)

    common.finish = finish
    try:
        yield
    finally:
        common.finish = real


def examples(ctx: core.Context, rec_new, rec_orig) -> list[dict]:
    """The replica's examples of the variations (:mod:`fsl_bet_options`), each with the
    original's panels, comparisons, facts and provenance."""
    from larmorx_validation.evidence import fsl_bet_options

    found: dict[str, bool] = {}
    with _finishing(ctx, rec_orig, found):
        docs = fsl_bet_options.examples(ctx, rec_new)
    missing = [x for x in EXAMPLES if x not in found]
    if missing:
        raise ValueError(f"the original's panels were not added to {missing}")
    for doc in docs:
        if doc["id"] not in EXAMPLES:
            continue
        e = entry(ctx, doc["id"])
        info = e.info()
        named = info.get("named", {})
        prog = info["runs"][0]["argv"][4]
        doc["facts"].append(
            {
                "label": "Clean-room original",
                "text": f"LARMORX's clean-room original (crate larmorx-fsl, Apache-2.0; "
                f"{info['versions']['larmorx']}), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`: `larmorx fsl {prog}` on the same call "
                f"(`{info['command']}`), in its default and reproduce modes, every program the "
                "script runs its own clean-room program, in this process; the standard images "
                "read from FSL 6.0.7.23's data (`$FSLDIR/data/standard`) by the data search. "
                "Named by its default: "
                + (", ".join(named.get("orig", [])) or "no correction")
                + "; by reproduce: "
                + (", ".join(named.get("orig-reproduce", [])) or "nothing")
                + ".",
            }
        )
        doc["provenance"].append(
            f"Runs of the clean-room original: figures cache entry `{TOOL}/{e.case}`, computed "
            f"{info.get('computed')} with larmorx `{info['larmorx_commit'][:12]}`; "
            f"{info['versions']['larmorx']}; every command, its environment and exit status in "
            "its `RUN_INFO.json`"
        )
        doc["summary"] = (
            doc.get("summary", "").rstrip()
            + " Beside them, LARMORX's clean-room original in both modes on the same call (since "
            "2026-11-05), compared with FSL's run and the replica's."
        ).strip()
    return docs


def provenance(ctx: core.Context) -> list[dict]:
    return [fcache.provenance(entry(ctx, x)) for x in EXAMPLES]
