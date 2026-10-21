# SPDX-License-Identifier: Apache-2.0
"""3dTshift's evidence: fMRIPrep's slice-timing correction of a real BOLD run by AFNI 25.2.09,
by fMRIPrep's own AFNI, and by both larmorx implementations (the clean-room original and the
GPL replica, each in its default and ``reproduce`` mode); a deliberate correction (AFNI-01) on
the same run; the two records' cases; speed.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs as a separate program, as everywhere in
the validation package (Apache-2.0).
"""

from __future__ import annotations

import json
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import core
from larmorx_validation.figures import cache as fcache

TOOL = "afni-tshift"
CASE = "fmriprep-ds005040-clip-1"
RUN = "ds005040-sub-002-noreconall"
BOLD = "openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz"
NODE = (
    "work/fmriprep_25_2_wf/sub_002_wf/bold_task_clip_run_1_wf/bold_native_wf/bold_stc_wf/"
    "slice_timing_correction"
)
FMRIPREP_OUT = "sub-002_task-clip_run-1_bold_tshift.nii.gz"
MASK = "out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz"
NEG = "negative-slope.nii.gz"
#: (label, implementation, mode, threads): every larmorx run of an input
LARMORX_RUNS = [
    ("original", "original", "fix", None),
    ("original-1", "original", "fix", "1"),
    ("original-reproduce", "original", "reproduce", None),
    ("replica", "replica", "fix", None),
    ("replica-1", "replica", "fix", "1"),
    ("replica-reproduce", "replica", "reproduce", None),
]


def _oracles() -> Path:
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    if ws is None:
        raise FileNotFoundError("no workspace with oracles/")
    return ws / "oracles"


def negative_slope(src: Path, dst: Path) -> None:
    """The same values stored with a negative scale factor: the stored integers negated and
    ``scl_slope`` negated (a legal NIfTI encoding of the same data; AFNI-01's condition). The
    rest of the file (header, extensions) is the input's, byte for byte."""
    import gzip

    import nibabel as nib

    img = nib.load(str(src))
    raw = np.asanyarray(img.dataobj.get_unscaled())
    if raw.min() <= np.iinfo(raw.dtype).min:
        raise ValueError("the input's most negative value cannot be negated")
    hdr = img.header
    offset = int(img.dataobj.offset)
    with gzip.open(src, "rb") as f:
        head = bytearray(f.read(offset))
    # scl_slope: the float32 at byte 112 of the header, in the file's byte order
    fmt = np.dtype(np.float32).newbyteorder(hdr.endianness)
    head[112:116] = np.array([-float(img.dataobj.slope)], dtype=fmt).tobytes()
    data = (-raw).astype(raw.dtype.newbyteorder(hdr.endianness)).tobytes(order="F")
    with open(dst, "wb") as f, gzip.GzipFile(fileobj=f, mode="wb", mtime=0) as g:
        g.write(bytes(head) + data)
    back = nib.load(str(dst))
    assert np.array_equal(np.asanyarray(back.dataobj.get_unscaled()), -raw)
    assert float(back.dataobj.slope) == -float(img.dataobj.slope)
    assert np.array_equal(np.asanyarray(back.dataobj), np.asanyarray(img.dataobj))


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.afni_tshift import (
        _environment,
        afni_binary,
        afni_version,
        replica_binary,
        replica_version,
    )

    ora = _oracles()
    run = ora / "fmriprep-25.2.6" / RUN
    afni, gpl = afni_binary(), replica_binary()
    if afni is None or gpl is None or not run.is_dir():
        raise FileNotFoundError("the AFNI oracle, the larmorx-gpl replica or the fMRIPrep run")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    bold = e.input("BOLD", Path(td.get(BOLD)))
    e.input("fMRIPrep's output", run / NODE / FMRIPREP_OUT)
    side = json.loads(Path(td.get(BOLD.replace(".nii.gz", ".json"))).read_text(encoding="utf-8"))
    times = [float(t) for t in side["SliceTiming"]]
    if str(side.get("SliceEncodingDirection", "")).endswith("-"):
        times = times[::-1]
    # as nipype's TShift writes it, next to the node's input
    (e.dir / "slice_timing.1D").write_text("\t".join(str(t) for t in times), encoding="utf-8")
    neg = e.dir / NEG
    negative_slope(bold, neg)
    e.input("BOLD with a negative scale factor (made from BOLD)", neg)
    # the record's environment: HOME an empty directory (no .afnirc), AFNI's log file and
    # type warnings off
    afni_env = {k: v for k, v in _environment().items() if k == "HOME" or k.startswith("AFNI_")}
    unset = ("LD_PRELOAD", "OMP_NUM_THREADS", "LARMORX_UPSTREAM_DEFECTS", "LARMORX_IMPLEMENTATION")

    def argv(prefix: str, inp: Path) -> list[str]:
        out = []
        for a in shlex.split(command)[1:]:
            if a.endswith(".nii.gz") and a.startswith("/work/"):
                a = str(inp)
            elif a == "sub-002_task-clip_run-1_bold_tshift.nii.gz":
                a = prefix
            elif a == "@slice_timing.1D":
                a = f"@{e.dir / 'slice_timing.1D'}"
            out.append(a)
        return out

    for tag, inp in (("", bold), ("neg-", neg)):
        e.run(f"{tag}afni", [afni, *argv(str(e.dir / f"{tag}afni.nii.gz"), inp)], env=afni_env)
        for label, impl, mode, threads in LARMORX_RUNS:
            if tag and label.endswith("-1"):
                continue
            env = {**afni_env, "LARMORX_UPSTREAM_DEFECTS": mode}
            if threads:
                env["OMP_NUM_THREADS"] = threads
            out = str(e.dir / f"{tag}{label}.nii.gz")
            if impl == "replica":
                cmd = [gpl, "afni", "3dTshift", *argv(out, inp)]
            else:
                env["LARMORX_IMPLEMENTATION"] = "original"
                cmd = fcache.larmorx_cli("afni", "3dTshift", *argv(out, inp))
            e.run(f"{tag}{label}", cmd, env=env, unset=unset)
    e.finish(
        versions={"afni": afni_version(), "larmorx-gpl": replica_version()},
        command=command,
        slice_timing=times,
        tr=float(side["RepetitionTime"]),
        settings="fMRIPrep 25.2.6's 3dTshift node (slice_timing_correction) of ds005040 sub-002 "
        "task-clip run-1: its arguments from command.txt, the slice times written as nipype "
        "writes them; larmorx with all logical CPUs (OMP_NUM_THREADS unset) and with one; the "
        "same on the run re-stored with a negative scale factor (AFNI-01)",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's slice-timing correction of ds005040 sub-002 task-clip run-1: AFNI, fMRIPrep, "
        "the clean-room original and the replica, default and reproduce",
        _compute,
    )


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    e = entry(core.Context(repo=REPO, site=Path("."), cache=default_cache()))
    print("done", e.dir)
