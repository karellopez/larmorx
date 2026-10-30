# SPDX-License-Identifier: Apache-2.0
"""larmorx needs no more memory than the original program for the same call (user,
2026-10-25: "we should fix the memory problem"): each ANTs image program and larmorx's command
(``python -m larmorx ants <program>``, what the ``larmorx`` console script runs) on the same
mid-size case, each process's peak resident memory from ``/usr/bin/time -v``, larmorx in both
D13 modes; its output bit-identical to ANTs' in ``reproduce`` (N4BiasFieldCorrection's and
DenoiseImage's to ANTs with correctly rounded maths preloaded, the build their records compare
with; ``validation/investigations/difftest/memory_pair.py`` measures any call by hand). FSL's
BET programs likewise, against the ``larmorx-nc`` replica (``BET_CASES``).

The allowance is the user's rule of 2026-10-27, "similar" memory: at most ``RATIO`` times the
original's peak plus a fixed cost (a 16-voxel call needs 20 MiB in larmorx, 18 in ANTs;
``docs/findings/ants-image-programs.md``, ``ants-n4.md`` and ``ants-denoise.md``, "Memory"; the
decision log, 2026-10-27). Since 2026-10-30 (the user: "python has it on memory initialization, so
we should count this separately") a call through the Python entry point (``python -m larmorx``,
every call of this file) has as its fixed cost the peak of ``python -m larmorx --version``
measured the same way, in the same environment (:func:`entry_peak`: the interpreter's start-up
and larmorx's Rust core, about 14 MiB here); a call of the standalone binary keeps
``OVERHEAD_MIB`` (:func:`allowance`).
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

nib = pytest.importorskip("nibabel")
np = pytest.importorskip("numpy")
programs = pytest.importorskip("larmorx_validation.parity.ants_programs")

#: The constant the standalone ``larmorx`` binary may need beyond the original (MiB): the gzip
#: writer's two blocks per worker and the runtime's own.
OVERHEAD_MIB = 8.0
#: larmorx's peak may be this many times the original's, plus the fixed cost (:func:`allowance`).
RATIO = 1.10

CASES = [
    ("ResampleImageBySpacing", "128x128x64", "2,2,2", "3 {in} {out} 0.5 0.5 0.5"),
    ("ResampleImage", "128x128x64", "2,2,2", "3 {in} {out} 0.8x0.8x0.8 0 0"),
    ("ThresholdImage", "256x256x128", "1,1,1", "3 {in} {out} 20 60"),
    ("SmoothImage", "256x256x128", "1,1,1", "3 {in} 1.5 {out}"),
    ("MultiplyImages", "256x256x128", "1,1,1", "3 {in} 2.5 {out}"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} m {in} {in}"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} ME {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} GD {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} GE {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} TruncateImageIntensity {in} 0.01 0.999 256"),
    (
        "N4BiasFieldCorrection",
        "128x128x96",
        "1.5,1.5,1.5",
        "-d 3 -i {in} -o {out} -s 2 -c [20x20,0]",
    ),
    (
        "N4BiasFieldCorrection",
        "128x128x96",
        "1.5,1.5,1.5",
        "-d 3 -i {in} -o {out} -s 4 -c [20x20,0]",
    ),
    ("DenoiseImage", "96x96x64", "2,2,2", "-d 3 -i {in} -o {out}"),
    # The data panel's re-validation (2026-10-26): B-spline coefficients of a large input (their
    # z columns were all held at once), and strided axes of short rows (their task list was
    # 16 bytes per row piece for the whole image: a 588-volume series' SmoothImage).
    ("ResampleImage", "192x224x160", "1,1,1", "3 {in} {out} 2x2x2 0 4"),
    ("SmoothImage", "16x1024x1024", "1,1,1", "3 {in} 1.5 {out}"),
]

#: The programs whose records compare with ANTs built with correctly rounded maths (their
#: ``expf``, ``logf`` and ``exp``): ANTs runs with CORE-MATH preloaded for them.
CORRECTLY_ROUNDED = {"N4BiasFieldCorrection", "DenoiseImage"}


def _peak(cmd: list[str], env: dict[str, str], cwd: Path | None = None) -> tuple[int, float]:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd, check=False
    )
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    assert m, p.stderr[-400:]
    return p.returncode, int(m.group(1)) / 1024


def entry_peak(env: dict[str, str], cwd: Path | None = None) -> float:
    """The peak (MiB) of ``python -m larmorx --version`` in ``env``, measured as the calls are:
    the Python entry point's fixed cost (the interpreter's start-up and larmorx's Rust core),
    which every call through it pays whatever its work (user, 2026-10-30)."""
    code, mib = _peak([sys.executable, "-m", "larmorx", "--version"], env, cwd)
    assert code == 0
    return mib


def allowance(original_mib: float, entry_mib: float | None = None) -> float:
    """The most larmorx's call may need (MiB): ``RATIO`` times the original's peak, plus the
    Python entry point's fixed cost for a call through it (``entry_mib``, :func:`entry_peak`),
    or ``OVERHEAD_MIB`` for the standalone binary (``entry_mib`` ``None``)."""
    return RATIO * original_mib + (OVERHEAD_MIB if entry_mib is None else entry_mib)


@pytest.mark.parametrize(("program", "size", "spacing", "args"), CASES)
def test_larmorx_needs_no_more_memory_than_ants(tmp_path, program, size, spacing, args):
    if programs.ants_bin() is None or not (programs.ants_bin() / program).is_file():
        pytest.skip("the ANTs 2.6.5 oracle was not found")
    if not Path("/usr/bin/time").is_file() or shutil.which("python") is None:
        pytest.skip("/usr/bin/time was not found")
    shape = [int(x) for x in size.split("x")]
    n = int(np.prod(shape))
    data = ((np.arange(n, dtype=np.int64) * 7919) % 1000).astype(np.float32) / 10.0
    inp = tmp_path / "in.nii.gz"
    nib.save(
        nib.Nifti1Image(
            data.reshape(shape, order="F"),
            np.diag([*(float(s) for s in spacing.split(",")), 1.0]),
        ),
        str(inp),
    )
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "OMP_NUM_THREADS": "1"}
    words = args.split()

    def argv(out: Path) -> list[str]:
        return [w.format(**{"in": inp, "out": out}) for w in words]

    a_out = tmp_path / "ants.nii.gz"
    ants_env = dict(env)
    if program in CORRECTLY_ROUNDED:
        lib = programs.crmath_preload()
        if lib is None:
            pytest.skip("libcrmath.so (scripts/build_crmath_preload.sh) was not found")
        ants_env["LD_PRELOAD"] = str(lib)
    code, ants_mib = _peak([str(programs.ants_bin() / program), *argv(a_out)], ants_env)
    assert code == 0
    for mode in ("reproduce", "fix"):
        out = tmp_path / f"larmorx-{mode}.nii.gz"
        l_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode}
        entry = entry_peak(l_env)
        code, mib = _peak([sys.executable, "-m", "larmorx", "ants", program, *argv(out)], l_env)
        assert code == 0
        print(f"{program} ({mode}): larmorx {mib:.1f} MiB, ANTs {ants_mib:.1f}, entry {entry:.1f}")
        assert mib <= allowance(ants_mib, entry), (
            f"{program} ({mode}): larmorx {mib:.0f} MiB, ANTs {ants_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            a = np.asarray(nib.load(a_out).dataobj)
            b = np.asarray(nib.load(out).dataobj)
            assert a.tobytes() == b.tobytes(), program


#: FSL's BET programs (FSL 6.0.7.23, ``~/fsl-6.0.7.23``) against larmorx's command (``python -m larmorx
#: fsl <tool>``, which runs the ``larmorx-nc`` replica as a separate process; ``/usr/bin/time``
#: reports the larger of the two processes): bet2 with each output, bet's ``-R`` (bet2 run
#: repeatedly, fslstats) and ``-F`` (a 4D series: fslroi, bet2, fslmaths ``-mas``), betsurf on
#: bet2's surface. ``{in}``: a head of 160 x 192 x 160 voxels; ``{in4d}``: 30 volumes of a smaller
#: one, stored as int16; ``{mesh}``: FSL's bet2 surface of ``{in}``; ``{mat}``: an identity.
#: Every output (images decompressed) is FSL's byte for byte in ``reproduce``.
BET_CASES = [
    ("bet2", "{in} out -m"),
    ("bet2", "{in} out -m -s -o -e"),
    ("bet", "{in} out -R -m"),
    ("bet", "{in4d} out -F"),
    ("betsurf", "--t1only -m -s {in} {mesh} {mat} out"),
]


def _bet_inputs(tmp: Path, fb) -> dict[str, Path]:
    head, _ = fb._head(fb.T1_LEVELS, 1, shape=(160, 192, 160))
    paths = {"in": fb._save(head.astype(np.float32), tmp / "head.nii.gz")}
    small, _ = fb._head(fb.T1_LEVELS, 2)
    series = np.stack([small * (1.0 + 0.01 * t) for t in range(30)], axis=-1)
    paths["in4d"] = fb._save(np.round(series).astype(np.int16), tmp / "series.nii.gz")
    prep = tmp / "prep"
    prep.mkdir()
    p = subprocess.run(
        [str(fb.fsl_dir() / "bin" / "bet2"), str(paths["in"]), "t1", "-e", "-n"],
        cwd=prep,
        env=fb.environment(),
        capture_output=True,
        check=False,
    )
    assert p.returncode == 0, p.stderr
    paths["mesh"] = prep / "t1_mesh.vtk"
    paths["mat"] = tmp / "identity.mat"
    paths["mat"].write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
    return paths


def _contents(d: Path) -> dict[str, bytes]:
    import gzip

    out = {}
    for f in sorted(d.iterdir()):
        raw = f.read_bytes()
        out[f.name] = gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw
    return out


@pytest.mark.parametrize(("tool", "args"), BET_CASES)
def test_larmorx_needs_no_more_memory_than_fsl(tmp_path, tool, args):
    fb = pytest.importorskip("larmorx_validation.parity.fsl_bet")
    if fb.fsl_dir() is None:
        pytest.skip("FSL (~/fsl-6.0.7.23, LARMORX_FSLDIR) was not found")
    if fb.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    paths = _bet_inputs(tmp_path, fb)
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fb.environment(), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _peak([str(fb.fsl_dir() / "bin" / tool), *words], env, f_dir)
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(fb.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", tool, *words], l_env, r_dir)
        assert code == 0
        print(f"{tool} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}")
        assert mib <= allowance(fsl_mib, entry), (
            f"{tool} {args} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _contents(f_dir) == _contents(r_dir), f"{tool} {args}"


#: The kinds of call the data panel's re-validation brought (2026-10-26, part a): Atropos,
#: antsAI, antsAffineInitializer, antsApplyTransforms on a 4D series through a displacement
#: field and antsRegistration's linear and SyN stages, each ANTs program with correctly rounded
#: maths preloaded (the build their records compare with) and larmorx at one thread, on a head of
#: 128 x 128 x 96 voxels (``{fixed}``, ``{moving}`` two voxels apart, ``{mask}``), a 20-volume
#: series (``{series}``) and a smooth displacement field (``{warp}``); ``{out}`` is the run's own
#: output directory. Every output is ANTs' byte for byte in ``reproduce`` (images: header and
#: values as stored). antsAI and antsAffineInitializer keep their images in float, as ANTs does
#: (2026-10-28: 96 and 91 MiB against ANTs' 92 and 90 on these inputs; they needed 127 and 110
#: while the metric held them in double: ``docs/findings/ants-ai.md``, "Memory and speed").
MORE_CASES = [
    (
        "Atropos",
        "-d 3 -a {fixed} -x {mask} -i KMeans[3] -c [5,0] -m [0.1,1x1x1] "
        "-o [{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
    ),
    (
        "antsAI",
        "-d 3 -m Mattes[{fixed},{moving},32,Regular,0.25] -t Rigid[0.1] -s [30,0.12] -c 10 "
        "-o {out}/ai.mat",
    ),
    ("antsAffineInitializer", "3 {fixed} {moving} {out}/init.mat 15 0.1 0 10"),
    (
        "antsApplyTransforms",
        "-d 3 -e 3 -i {series} -r {fixed} -t {warp} -n LanczosWindowedSinc --float 1 "
        "-o {out}/o.nii.gz",
    ),
    (
        "antsRegistration",
        "-d 3 -o [{out}/r] -t Rigid[0.1] -m MI[{fixed},{moving},1,32,Regular,0.25] "
        "-c [20x10,1e-6,10] -f 2x1 -s 1x0vox -t SyN[0.1,3,0] -m CC[{fixed},{moving},1,2] "
        "-c [4x2,1e-6,10] -f 2x1 -s 1x0vox",
    ),
    (
        "antsRegistration",
        "-d 3 -o [{out}/b] -t BSpline[0.1,4x4x4] -m MeanSquares[{fixed},{moving},1] "
        "-c [3x2,1e-6,10] -f 2x1 -s 1x0vox",
    ),
]


def _more_inputs(d: Path) -> dict[str, str]:
    rng = np.random.default_rng(11)
    shape = (128, 128, 96)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 64) ** 2 + (j - 64) ** 2 + ((k - 48) * 1.3) ** 2)
    head = np.where(r < 50, 400 + i, 20) + np.where(r < 25, 300, 0) + np.where(r < 12, 150, 0)
    img = (head + rng.normal(0, 10, shape)).astype(np.float32)
    a = np.diag([1.5, 1.5, 1.5, 1.0])
    out = {}

    def save(name: str, data, zooms=None, intent=None) -> None:
        im = nib.Nifti1Image(data, a)
        if zooms is not None:
            im.header.set_zooms(zooms)
        if intent is not None:
            im.header.set_intent(intent)
        nib.save(im, d / name)
        out[name.split(".")[0]] = str(d / name)

    save("fixed.nii.gz", img)
    save("moving.nii.gz", np.roll(img, 2, axis=0))
    save("mask.nii.gz", (r < 50).astype(np.uint8))
    series = np.stack([np.roll(img, t % 2, axis=1) * (1 + 0.01 * t) for t in range(20)], -1)
    save("series.nii.gz", series.astype(np.float32), zooms=(1.5, 1.5, 1.5, 2.0))
    field = np.zeros((*shape, 1, 3), np.float32)
    field[..., 0, 0] = 1.5 * (i / shape[0]) - 0.75
    field[..., 0, 1] = 0.75 * (k / shape[2])
    save("warp.nii.gz", field, intent="vector")
    return out


def _stored(d: Path) -> dict[str, bytes]:
    """Each output file's contents (decompressed; a NIfTI file's four extension bytes after its
    header aside, as ``validation/investigations/panel-ants/memory_real.py`` compares)."""
    out = {}
    for name, raw in _contents(d).items():
        out[name] = raw[:348] + raw[352:] if name.endswith((".nii", ".nii.gz")) else raw
    return out


@pytest.mark.parametrize(("program", "args"), MORE_CASES)
def test_larmorx_needs_no_more_memory_than_ants_more(tmp_path, program, args):
    if programs.ants_bin() is None or not (programs.ants_bin() / program).is_file():
        pytest.skip("the ANTs 2.6.5 oracle was not found")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    lib = programs.crmath_preload()
    if lib is None:
        pytest.skip("libcrmath.so (scripts/build_crmath_preload.sh) was not found")
    inputs = tmp_path / "in"
    inputs.mkdir()
    paths = _more_inputs(inputs)
    env = {
        **{k: v for k, v in os.environ.items() if k not in ("RAYON_NUM_THREADS", "NSLOTS")},
        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1",
        "OMP_NUM_THREADS": "1",
        "ANTS_RANDOM_SEED": "42",
    }

    def argv(out: Path) -> list[str]:
        return [w.format(**paths, out=out) for w in args.split()]

    a_dir = tmp_path / "ants"
    a_dir.mkdir()
    code, ants_mib = _peak(
        [str(programs.ants_bin() / program), *argv(a_dir)], {**env, "LD_PRELOAD": str(lib)}
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode}
        entry = entry_peak(l_env)
        code, mib = _peak([sys.executable, "-m", "larmorx", "ants", program, *argv(r_dir)], l_env)
        assert code == 0
        print(f"{program} ({mode}): larmorx {mib:.1f} MiB, ANTs {ants_mib:.1f}, entry {entry:.1f}")
        assert mib <= allowance(ants_mib, entry), (
            f"{program} ({mode}): larmorx {mib:.0f} MiB, ANTs {ants_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _stored(a_dir) == _stored(r_dir), program


#: AFNI's programs (26.3.05, the replicas' build) against larmorx's command with the replica
#: (built into larmorx since 2026-10-29: ``python -m larmorx afni <program>`` with
#: ``LARMORX_IMPLEMENTATION=replica``, in the same process), at one thread, on fMRIPrep's calls:
#: 3dTshift on a BOLD series of 64 x 64 x 40 voxels and 160 volumes (``{bold}``), 3dAutomask on
#: its mean (``{mean}``), 3dUnifize ``-T2`` on a head of 160 x 192 x 160 voxels (``{t1}``) and
#: ``-EPI`` on the series. Every output is AFNI's byte for byte in ``reproduce`` (the header's
#: four extension bytes aside: larmorx writes no AFNI history extension).
#:
#: Measured on 2026-10-29 (``validation/investigations/afni-move/memory.py``: AFNI, the Python entry
#: point, the standalone ``larmorx`` binary and the earlier ``larmorx-gpl`` program): the move into
#: larmorx added nothing (the binary within 1.2 MiB of ``larmorx-gpl``). Since 2026-10-30 the
#: replica's 3dUnifize holds what AFNI holds (it held 1.5 times AFNI's peak with ``-T2`` and 2.8
#: times with ``-EPI``: the input kept after its float copy, a copy for each contrast inversion, the
#: masked image after its half-size copy, and with ``-EPI`` the series in and out side by side;
#: ``validation/investigations/specs-memory/README.md``), and the command line no longer imports
#: ``larmorx._replica`` (2.3 MiB). 3dAutomask's image (64 x 64 x 40) is so small that the Python
#: entry point's fixed cost, the interpreter (9.0 MiB) and the Rust core it loads (4.0 MiB),
#: exceeded the earlier fixed 8 MiB beyond AFNI's whole run (7.7 MiB; the entry point 19.6 MiB,
#: ``_core.cli_main`` called alone 18.5, the standalone binary 8.4:
#: ``validation/investigations/specs-memory/entry_memory.py``), a strict expected failure until
#: the user's decision of 2026-10-30 counted that fixed cost apart (:func:`allowance`): measured
#: that day in two runs, 18.7 to 19.3 MiB against AFNI's 7.4 to 7.9 and the entry point's 13.0 to
#: 14.0, within the allowance (21.6 to 22.7 MiB).
AFNI_CASES = [
    ("3dTshift", "-ignore 0 -tzero 1.0 -TR 2s -tpattern alt+z -prefix {out}/ts.nii.gz {bold}"),
    (
        "3dAutomask",
        "-apply_prefix {out}/masked.nii.gz -dilate 1 -prefix {out}/mask.nii.gz {mean}",
    ),
    ("3dUnifize", "-T2 -clfrac 0.2 -prefix {out}/uni.nii.gz -input {t1}"),
    ("3dUnifize", "-EPI -prefix {out}/epi.nii.gz -input {bold}"),
]


def _afni_inputs(d: Path) -> dict[str, str]:
    rng = np.random.default_rng(7)
    out = {}

    def save(name: str, data, zooms) -> None:
        im = nib.Nifti1Image(data, np.diag([*zooms[:3], 1.0]))
        im.header.set_zooms(zooms)
        im.header.set_xyzt_units("mm", "sec")
        nib.save(im, d / name)
        out[name.split(".")[0]] = str(d / name)

    shape = (64, 64, 40)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 32) ** 2 + (j - 32) ** 2 + ((k - 20) * 1.2) ** 2)
    brain = np.where(r < 26, 900.0 + 3 * i, 30.0) + np.where(r < 12, 250.0, 0.0)
    t = np.arange(160)
    bold = brain[..., None] * (1 + 0.01 * np.sin(t / 7.0)) + rng.normal(0, 8, (*shape, 160))
    save("bold.nii.gz", bold.astype(np.float32), (3.0, 3.0, 3.5, 2.0))
    save("mean.nii.gz", bold.mean(axis=-1).astype(np.float32), (3.0, 3.0, 3.5))
    shape = (160, 192, 160)
    i, j, k = np.indices(shape, dtype=np.float32)
    r = np.sqrt((i - 80) ** 2 + (j - 96) ** 2 + ((k - 80) * 1.1) ** 2)
    head = np.where(r < 70, 600.0 + i, 15.0) + np.where(r < 40, 400.0, 0.0)
    head = head + np.where(r < 18, -300.0, 0.0)
    save("t1.nii.gz", (head + rng.normal(0, 12, shape)).astype(np.float32), (1.0, 1.0, 1.0))
    return out


@pytest.mark.parametrize(("program", "args"), AFNI_CASES)
def test_larmorx_needs_no_more_memory_than_afni(tmp_path, program, args):
    ar = pytest.importorskip("larmorx_validation.parity.afni_replica")
    afni = ar.afni_binary(program, "26.3.05")
    if afni is None:
        pytest.skip(f"the AFNI 26.3.05 {program} oracle was not found")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    inputs = tmp_path / "in"
    inputs.mkdir()
    paths = _afni_inputs(inputs)
    home = tmp_path / "home"
    home.mkdir()
    env = {
        **{k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))},
        "OMP_NUM_THREADS": "1",
        "HOME": str(home),
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_NOEXT": "YES",
    }

    def argv(out: Path) -> list[str]:
        return [w.format(**paths, out=out) for w in args.split()]

    a_dir = tmp_path / "afni"
    a_dir.mkdir()
    code, afni_mib = _peak([str(afni), *argv(a_dir)], env, a_dir)
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "LARMORX_IMPLEMENTATION": "replica"}
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak(
            [sys.executable, "-m", "larmorx", "afni", program, *argv(r_dir)], l_env, r_dir
        )
        assert code == 0
        print(f"{program} ({mode}): larmorx {mib:.1f} MiB, AFNI {afni_mib:.1f}, entry {entry:.1f}")
        assert mib <= allowance(afni_mib, entry), (
            f"{program} {args} ({mode}): larmorx {mib:.0f} MiB, AFNI {afni_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _stored(a_dir) == _stored(r_dir), f"{program} {args}"
