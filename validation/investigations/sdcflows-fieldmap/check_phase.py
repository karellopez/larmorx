# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' phase-difference preparation with sdcflows'.

Runs, in this interpreter's environment (each kept one: ``oracles/python-newest`` and the
shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_phase.py

1. Synthetic cases against sdcflows' interfaces (``PhaseMap2rads``, ``SubtractPhases``,
   ``Phasediff2Fieldmap``, ``CheckB0Units``, ``CheckRegister``) and ``delta_te``: the files
   (decompressed bytes), the metadata, or the same exception; phase maps in uint16, int16,
   scaled int16, float32 and NIfTI-2; echo times in either order; one phase map; every
   metadata form ``delta_te`` reads and its messages; field maps in Hz and rad/s (float32,
   int16 re-quantised, scaled); images with the same affine, a moved affine (rewritten), a
   shape or orientation mismatch; SDC-04 (a constant phase map) and SDC-09 (no ``EchoTime``)
   reproduced and corrected.
2. fMRIPrep 25.2.6's own calls (the oracle runs' working directories): every ``phmap2rads``,
   ``calc_phdiff`` and ``compfmap`` node's inputs through larmorx, against the files fMRIPrep
   wrote (ds001600's three estimators, ds002422's two, ds002603, ds005454).
3. The whole phase-difference workflow (``phdiff_workflow``, PRELUDE through ``lx.fsl.prelude``)
   on those runs' inputs, against fMRIPrep's field map: INFO lines (PRELUDE's clean-room
   original is validated within its thresholds, ``docs/validation/fsl-prelude.md``; with the
   replica installed the comparison is exact).

One line per case; exit status 1 if any of 1 or 2 differs.
"""

import gzip
import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from nipype.utils.filemanip import loadpkl
from sdcflows.interfaces.fmap import (
    CheckB0Units,
    CheckRegister,
    Phasediff2Fieldmap,
    PhaseMap2rads,
    SubtractPhases,
)
from sdcflows.utils.phasemanip import delta_te as sdc_delta_te

from larmorx import sdcflows as lxs

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
BIDS = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
FAILS: list[str] = []
REPRODUCE = {"upstream_defects": "reproduce"}


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def exc(e):
    return None if e is None else (type(e).__name__, str(e).splitlines()[0] if str(e) else "")


def write(path, data, affine, dtype=np.float32, slope=None, inter=None, version=1):
    klass = nb.Nifti2Image if version == 2 else nb.Nifti1Image
    img = klass(np.asarray(data).astype(dtype), affine, dtype=np.dtype(dtype))
    img.header.set_qform(affine, 1)
    img.header.set_sform(affine, 1)
    img.header.set_xyzt_units("mm", "sec")
    if slope is not None:
        img.header.set_slope_inter(slope, 0.0 if inter is None else inter)
    img.to_filename(path)
    return Path(path)


def run_node(node):
    with tempfile.TemporaryDirectory() as t, warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            outs = node.run(cwd=t).outputs
        except Exception as e:  # the exception is the result
            return None, exc(e), {}
        files = {}
        for k, v in outs.get().items():
            if isinstance(v, str) and v.endswith((".nii", ".nii.gz")) and Path(v).exists():
                files[k] = (Path(v).name, raw(v))
            elif isinstance(v, list):
                files[k] = [(Path(x).name, raw(x)) for x in v]
        return outs.get(), None, files


def run_mine(fn):
    with tempfile.TemporaryDirectory() as t, warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            res = fn(t)
        except Exception as e:  # the exception is the result
            return None, exc(e), {}
        files = {}
        for k in ("out_file", "mag_files", "fmap_files"):
            v = getattr(res, k, None)
            if isinstance(v, Path) and v.exists():
                files[k] = (v.name, raw(v))
            elif isinstance(v, tuple):
                files[k] = [(Path(x).name, raw(x)) for x in v]
        return res, None, files


def compare(name, ref, mine, key_map, meta=False):
    (ro, re_, rf), (mo, me, mf) = ref, mine
    if re_ or me:
        report(name, re_ == me, f"{re_} vs {me}")
        return
    checks = {}
    for rk, mk in key_map.items():
        checks[rk] = rf.get(rk) == mf.get(mk)
    if meta:
        checks["metadata"] = ro["metadata"] == mo.metadata
    bad = [k for k, v in checks.items() if not v]
    report(name, not bad, f"differ: {bad}" if bad else "")


def rads_case(name, path, **kw):
    ref = run_node(PhaseMap2rads(in_file=str(path)))
    mine = run_mine(lambda t: lxs.au2rads(str(path), out_dir=t, **kw))
    compare(f"PhaseMap2rads {name}", ref, mine, {"out_file": "out_file"})


def sub_case(name, phases, metas, **kw):
    node = SubtractPhases(in_phases=[str(p) for p in phases], in_meta=[dict(m) for m in metas])
    ref = run_node(node)
    mine = run_mine(lambda t: lxs.subtract_phases([str(p) for p in phases], metas, out_dir=t, **kw))
    if len(phases) == 1 and ref[1] is None and mine[1] is None:
        ok = (
            ref[0]["phase_diff"] == str(mine[0].out_file) and ref[0]["metadata"] == mine[0].metadata
        )
        report(f"SubtractPhases {name}", ok)
        return
    if ref[1] is None and mine[1] is None:
        ref[2]["out_file"] = ref[2].get("phase_diff")
    compare(f"SubtractPhases {name}", ref, mine, {"out_file": "out_file"}, meta=ref[1] is None)


def check_synthetic(tmp: Path) -> None:
    rng = np.random.default_rng(4)
    aff = np.diag([3.0, 3.0, 4.0, 1.0])
    aff[:3, 3] = (-90, -100, -40)
    las = aff.copy()
    las[0, 0], las[0, 3] = -3.0, 90
    shape = (20, 22, 10)
    ph = rng.integers(-4096, 4095, shape)
    p_i16 = write(tmp / "ph_i16.nii.gz", ph, las, np.int16)
    p_u16 = write(tmp / "ph_u16.nii.gz", ph + 4096, aff, np.uint16)
    p_s16 = write(tmp / "ph_s16.nii.gz", ph, las, np.int16, slope=0.001534, inter=0.0)
    p_f32 = write(tmp / "ph_f32.nii.gz", rng.uniform(-np.pi, np.pi, shape), aff)
    p_n2 = write(tmp / "ph_n2.nii.gz", ph, las, np.int16, version=2)
    p_4d = write(tmp / "ph_4d.nii.gz", ph[..., None], las, np.int16)
    p_const = write(tmp / "ph_const.nii.gz", np.full(shape, 7), aff, np.int16)
    for name, p in [
        ("int16", p_i16),
        ("uint16", p_u16),
        ("scaled int16", p_s16),
        ("float32", p_f32),
        ("NIfTI-2", p_n2),
        ("4D singleton", p_4d),
    ]:
        rads_case(name, p)
    rads_case("constant (SDC-04 reproduced)", p_const, **REPRODUCE)
    mine = run_mine(lambda t: lxs.au2rads(str(p_const), out_dir=t))
    report(
        "PhaseMap2rads constant (SDC-04 corrected: an error)",
        mine[1] is not None and mine[1][0] == "ValueError" and "SDC-04" in mine[1][1],
    )
    # two phase maps
    r1 = lxs.au2rads(str(p_i16), out_dir=tmp).out_file
    r2 = lxs.au2rads(str(write(tmp / "ph2.nii.gz", np.roll(ph, 3, 0), las, np.int16)), out_dir=tmp)
    r2 = r2.out_file
    m1 = {"EchoTime": 0.00492, "EchoTime1": 99, "Units": "rad"}
    m2 = {"EchoTime": 0.00738, "IntendedFor": ["func/x.nii.gz"]}
    sub_case("two phases", [r1, r2], [m1, m2])
    sub_case("two phases, longer echo first", [r2, r1], [m2, m1])
    sub_case("equal echo times", [r1, r2], [m1, dict(m2, EchoTime=0.00492)])
    sub_case("one phase difference (passed through)", [r1], [m1])
    sub_case("lengths differ", [r1, r2], [m1])
    sub_case("no EchoTime (SDC-09 reproduced)", [r1, r2], [m1, {"Units": "rad"}], **REPRODUCE)
    sub_case("no EchoTimes (SDC-09 reproduced)", [r1, r2], [{}, {}], **REPRODUCE)
    mine = run_mine(lambda t: lxs.subtract_phases([str(r1), str(r2)], [m1, {}], out_dir=t))
    report(
        "SubtractPhases no EchoTime (SDC-09 corrected: a clear error)",
        mine[1] is not None and mine[1][0] == "ValueError" and "EchoTime" in mine[1][1],
    )
    # delta_te
    for meta in [
        {"EchoTime1": 0.00492, "EchoTime2": 0.00738},
        {"EchoTime1": "0.005", "EchoTime2": 0.0074},
        {"EchoTime2": 0.00738},
        {"EchoTime1": 0, "EchoTime2": 0.00738},
        {"EchoTime1": "x", "EchoTime2": 0.00738},
        {"EchoTimeDifference": -0.00246},
        {"EchoTimeDifference": "0.00246"},
        {"EchoTimeDifference": "abc"},
        {},
    ]:
        out = []
        for fn in (sdc_delta_te, lxs.delta_te):
            try:
                out.append(("ok", repr(fn(dict(meta)))))
            except Exception as e:  # the exception is the result
                out.append(exc(e))
        report(f"delta_te {meta}", out[0] == out[1], f"{out[0]} vs {out[1]}")
    # phase difference to Hz
    meta = {"EchoTime1": 0.00492, "EchoTime2": 0.00738}
    unw = write(tmp / "unw.nii.gz", rng.normal(0, 3, shape), las)
    unw16 = write(tmp / "unw16.nii.gz", rng.normal(0, 3000, shape), aff, np.int16, slope=0.001)
    for name, p in [("float32", unw), ("scaled int16", unw16)]:
        ref = run_node(Phasediff2Fieldmap(in_file=str(p), metadata=meta))
        mine = run_mine(lambda t, p=p: lxs.phdiff2fmap(str(p), metadata=meta, out_dir=t))
        compare(f"Phasediff2Fieldmap {name}", ref, mine, {"out_file": "out_file"})
    ref = run_node(Phasediff2Fieldmap(in_file=str(unw), metadata={}))
    mine = run_mine(lambda t: lxs.phdiff2fmap(str(unw), metadata={}, out_dir=t))
    compare("Phasediff2Fieldmap no echo times", ref, mine, {"out_file": "out_file"})
    # field-map units
    fm = rng.normal(0, 300, shape)
    for name, p in [
        ("float32", write(tmp / "fm_f32.nii.gz", fm, aff)),
        ("int16 (re-quantised)", write(tmp / "fm_i16.nii.gz", fm, las, np.int16)),
        ("scaled int16", write(tmp / "fm_s16.nii.gz", fm * 10, las, np.int16, slope=0.1)),
        ("float64 NIfTI-2", write(tmp / "fm_n2.nii.gz", fm, aff, np.float64, version=2)),
    ]:
        for units in ("rad/s", "Hz"):
            ref = run_node(CheckB0Units(in_file=str(p), units=units))
            mine = run_mine(lambda t, p=p, u=units: lxs.check_b0_units(str(p), u, out_dir=t))
            if units == "Hz":
                ok = ref[1] is None and ref[0]["out_file"] == str(mine[0].out_file)
                report(f"CheckB0Units {name} Hz (passed through)", ok)
            else:
                compare(f"CheckB0Units {name} rad/s", ref, mine, {"out_file": "out_file"})
    # geometry and affines
    mag = write(tmp / "mag.nii.gz", rng.uniform(0, 900, shape), las, np.int16)
    moved = las.copy()
    moved[:3, 3] += (0.004, -0.002, 0.003)
    fm_moved = write(tmp / "fm_moved.nii.gz", rng.normal(0, 2, shape), moved, np.int16, slope=0.01)
    fm_same = write(tmp / "fm_same.nii.gz", rng.normal(0, 2, shape), las)
    fm_far = write(
        tmp / "fm_far.nii.gz", rng.normal(0, 2, shape), las + np.c_[np.zeros((4, 3)), [5, 0, 0, 0]]
    )
    fm_shape = write(tmp / "fm_shape.nii.gz", rng.normal(0, 2, (20, 22, 9)), las)
    fm_ras = write(tmp / "fm_ras.nii.gz", rng.normal(0, 2, shape), aff)
    mag2 = write(tmp / "mag2.nii.gz", rng.uniform(0, 900, shape), las, np.int16)
    for name, mags, fmaps in [
        ("same affines", [mag], [fm_same]),
        ("a field map's affine moved by microns (kept)", [mag], [fm_moved]),
        ("a field map 5 mm away (rewritten)", [mag], [fm_far]),
        ("two magnitudes and two field maps", [mag, mag2], [fm_same, fm_far]),
        ("shape mismatch", [mag], [fm_shape]),
        ("orientation mismatch", [mag], [fm_ras]),
        ("field maps' shape mismatch", [mag], [fm_same, fm_shape]),
    ]:
        report(f"CheckRegister {name}", *register_case(mags, fmaps))


def register_case(mags, fmaps):
    """CheckRegister against check_register: each output passed through (the input's path) or
    rewritten (the same file name and bytes), or the same exception."""
    with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:
        err_ref = err = None
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                ref = (
                    CheckRegister(
                        mag_files=[str(m) for m in mags], fmap_files=[str(f) for f in fmaps]
                    )
                    .run(cwd=t1)
                    .outputs
                )
        except Exception as e:  # the exception is the result
            err_ref = exc(e)
        try:
            mine = lxs.check_register([str(m) for m in mags], [str(f) for f in fmaps], out_dir=t2)
        except Exception as e:  # the exception is the result
            err = exc(e)
        if err_ref or err:
            return err_ref == err, f"{err_ref} vs {err}"
        bad = []
        for key, inputs in (("mag_files", mags), ("fmap_files", fmaps)):
            want = getattr(ref, key)
            want = want if isinstance(want, list) else [want]
            for w, g, src in zip(want, getattr(mine, key), inputs, strict=True):
                if Path(w) == Path(src):
                    if Path(g) != Path(src):
                        bad.append(f"{Path(src).name} kept by sdcflows")
                elif Path(w).name != Path(g).name or raw(w) != raw(g):
                    bad.append(f"{Path(src).name} rewritten differently")
        return not bad, "; ".join(bad)


def host(run: str, p: str) -> str:
    base = R / f"{run}-noreconall"
    for c, h in (
        ("/work/", base / "work"),
        ("/out/", base / "out"),
        ("/data/", BIDS / run.split("-")[0]),
    ):
        if p.startswith(c):
            return str(h / p[len(c) :])
    return p


def node_inputs(path: Path) -> dict:
    return loadpkl(path)


def check_oracle() -> None:
    runs = [
        ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00000"),
        ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00001"),
        ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00002"),
        ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
        ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00001"),
        ("ds002603-sub-01", "sub_01_wf/fmap_preproc_wf/wf_auto_00000"),
        ("ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ]
    for run, node in runs:
        wf = R / f"{run}-noreconall" / "work" / "fmriprep_25_2_wf" / node / "phdiff_wf"
        if not wf.exists():
            print(f"skipped (no oracle run here): {run}")
            continue
        tag = f"{run} {node.rsplit('/', 1)[-1]}"
        with tempfile.TemporaryDirectory() as t:
            # phmap2rads, per phase map
            rads = []
            for d in sorted((wf / "phmap2rads" / "mapflow").glob("_phmap2rads*")):
                inp = host(run, node_inputs(d / "_inputs.pklz")["in_file"])
                want = sorted(d.glob("*_rads.nii.gz"))
                got = lxs.au2rads(inp, out_dir=t).out_file
                rads.append(got)
                if want:
                    report(f"{tag} phmap2rads {d.name}", raw(want[0]) == raw(got), want[0].name)
            # calc_phdiff
            ci = node_inputs(wf / "calc_phdiff" / "_inputs.pklz")
            phases = [host(run, p) for p in ci["in_phases"]]
            res = lxs.subtract_phases(phases, ci["in_meta"], out_dir=t)
            want = sorted((wf / "calc_phdiff").glob("*_phdiff.nii.gz"))
            # calc_phdiff's output metadata, as compfmap received them
            fi = node_inputs(wf / "compfmap" / "_inputs.pklz")
            ok = res.metadata == fi["metadata"]
            if want:
                ok = ok and raw(want[0]) == raw(res.out_file)
            report(f"{tag} calc_phdiff", ok, f"{len(phases)} phase map(s)")
            # compfmap from fMRIPrep's own PRELUDE output
            unw = host(run, fi["in_file"])
            want = sorted((wf / "compfmap").glob("*_fmap.nii.gz"))
            got = lxs.phdiff2fmap(unw, metadata=fi["metadata"], out_dir=t).out_file
            report(
                f"{tag} compfmap",
                bool(want) and raw(want[0]) == raw(got),
                want[0].name if want else "",
            )
            # the whole workflow with lx.fsl.prelude
            pi = node_inputs(wf / "prelude" / "_inputs.pklz")
            wf_in = [
                host(run, node_inputs(d / "_inputs.pklz")["in_file"])
                for d in sorted((wf / "phmap2rads" / "mapflow").glob("_phmap2rads*"))
            ]
            out = lxs.phdiff_workflow(
                host(run, pi["magnitude_file"]),
                wf_in,
                ci["in_meta"],
                host(run, pi["mask_file"]),
                out_dir=Path(t) / "wf",
                n_threads=8,
            )
            ref = nb.load(want[0]).get_fdata()
            mine = np.asarray(out.fieldmap.image.data, dtype=np.float64)
            d = np.abs(ref - mine)
            ha, hb = nb.load(want[0]).header, nb.load(out.fieldmap.out_file).header
            fields = [k for k in ha.keys() if ha[k].tobytes() != hb[k].tobytes()]  # noqa: SIM118
            print(
                f"INFO {tag} phdiff_workflow: max |diff| {np.nanmax(d):.4g} Hz, "
                f"{np.mean(d == 0) * 100:.2f} % voxels identical; header fields that differ: "
                f"{fields or 'none'} (descrip is the PRELUDE implementation's own)"
            )


def main() -> int:
    print(f"numpy {np.__version__}")
    with tempfile.TemporaryDirectory() as tmp:
        check_synthetic(Path(tmp))
    check_oracle()
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
