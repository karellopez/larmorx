# SPDX-License-Identifier: Apache-2.0
"""larmorx needs no more memory than the original program for the same call (user,
2026-10-25: "we should fix the memory problem"): each ANTs image program and larmorx's command
(``python -m larmorx ants <program>``, what the ``larmorx`` console script runs) on the same
mid-size case, each process's peak resident memory from ``/usr/bin/time -v``, larmorx in both
D13 modes; its output bit-identical to ANTs' in ``reproduce`` (N4BiasFieldCorrection's and
DenoiseImage's to ANTs with correctly rounded maths preloaded, the build their records compare
with; ``validation/investigations/difftest/memory_pair.py`` measures any call by hand). FSL's
BET programs likewise, against the ``larmorx-nc`` replica (``BET_CASES``), and FLIRT's, FAST's,
topup's and applytopup's, fslmaths' and fslstats' (``FLIRT_CASES``, ``FAST_CASES``,
``TOPUP_CASES``, ``AVW_CASES``), and their clean-room originals (``ORIGINAL_*_CASES``:
fslmaths' and fslstats' since 2026-11-03); prelude's replica (``PRELUDE_CASES``, 2026-11-06) and clean-room original
(``ORIGINAL_PRELUDE_CASES``, 2026-11-06); tedana's t2smap against the ``larmorx-gpl`` replica
(``T2SMAP_CASES``, 2026-11-08).

The allowance is the user's rule of 2026-10-27, "similar" memory: at most ``RATIO`` times the
original's peak plus a fixed cost (a 16-voxel call needs 20 MiB in larmorx, 18 in ANTs;
``docs/findings/ants-image-programs.md``, ``ants-n4.md`` and ``ants-denoise.md``, "Memory"; the
decision log, 2026-10-27). Since 2026-10-30 (the user: "python has it on memory initialization, so
we should count this separately") a call through the Python entry point (``python -m larmorx``,
every call of this file) has as its fixed cost the peak of ``python -m larmorx --version``
measured the same way, in the same environment (:func:`entry_peak`: the interpreter's start-up
and larmorx's Rust core, about 14 MiB here); a call of the standalone binary keeps
``OVERHEAD_MIB`` (:func:`allowance`).

The originals' peaks come from the oracle-output cache (since 2026-11-02, the user: "Didnt we
agreed on running oracles once and then larmorx program everytime?"): the first time each call
of an original runs three times (the cache's confirmation runs, its output places emptied before
each), its peak kept in its entry for this machine and its outputs stored; afterwards the call is
served, its outputs replayed and its peak the median of the three, and only larmorx runs
(:func:`_original_peak`; ``--oracle-cache refresh`` measures again, ``off`` measures every time).
"""

from __future__ import annotations

import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path

import pytest

nib = pytest.importorskip("nibabel")
np = pytest.importorskip("numpy")
programs = pytest.importorskip("larmorx_validation.parity.ants_programs")
oracle_cache = pytest.importorskip("larmorx_validation.oracle_cache")

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
    code, mib, _ = _peak_run(cmd, env, cwd)
    return code, mib


def _peak_run(
    cmd: list[str], env: dict[str, str], cwd: Path | None = None
) -> tuple[int, float, str]:
    """``cmd``'s exit status, peak (MiB) and standard output."""
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd, check=False
    )
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    assert m, p.stderr[-400:]
    return p.returncode, int(m.group(1)) / 1024, p.stdout


def _emptier(d: Path):
    """Empties ``d`` (an original's output place) before a repeated measuring run."""

    def empty() -> None:
        shutil.rmtree(d)
        d.mkdir()

    return empty


def _original_peak(
    cmd: list[str],
    env: dict[str, str],
    family: str,
    reset,
    cwd: Path | None = None,
    volatile: tuple[str, ...] = (),
) -> tuple[int, float]:
    """An original program's exit status and peak (MiB), through the oracle-output cache: served
    (the median of the peaks its entry keeps for this machine; its outputs replayed), or run
    under ``/usr/bin/time``, the first time three times (``reset`` empties its output place before
    each repeat, so each run starts alike), a run the cache never stores once; ``volatile``: what
    its messages hold that changes from run to run (tedana's progress bars)."""
    code, mib, _ = _original_run(cmd, env, family, reset, cwd, volatile)
    return code, mib


def _original_run(
    cmd: list[str],
    env: dict[str, str],
    family: str,
    reset,
    cwd: Path | None = None,
    volatile: tuple[str, ...] = (),
) -> tuple[int, float, str]:
    """:func:`_original_peak` with the original's standard output."""
    policy = oracle_cache.Policy(threads=1, family=family, volatile=volatile)
    peaks = []
    for n in range(3):
        if n:
            reset()
        done = oracle_cache.run(
            cmd, env=env, cwd=cwd, policy=policy, runner="memory-peak", peak=True
        )
        if done.cached:
            return done.returncode, done.peak_mib, done.stdout
        peaks.append(done.peak_mib)
        if done.returncode != 0 or done.peaks_kept in (0, 3):
            break
    return done.returncode, statistics.median(peaks), done.stdout


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
    code, ants_mib = _original_peak(
        [str(programs.ants_bin() / program), *argv(a_out)],
        ants_env,
        "itk",
        lambda: a_out.unlink(missing_ok=True),
    )
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
#: bet's ``-S``, ``-B`` and ``-A2`` and standard_space_roi ``-b`` (2026-11-04: FLIRT to FSL's
#: MNI152_T1_2mm, standard_space_roi, FAST, fslmaths, fslstats, betsurf, every program in the
#: replica's process, one after the other) on the suite's 96 x 112 x 84 head ``{head}`` and its
#: T2-like contrast ``{head2}`` (FSL's standard images from ``$FSLDIR/data``, as FSL reads them).
#: Every output (images decompressed) is FSL's byte for byte in ``reproduce``.
BET_CASES = [
    ("bet2", "{in} out -m"),
    ("bet2", "{in} out -m -s -o -e"),
    ("bet", "{in} out -R -m"),
    ("bet", "{in4d} out -F"),
    ("betsurf", "--t1only -m -s {in} {mesh} {mat} out"),
    # standard_space_roi (its original since 2026-11-05, every program in its process, one
    # after the other) and fslroi (a box and volumes of the int16 series)
    ("standard_space_roi", "{head} out -b"),
    ("fslroi", "{in4d} out 10 30 12 30 5 30 2 20"),
    # bet -S, -B, -A2 (the original's since 2026-11-05, its programs in its process, one after
    # the other as the script runs them)
    ("bet", "{head} out -S -m"),
    ("bet", "{head} out -B"),
    ("bet", "{head} out -A2 {head2}"),
]
#: Calls measured for the replica alone: none since 2026-11-05 (the clean-room catch-up wrote
#: bet -S, -B, -A, -A2's original).
REPLICA_ONLY_BET_CASES: list[tuple[str, str]] = []


def _bet_inputs(tmp: Path, fb) -> dict[str, Path]:
    head, _ = fb._head(fb.T1_LEVELS, 1, shape=(160, 192, 160))
    paths = {"in": fb._save(head.astype(np.float32), tmp / "head.nii.gz")}
    small, _ = fb._head(fb.T1_LEVELS, 2)
    series = np.stack([small * (1.0 + 0.01 * t) for t in range(30)], axis=-1)
    paths["in4d"] = fb._save(np.round(series).astype(np.int16), tmp / "series.nii.gz")
    t1, _ = fb._head(fb.T1_LEVELS, 1)
    t2, _ = fb._head(fb.T2_LEVELS, 2)
    paths["head"] = fb._save(t1, tmp / "head1.nii.gz")
    paths["head2"] = fb._save(t2, tmp / "head2.nii.gz")
    prep = tmp / "prep"
    prep.mkdir()
    p = oracle_cache.run(
        [str(fb.fsl_dir() / "bin" / "bet2"), str(paths["in"]), "t1", "-e", "-n"],
        cwd=prep,
        env=fb.environment(),
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="memory-peak",
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


@pytest.mark.parametrize(("tool", "args"), BET_CASES + REPLICA_ONLY_BET_CASES)
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
    code, fsl_mib = _original_peak(
        [str(fb.fsl_dir() / "bin" / tool), *words], env, "fsl", _emptier(f_dir), f_dir
    )
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


#: FSL's FLIRT (FSL 6.0.7.23) against ``python -m larmorx fsl flirt`` (the ``larmorx-nc``
#: replica, 2026-11-01) on ds005040's fMRIPrep pair (a 3 mm BOLD reference and the 0.8 mm T1w
#: brain): fMRIPrep's boundary-based call (``bbr.sch``, its white-matter mask) and a default
#: 12-dof call, both with ``-out``. The outputs bit-identical in ``reproduce``. FLIRT's other
#: programs that read images (part 2): avscale with the T1w's centre, rmsdiff over its mask;
#: part 3: applyxfm4D of fMRIPrep's 63-volume preprocessed BOLD series onto its reference
#: (trilinear, and spline with its coefficients per volume; the series held in its type and
#: each volume converted alone, as FSL holds it).
FLIRT_CASES = [
    (
        "bbr",
        "flirt",
        "-in {epi} -ref {t1} -out out -omat out.mat -basescale 1 -searchcost bbr -dof 6 "
        "-init {init} -schedule {bbr} -wmseg {wm}",
    ),
    ("dof12", "flirt", "-in {epi} -ref {t1} -out out -omat out.mat"),
    # no output volume: the peak is the reference's blur (each pass's input released)
    ("omat-only", "flirt", "-in {epi} -ref {t1} -omat out.mat"),
    # weights: the weighted filter's temporaries released once used
    ("inweight", "flirt", "-in {epi} -ref {t1} -inweight {epi} -nosearch -omat out.mat"),
    ("avscale-volume", "avscale", "--allparams {init} {t1}"),
    ("rmsdiff-mask", "rmsdiff", "{init} {init} {t1} {wm}"),
    (
        "applyxfm4D-trilinear",
        "applyxfm4D",
        "{bold} {epi} out {init} -singlematrix -interp trilinear",
    ),
    ("applyxfm4D-spline", "applyxfm4D", "{bold} {epi} out {init} -singlematrix -interp spline"),
]


@pytest.mark.parametrize(("name", "program", "args"), FLIRT_CASES)
def test_flirt_replica_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    fr = pytest.importorskip("larmorx_validation.parity.fsl_flirt_replica")
    if fr.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not (fr.fsldir() / "bin" / "flirt").is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    src = fr._source_dir()
    node = fr._bbr_node(src) if src.is_dir() else None
    if node is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    bbr = node.parent
    paths = {
        "epi": src / "out/sub-002/func/sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz",
        "t1": next((bbr / "mask_t1w_brain").glob("*_masked.nii.gz")),
        "wm": next((bbr / "wm_mask").glob("*.nii.gz")),
        "init": bbr / "lta_to_fsl" / "out.mat",
        "bbr": fr.fsldir() / "etc" / "flirtsch" / "bbr.sch",
        "bold": src / "out/sub-002/func/sub-002_task-clip_run-1_desc-preproc_bold.nii.gz",
    }
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fr._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(fr.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(fr.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir)
        assert code == 0
        print(
            f"{program} {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"{program} {name}"


#: FSL's FAST (FSL 6.0.7.23) against ``python -m larmorx fsl fast`` (the ``larmorx-nc`` replica,
#: 2026-11-02): fMRIPrep's call on ds000005's T1w brain (``{t1}``, its fMRIPrep input), the bias
#: field with both its outputs on the 2 mm MNI152NLin2009cAsym T1w (``{mni_t1}``, the suite's
#: derived input), and two channels (its T1w and T2w) with their bias fields. The outputs
#: bit-identical in ``reproduce``, but ``-p``'s probability maps: FSL never writes them outside
#: the mask (FSL-96; the replica writes 0 there in both modes).
FAST_CASES = [
    ("fmriprep", "-o out -I 0 -N -p -g -S 1 {t1}"),
    ("bias", "-o out -b -B {mni_t1}"),
    ("two-channels", "-o out -S 2 -b -B {mni_t2} {mni_t1}"),
]


@pytest.mark.parametrize(("name", "args"), FAST_CASES)
def test_fast_replica_needs_no_more_memory_than_fsl(tmp_path, name, args):
    ff = pytest.importorskip("larmorx_validation.parity.fsl_fast_replica")
    if ff.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not (ff.fsldir() / "bin" / "fast").is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    calls = [(run, node) for run, node in ff.fmriprep_calls() if run == ff.SOURCE_RUN]
    t1 = ff.fmriprep_input(calls[0][0], str(calls[0][1])) if calls else None
    if t1 is None:
        pytest.skip("the ds000005 fMRIPrep oracle run was not found")
    derived = ff.inputs()
    paths = {"t1": t1, "mni_t1": derived / "mni_t1.nii.gz", "mni_t2": derived / "mni_t2.nii.gz"}
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**ff._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(ff.fsldir() / "bin" / "fast"), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(ff.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", "fast", *words], l_env, r_dir)
        assert code == 0
        print(f"fast {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}")
        assert mib <= allowance(fsl_mib, entry), (
            f"fast {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            fsl, ours = _blanked(f_dir), _blanked(r_dir)
            for files in (fsl, ours):
                for key in [k for k in files if "_prob_" in k]:
                    del files[key]
            assert fsl == ours, f"fast {name}"


#: FSL's topup and applytopup (FSL 6.0.7.23) against ``python -m larmorx fsl topup`` and
#: ``applytopup`` (the ``larmorx-nc`` replicas, 2026-11-03): fMRIPrep's topup call on ds005040's
#: pair (``{pair}``, sdcflows' ``b02b0.cnf``, every output), and applytopup's least-squares
#: restoration, Jacobian modulation and variational-Bayes restoration across slices of the
#: half-resolution pair (``{half_up}``, ``{half_dn}``) with the suite's topup outputs (``{tu}``).
#: The outputs bit-identical in ``reproduce``.
TOPUP_CASES = [
    (
        "fmriprep",
        "topup",
        "--config={cnf} --datain={acq} --imain={pair} --out=base --iout=corrected --fout=field "
        "--jacout=jac --logout=topup.log --rbmout=xfm --dfout=warpfield",
    ),
    (
        "lsr",
        "applytopup",
        "--imain={half_up},{half_dn} --inindex=1,2 --datain={acq} --topup={tu} --out=hifi",
    ),
    (
        "jac",
        "applytopup",
        "--imain={half_up} --inindex=1 --datain={acq} --topup={tu} --out=hifi --method=jac",
    ),
    (
        "vb3d",
        "applytopup",
        "--imain={half_up},{half_dn} --inindex=1,2 --datain={acq} --topup={tu} --out=hifi "
        "--method=vb3D",
    ),
]


@pytest.mark.parametrize(("name", "program", "args"), TOPUP_CASES)
def test_topup_replica_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    ft = pytest.importorskip("larmorx_validation.parity.fsl_topup_replica")
    if ft.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not (ft.fsldir() / "bin" / program).is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    if ft._source_node() is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    derived = ft.inputs()
    ft.applytopup_inputs()
    paths = {
        "cnf": ft.workspace() / ft.SDCFLOWS_CONFIG,
        "acq": derived / "acq.txt",
        "pair": derived / "pair.nii.gz",
        "half_up": derived / "half_up.nii.gz",
        "half_dn": derived / "half_dn.nii.gz",
        "tu": ft.topup_outputs("y") / "tu",
    }
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**ft._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(ft.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(ft.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir)
        assert code == 0
        print(
            f"{program} {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"{program} {name}"


#: FSL's fslmaths and fslstats (FSL 6.0.7.23) against ``python -m larmorx fsl fslmaths`` and
#: ``fslstats`` (the ``larmorx-nc`` replicas, 2026-11-03): niworkflows' combination of two masks on
#: ds005040 (``{mask_a}``, ``{mask_b}``: its first combine_masks call), fMRIPrep's goodvoxels
#: steps on ds005040's 63-volume BOLD series in T1w space (``{bold}``: -Tstd; ``{norm}``,
#: ``{smooth}``: the smoothing, division and modal dilation; ``{cov_ribbon}``: the ribbon's
#: nonzero mean; the series' per-volume statistics), and filters on a head of 160 x 192 x 160
#: voxels (``{head}``). The outputs (and fslstats' printed numbers) bit-identical in
#: ``reproduce``.
AVW_CASES = [
    ("combine-masks", "fslmaths", "{mask_a} -mul {mask_b} out"),
    ("tstd", "fslmaths", "{bold} -Tstd out"),
    ("smooth-dild", "fslmaths", "{norm} -s 5 -div {smooth} -dilD out"),
    ("filters", "fslmaths", "{head} -s 2 -kernel boxv 3 -dilM -thrp 10 -bin out -odt char"),
    ("ribbon-mean", "fslstats", "{cov_ribbon} -M"),
    ("series", "fslstats", "-t {bold} -M -S -r"),
]


def _avw_inputs(tmp: Path, fm, fb) -> dict[str, Path] | None:
    calls = [node for run, node in fm.fmriprep_calls() if run == fm.GOODVOXELS_RUN]
    chain = fm._goodvoxels_chain(fm.GOODVOXELS_RUN)
    if not calls or chain is None:
        return None
    words = fm.fmriprep_args(fm.GOODVOXELS_RUN, calls[0])
    head, _ = fb._head(fb.T1_LEVELS, 1, shape=(160, 192, 160))
    return {
        "mask_a": Path(words[0]),
        "mask_b": Path(words[2]),
        "bold": chain / "stdev_volume" / "bold.nii.gz",
        "norm": chain / "cov_ribbon_norm_smooth" / "cov_ribbon_norm.nii.gz",
        "smooth": chain / "cov_ribbon_norm_smooth" / "smooth_norm.nii.gz",
        "cov_ribbon": chain / "cov_ribbon" / "cov_ribbon.nii.gz",
        "head": fb._save(head.astype(np.float32), tmp / "head.nii.gz"),
    }


@pytest.mark.parametrize(("name", "program", "args"), AVW_CASES)
def test_avwutils_replica_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    fm = pytest.importorskip("larmorx_validation.parity.fsl_maths_replica")
    fb = pytest.importorskip("larmorx_validation.parity.fsl_bet")
    if fm.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not (fm.fsldir() / "bin" / program).is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    paths = _avw_inputs(tmp_path, fm, fb)
    if paths is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fm._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib, fsl_out = _original_run(
        [str(fm.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(fm.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib, out = _peak_run(
            [sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir
        )
        assert code == 0
        print(
            f"{program} {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"{program} {name}"
            assert out == fsl_out, f"{program} {name}"


#: FSL's prelude (FSL 6.0.7.23) against ``python -m larmorx fsl prelude`` (the ``larmorx-nc``
#: replica, 2026-11-06): fMRIPrep's call on ds005454-sub-16 (sdcflows' phase-difference workflow:
#: 14,678 regions, the largest of the oracle runs), as it ran and with the slice-by-slice
#: unwrapping, the ramps and every output added. The outputs bit-identical in ``reproduce``.
PRELUDE_CASES = [
    ("fmriprep", ""),
    ("slices-ramps-outputs", "-s --removeramps --savemask=mask -l labels -r raw"),
]


@pytest.mark.parametrize(("name", "extra"), PRELUDE_CASES)
def test_prelude_replica_needs_no_more_memory_than_fsl(tmp_path, name, extra):
    fp = pytest.importorskip("larmorx_validation.parity.fsl_prelude_replica")
    if fp.replica_binary() is None:
        pytest.skip("the larmorx-nc program is not built (crates-nc)")
    if not (fp.fsldir() / "bin" / "prelude").is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    calls = [node for run, node in fp.fmriprep_calls() if run == "ds005454-sub-16-noreconall"]
    if not calls:
        pytest.skip("the ds005454 fMRIPrep oracle run was not found")
    args, files = fp.fmriprep_args("ds005454-sub-16-noreconall", calls[0])
    words = []
    for a in args:
        key, sep, value = a.partition("=")
        words.append(f"{key}={files[value]}" if sep and value in files else a)
    words += extra.split()
    env = {**fp._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(fp.fsldir() / "bin" / "prelude"), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_NC_BIN": str(fp.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", "prelude", *words], l_env, r_dir)
        assert code == 0
        print(
            f"prelude {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"prelude {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"prelude {name}"


#: tedana 26.0.3's t2smap (the newest environment, through the suite's launcher) against
#: ``python -m larmorx tedana t2smap`` (the ``larmorx-gpl`` replica, 2026-11-08): fMRIPrep's call on
#: ds003592-sub-280 (curve fit; and the log-linear fit), at one thread, both D13 modes (the
#: default runs the maths' comparison passes, ``crates-gpl/larmorx-gpl-tedana/src/maths.rs``); the
#: files bit-identical to tedana's in ``reproduce``.
T2SMAP_CASES = [("fmriprep-curvefit", "curvefit"), ("fmriprep-loglin", "loglin")]


@pytest.mark.parametrize(("name", "fit"), T2SMAP_CASES)
def test_t2smap_replica_needs_no_more_memory_than_tedana(tmp_path, name, fit):
    tt = pytest.importorskip("larmorx_validation.parity.tedana_t2smap_replica")
    if tt.replica_binary() is None:
        pytest.skip("the larmorx-gpl program is not built (crates-gpl)")
    prog = tt.launcher("newest")
    if prog is None or not Path("/usr/bin/time").is_file():
        pytest.skip("tedana's newest environment or /usr/bin/time was not found")
    calls = [
        a
        for n, a in tt._investigation("check_stages").real_cases()
        if n == "ds003592-sub-280-rest-curvefit"
    ]
    if not calls:
        pytest.skip("the ds003592 fMRIPrep oracle run was not found")
    words = [fit if a == "curvefit" else a for a in calls[0]] + ["--out-dir", "out"]
    env = tt._base_env()
    t_dir = tmp_path / "tedana"
    t_dir.mkdir()
    code, t_mib = _original_peak(
        [str(prog), *words], env, "python", _emptier(t_dir), t_dir, tt.VOLATILE
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_GPL_BIN": str(tt.replica_binary()),
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
            # one thread, as tedana's default: without t2smap's own --n-threads larmorx-gpl
            # takes the run's thread count (2026-11-11)
            "OMP_NUM_THREADS": "1",
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak(
            [sys.executable, "-m", "larmorx", "tedana", "t2smap", *words], l_env, r_dir
        )
        assert code == 0
        print(
            f"t2smap {name} ({mode}): larmorx {mib:.1f} MiB, tedana {t_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(t_mib, entry), (
            f"t2smap {name} ({mode}): larmorx {mib:.0f} MiB, tedana {t_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert tt.compare_files(t_dir / "out", r_dir / "out") == {}, f"t2smap {name}"


#: PRELUDE's clean-room original (``crates/larmorx-fsl``, 2026-11-06) on the same calls, at one
#: thread: fMRIPrep's call on ds005454-sub-16 as it ran, and with the slice-by-slice unwrapping,
#: the ramps and every output added; the outputs bit-identical to FSL's in ``reproduce``
#: (``docs/validation/fsl-prelude.md``), its memory similar to FSL's (``specs/prelude.md`` §10).
ORIGINAL_PRELUDE_CASES = list(PRELUDE_CASES)


@pytest.mark.parametrize(("name", "extra"), ORIGINAL_PRELUDE_CASES)
def test_prelude_original_needs_no_more_memory_than_fsl(tmp_path, name, extra):
    fo = pytest.importorskip("larmorx_validation.parity.fsl_prelude")
    fv = pytest.importorskip("larmorx_validation.parity.fsl_versions")
    prelude = fv.program("prelude")
    if not prelude.is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    calls = [
        w for run, _node, w in fo.fmriprep_calls() if run.startswith("ds005454-sub-16-noreconall/")
    ]
    if not calls:
        pytest.skip("the ds005454 fMRIPrep oracle run was not found")
    words = [*calls[0], *extra.split()]
    home = tmp_path / "home"
    home.mkdir()
    env = {**fo._base_env(home, {}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak([str(prelude), *words], env, "fsl", _emptier(f_dir), f_dir)
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", "prelude", *words], l_env, r_dir)
        assert code == 0
        print(
            f"prelude original {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"prelude original {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"prelude original {name}"


def _large_phase_map(d: Path, shape: tuple[int, int, int] = (128, 128, 128), noise: float = 0.15):
    """The benchmark's large map (``validation/investigations/fsl-prelude-original/bench_large.py``,
    ``make``): a wrapped smooth phase with noise over an ellipsoid and its magnitude, seed 7; at
    128³ and noise 0.15 its mask holds 66,020 regions and 295,535 constraints."""
    rng = np.random.default_rng(7)
    x, y, z = np.meshgrid(*(np.linspace(-1, 1, n, dtype=np.float32) for n in shape), indexing="ij")
    inside = x**2 + y**2 + z**2 < 0.9
    truth = 30 * x**2 + 20 * y**2 + 12 * x * z + 8 * z
    phase = np.mod(truth + rng.normal(0, noise, shape) + np.pi, 2 * np.pi) - np.pi
    mag = np.where(inside, 1000.0, 20.0) + rng.normal(0, 10, shape)
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    nib.save(nib.Nifti1Image(phase.astype(np.float32), aff), str(d / "ph.nii.gz"))
    nib.save(nib.Nifti1Image(mag.astype(np.float32), aff), str(d / "mag.nii.gz"))


#: PRELUDE's clean-room original on a large map (``_large_phase_map``: 128³, 66,020 regions, the
#: benchmark's, ``docs/benchmarks/fsl-prelude.md``), at one thread, as it is and with the
#: slice-by-slice unwrapping: the merging's structures at their largest (FSL needs about 3 minutes
#: for its first call, the oracle cache serves it afterwards).
ORIGINAL_PRELUDE_LARGE_CASES = [("large-128", ""), ("large-128-slices", "-s")]


@pytest.mark.parametrize(("name", "extra"), ORIGINAL_PRELUDE_LARGE_CASES)
def test_prelude_original_large_map_needs_no_more_memory_than_fsl(tmp_path, name, extra):
    fo = pytest.importorskip("larmorx_validation.parity.fsl_prelude")
    fv = pytest.importorskip("larmorx_validation.parity.fsl_versions")
    prelude = fv.program("prelude")
    if not prelude.is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    home = tmp_path / "home"
    home.mkdir()
    env = {**fo._base_env(home, {}), "OMP_NUM_THREADS": "1"}
    inputs = tmp_path / "inputs"
    inputs.mkdir()
    _large_phase_map(inputs)
    # the inputs named whole (the oracle cache keys them by their content), the outputs in each
    # run's own folder
    words = [
        "-p",
        str(inputs / "ph.nii.gz"),
        "-a",
        str(inputs / "mag.nii.gz"),
        "-o",
        "out",
        *extra.split(),
    ]
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak([str(prelude), *words], env, "fsl", _emptier(f_dir), f_dir)
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", "prelude", *words], l_env, r_dir)
        assert code == 0
        print(
            f"prelude original {name} ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"prelude original {name} ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if mode == "reproduce":
            assert _blanked(f_dir) == _blanked(r_dir), f"prelude original {name}"


#: FAST's clean-room original (``crates/larmorx-fsl``, 2026-11-03) on the same calls, at one
#: thread: its outputs are FSL's within the thresholds of its record
#: (``docs/validation/fsl-fast.md``), its memory similar to FSL's.
ORIGINAL_FAST_CASES = list(FAST_CASES)


@pytest.mark.parametrize(("name", "args"), ORIGINAL_FAST_CASES)
def test_fast_original_needs_no_more_memory_than_fsl(tmp_path, name, args):
    ff = pytest.importorskip("larmorx_validation.parity.fsl_fast_replica")
    if not (ff.fsldir() / "bin" / "fast").is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    calls = [(run, node) for run, node in ff.fmriprep_calls() if run == ff.SOURCE_RUN]
    t1 = ff.fmriprep_input(calls[0][0], str(calls[0][1])) if calls else None
    if t1 is None:
        pytest.skip("the ds000005 fMRIPrep oracle run was not found")
    derived = ff.inputs()
    paths = {"t1": t1, "mni_t1": derived / "mni_t1.nii.gz", "mni_t2": derived / "mni_t2.nii.gz"}
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**ff._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(ff.fsldir() / "bin" / "fast"), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", "fast", *words], l_env, r_dir)
        assert code == 0
        print(
            f"fast {name} original ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, "
            f"entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"fast {name} original ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        assert sorted(p.name for p in f_dir.iterdir()) == sorted(p.name for p in r_dir.iterdir())


#: topup's and applytopup's clean-room original (``crates/larmorx-fsl``, 2026-11-03) on the same
#: calls, at one thread: its outputs are FSL's within the thresholds of its record
#: (``docs/validation/fsl-topup.md``), its memory similar to FSL's.
ORIGINAL_TOPUP_CASES = list(TOPUP_CASES)


@pytest.mark.parametrize(("name", "program", "args"), ORIGINAL_TOPUP_CASES)
def test_topup_original_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    ft = pytest.importorskip("larmorx_validation.parity.fsl_topup_replica")
    if not (ft.fsldir() / "bin" / program).is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    if ft._source_node() is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    derived = ft.inputs()
    ft.applytopup_inputs()
    paths = {
        "cnf": ft.workspace() / ft.SDCFLOWS_CONFIG,
        "acq": derived / "acq.txt",
        "pair": derived / "pair.nii.gz",
        "half_up": derived / "half_up.nii.gz",
        "half_dn": derived / "half_dn.nii.gz",
        "tu": ft.topup_outputs("y") / "tu",
    }
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**ft._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(ft.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir)
        assert code == 0
        print(
            f"{program} {name} original ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, "
            f"entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} original ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        assert sorted(p.name for p in f_dir.iterdir()) == sorted(p.name for p in r_dir.iterdir())


#: fslmaths' and fslstats' clean-room originals (``crates/larmorx-fsl``, 2026-11-03) on the same
#: calls, at one thread (a ``.nii.gz`` output's parallel writer adds about 3 MiB per worker:
#: ``docs/benchmarks/fsl-maths.md``): their outputs are FSL's within the thresholds of their
#: records (``docs/validation/fsl-maths.md``, ``fsl-stats.md``), their memory similar to FSL's.
ORIGINAL_AVW_CASES = list(AVW_CASES)


@pytest.mark.parametrize(("name", "program", "args"), ORIGINAL_AVW_CASES)
def test_avwutils_original_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    fm = pytest.importorskip("larmorx_validation.parity.fsl_maths_replica")
    fb = pytest.importorskip("larmorx_validation.parity.fsl_bet")
    if not (fm.fsldir() / "bin" / program).is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    paths = _avw_inputs(tmp_path, fm, fb)
    if paths is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fm._environment({}), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(fm.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir)
        assert code == 0
        print(
            f"{program} {name} original ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, "
            f"entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} original ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        assert sorted(p.name for p in f_dir.iterdir()) == sorted(p.name for p in r_dir.iterdir())


#: FLIRT's clean-room original (``crates/larmorx-fsl``, 2026-11-02) on the same calls (FLIRT's
#: other programs since part 2): its matrices and images are FSL's within the thresholds of its
#: record (``docs/validation/fsl-flirt.md``), its memory similar to FSL's.
ORIGINAL_FLIRT_CASES = [
    *FLIRT_CASES,
    # part 2: each of FLIRT's other programs and scripts once, on the same pair and fMRIPrep's
    # matrix; {coords}, {pin}, {pref}, {mat2} and {text} are small text inputs the test writes
    ("img2imgcoord", "img2imgcoord", "-src {epi} -dest {t1} -xfm {init} -mm {coords}"),
    ("std2imgcoord", "std2imgcoord", "-img {epi} -std {t1} -xfm {init} -vox {coords}"),
    ("img2stdcoord", "img2stdcoord", "-img {epi} -std {t1} -xfm {init} {coords}"),
    ("makerot", "makerot", "-t 30 -a 1,0.5,0.2 -c 10,20,30 -o rot.mat"),
    ("midtrans-template", "midtrans", "--template={t1} -o mid.mat {init} {mat2}"),
    (
        "pointflirt-vox",
        "pointflirt",
        "-i {pin} -r {pref} -o aff.mat --vox --invol={epi} --refvol={t1}",
    ),
    ("extracttxt", "extracttxt", "value {text} 3"),
    # FSL's script stops at its NameError (FSL-81): exit 1 there and in the original's
    # reproduce; the original's default computes the matrix
    ("aff2rigid", "aff2rigid", "{init} rigid.mat"),
    # three registrations of the BOLD reference to itself with FSL's pairreg schedules
    ("pairreg", "pairreg", "{epi} {epi} {epi} {epi} pair.mat -dof 6"),
]

#: FSL's exit status and the original's in each mode, where a call does not succeed (FSL-81).
ORIGINAL_FLIRT_EXITS = {"aff2rigid": (1, {"reproduce": 1, "fix": 0})}

#: The small text inputs of :data:`ORIGINAL_FLIRT_CASES`.
_FLIRT_TEXTS = {
    "coords": "10 20 30\n-12.5 4 0\n0 0 0\n",
    "pin": "10 12 8\n40 15 9\n12 50 10\n15 18 30\n35 40 25\n",
    "pref": "60 70 50\n150 75 52\n64 180 55\n70 78 120\n140 150 100\n",
    "mat2": "0.99 0.01 0 1.5\n-0.01 0.99 0.02 -2\n0 -0.02 1.01 0.5\n0 0 0 1\n",
    "text": "".join(f"line {k} value {k * 3}\n" for k in range(1, 13)),
}


@pytest.mark.parametrize(("name", "program", "args"), ORIGINAL_FLIRT_CASES)
def test_flirt_original_needs_no_more_memory_than_fsl(tmp_path, name, program, args):
    fr = pytest.importorskip("larmorx_validation.parity.fsl_flirt_replica")
    if not (fr.fsldir() / "bin" / "flirt").is_file() or not Path("/usr/bin/time").is_file():
        pytest.skip("FSL 6.0.7.23 or /usr/bin/time was not found")
    src = fr._source_dir()
    node = fr._bbr_node(src) if src.is_dir() else None
    if node is None:
        pytest.skip("the ds005040 fMRIPrep oracle run was not found")
    bbr = node.parent
    paths = {
        "epi": src / "out/sub-002/func/sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz",
        "t1": next((bbr / "mask_t1w_brain").glob("*_masked.nii.gz")),
        "wm": next((bbr / "wm_mask").glob("*.nii.gz")),
        "init": bbr / "lta_to_fsl" / "out.mat",
        "bbr": fr.fsldir() / "etc" / "flirtsch" / "bbr.sch",
        "bold": src / "out/sub-002/func/sub-002_task-clip_run-1_desc-preproc_bold.nii.gz",
    }
    texts = tmp_path / "texts"
    texts.mkdir()
    for key, text in _FLIRT_TEXTS.items():
        (texts / f"{key}.txt").write_text(text, encoding="utf-8")
        paths[key] = texts / f"{key}.txt"
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fr._environment({}), "OMP_NUM_THREADS": "1"}
    if program == "aff2rigid":
        env["PATH"] = f"{fr.fsldir()}/bin:/usr/bin:/bin"  # the script runs `env fslpython`
    want, codes = ORIGINAL_FLIRT_EXITS.get(program, (0, {"reproduce": 0, "fix": 0}))
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(fr.fsldir() / "bin" / program), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == want
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", program, *words], l_env, r_dir)
        assert code == codes[mode], f"{program} {name} original ({mode}): exit {code}"
        print(
            f"{program} {name} original ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, "
            f"entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{program} {name} original ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        if codes[mode] == want:
            names = sorted(p.name for p in r_dir.iterdir())
            assert sorted(p.name for p in f_dir.iterdir()) == names


def _blanked(d: Path) -> dict[str, bytes]:
    """The files' contents with the NIfTI-1 headers' ``descrip`` blanked (FLIRT writes FSL's build
    string there, the replica its own name)."""
    out = {}
    for name, raw in _contents(d).items():
        if len(raw) >= 348 and int.from_bytes(raw[:4], "little") == 348:
            raw = raw[:148] + b"\0" * 80 + raw[228:]
        out[name] = raw
    return out


#: BET's clean-room original (``crates/larmorx-fsl``, 2026-10-30; betsurf's since 2026-10-31;
#: standard_space_roi's, fslroi's and bet -S, -B, -A2's since 2026-11-05) on the same calls: the
#: original's outputs
#: are FSL's within the thresholds of its record (``docs/validation/fsl-bet.md``), its memory
#: similar to FSL's.
ORIGINAL_BET_CASES = list(BET_CASES)


@pytest.mark.parametrize(("tool", "args"), ORIGINAL_BET_CASES)
def test_bet_original_needs_no_more_memory_than_fsl(tmp_path, tool, args):
    fb = pytest.importorskip("larmorx_validation.parity.fsl_bet")
    if fb.fsl_dir() is None:
        pytest.skip("FSL (~/fsl-6.0.7.23, LARMORX_FSLDIR) was not found")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    paths = _bet_inputs(tmp_path, fb)
    words = [w.format(**{k: str(v) for k, v in paths.items()}) for w in args.split()]
    env = {**fb.environment(), "OMP_NUM_THREADS": "1"}
    f_dir = tmp_path / "fsl"
    f_dir.mkdir()
    code, fsl_mib = _original_peak(
        [str(fb.fsl_dir() / "bin" / tool), *words], env, "fsl", _emptier(f_dir), f_dir
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {
            **env,
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
        }
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak([sys.executable, "-m", "larmorx", "fsl", tool, *words], l_env, r_dir)
        assert code == 0
        print(
            f"{tool} original ({mode}): larmorx {mib:.1f} MiB, FSL {fsl_mib:.1f}, entry {entry:.1f}"
        )
        assert mib <= allowance(fsl_mib, entry), (
            f"{tool} {args} original ({mode}): larmorx {mib:.0f} MiB, FSL {fsl_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        assert sorted(p.name for p in f_dir.iterdir()) == sorted(p.name for p in r_dir.iterdir())


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
    code, ants_mib = _original_peak(
        [str(programs.ants_bin() / program), *argv(a_dir)],
        {**env, "LD_PRELOAD": str(lib)},
        "itk",
        _emptier(a_dir),
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
#: ``-EPI`` on the series, 3dvolreg's RobustAverage call on its first 20 volumes (``{bold20}``;
#: since 2026-11-06). Every output is AFNI's byte for byte in ``reproduce`` (the header's
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
    # 3dvolreg (2026-11-06): RobustAverage's call (fMRIPrep's BOLD reference) on 20 volumes of
    # the series (-maxdisp1D left out: its file holds the command line, whose output paths
    # differ between the two directories).
    (
        "3dvolreg",
        "-Fourier -twopass -zpad 4 -1Dfile {out}/o.1D -1Dmatrix_save {out}/o.aff12.1D "
        "-prefix {out}/o.nii.gz {bold20}",
    ),
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
    save("bold20.nii.gz", bold[..., :20].astype(np.float32), (3.0, 3.0, 3.5, 2.0))
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
    code, afni_mib = _original_peak([str(afni), *argv(a_dir)], env, "afni", _emptier(a_dir), a_dir)
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


#: 3dvolreg's clean-room original (``crates/larmorx-afni``, 2026-11-08; ``specs/3dvolreg.md``
#: §14.4) at one thread, in both modes, against AFNI's peak: RobustAverage's call on the 20
#: volumes above (``bold20``), and on fMRIPrep's largest input of the validation, ds005454's
#: sliced series (160 x 160 x 96 x 20; ``validation/src/larmorx_validation/parity/
#: afni_volreg_original.py`` makes it from the oracle run: skipped until it exists). The outputs are
#: not AFNI's bits (the record holds them to the spec's thresholds), so each run is only checked to
#: write them.
ORIGINAL_VOLREG_CASES = [
    ("bold20", AFNI_CASES[4][1]),
    (
        "ds005454",
        "-Fourier -twopass -zpad 4 -1Dfile {out}/o.1D -1Dmatrix_save {out}/o.aff12.1D "
        "-prefix {out}/o.nii.gz {ds005454}",
    ),
]


@pytest.mark.parametrize(("name", "args"), ORIGINAL_VOLREG_CASES)
def test_volreg_original_needs_no_more_memory_than_afni(tmp_path, name, args):
    ar = pytest.importorskip("larmorx_validation.parity.afni_replica")
    vo = pytest.importorskip("larmorx_validation.parity.afni_volreg_original")
    afni = ar.afni_binary("3dvolreg", "26.3.05")
    if afni is None or not Path("/usr/bin/time").is_file():
        pytest.skip("the AFNI 26.3.05 3dvolreg oracle or /usr/bin/time was not found")
    inputs = tmp_path / "in"
    inputs.mkdir()
    paths = _afni_inputs(inputs) if name == "bold20" else {}
    if name == "ds005454":
        src = vo.fmriprep_dir() / "ds005454_sliced.nii.gz"
        if not src.is_file():
            pytest.skip("ds005454's sliced input was not made yet (the afni-volreg-original suite)")
        paths["ds005454"] = str(src)
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
    code, afni_mib = _original_peak([str(afni), *argv(a_dir)], env, "afni", _emptier(a_dir), a_dir)
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "LARMORX_IMPLEMENTATION": "original"}
        entry = entry_peak(l_env, r_dir)
        code, mib = _peak(
            [sys.executable, "-m", "larmorx", "afni", "3dvolreg", *argv(r_dir)], l_env, r_dir
        )
        assert code == 0
        print(f"3dvolreg original {name} ({mode}): larmorx {mib:.1f} MiB, AFNI {afni_mib:.1f}, entry {entry:.1f}")  # fmt: skip
        assert mib <= allowance(afni_mib, entry), (
            f"3dvolreg original {name} ({mode}): larmorx {mib:.0f} MiB, AFNI {afni_mib:.0f} MiB, "
            f"the entry point {entry:.1f} MiB"
        )
        assert sorted(p.name for p in r_dir.iterdir()) == sorted(p.name for p in a_dir.iterdir())


#: sdcflows' field-map estimation (field maps part A, 2026-11-08): fMRIPrep's calls of
#: ``BSplineApprox`` (its ``bs_filter`` nodes of ds002422's first estimator, 64 x 64 x 24, and of
#: ds005454's, 112 x 112 x 70) and ``ApplyCoeffsField`` on ds005454's coefficients, registration
#: and first 20 BOLD volumes (160 x 160 x 96, int16), sdcflows 2.17.1 in the newest Python
#: environment (``python_versions``) against ``lx.sdcflows``, each in a process of its own
#: (``validation/investigations/sdcflows-fieldmap/memory_call.py``), one thread; larmorx in both
#: D13 modes, its files sdcflows' byte for byte in ``reproduce`` (decompressed). The original's
#: run goes through the oracle cache as every original's does: the interpreter is keyed, its
#: packages are not, so the releases (and numpy's BLAS kernels) are an argument of the call, which
#: refuses to run with others. The allowance is the memory rule's: 1.1 times sdcflows' peak plus
#: ``OVERHEAD_MIB`` (both sides pay the interpreter).
SDCFLOWS_RUNS = "fmriprep-25.2.6"
SDCFLOWS_CASES = [
    ("fit-ds002422", "fit", "ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
    ("fit-ds005454", "fit", "ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ("apply-ds005454", "apply", "ds005454-sub-16", "sub_16_wf"),
]


def _sdcflows_inputs(tmp: Path, ws: Path, call: str, run: str, node: str) -> list[str] | None:
    base = ws / "oracles" / SDCFLOWS_RUNS / f"{run}-noreconall" / "work" / "fmriprep_25_2_wf"
    if call == "fit":
        wf = base / node
        fmaps = sorted((wf / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))
        mask = wf / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
        return [str(fmaps[0]), str(mask)] if fmaps and mask.is_file() else None
    coeffs = sorted((base / node).glob("fmap_preproc_wf/wf_gre16/bs_filter/*_coeff000.nii.gz"))
    xfm = (
        base / node / "bold_task_rest_wf/bold_fit_wf/fmapreg_wf/coregister/"
        "transform0GenericAffine.mat"
    )
    bold = (
        Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro" / "ds005454"
        / "sub-16" / "func" / "sub-16_task-rest_bold.nii.gz"
    )  # fmt: skip
    if not coeffs or not xfm.is_file() or not bold.is_file():
        return None
    img = nib.load(bold)
    head = tmp / "sub-16_task-rest_bold.nii.gz"
    sub = img.slicer[..., :20]
    nib.Nifti1Image(np.asanyarray(sub.dataobj), img.affine, img.header).to_filename(head)
    return [str(head), str(coeffs[0]), str(xfm), "0.0429303", "j-"]


@pytest.mark.parametrize(("name", "call", "run", "node"), SDCFLOWS_CASES)
def test_sdcflows_port_needs_no_more_memory_than_sdcflows(tmp_path, name, call, run, node):
    pv = pytest.importorskip("larmorx_validation.parity.python_versions")
    newest = pv.prefix(pv.NEWEST)
    if newest is None:
        pytest.skip("the newest Python environment (oracles/python-newest) was not found")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    inputs = _sdcflows_inputs(tmp_path, pv.workspace(), call, run, node)
    if inputs is None:
        pytest.skip(f"{run}'s oracle run or BOLD series was not found")
    script = (
        Path(__file__).resolve().parents[2]
        / "validation" / "investigations" / "sdcflows-fieldmap" / "memory_call.py"
    )  # fmt: skip
    py = str(newest / "bin" / "python")
    env = {
        **{k: v for k, v in os.environ.items() if k != "PYTHONPATH"},
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_NUM_THREADS": "1",
        "MKL_NUM_THREADS": "1",
        "OPENBLAS_CORETYPE": os.environ.get("OPENBLAS_CORETYPE", "Haswell"),
    }
    done = subprocess.run(
        [py, "-P", "-c", f"import runpy; print(runpy.run_path({str(script)!r})['releases']())"],
        capture_output=True, text=True, env=env, check=True,
    )  # fmt: skip
    releases = done.stdout.strip()
    s_dir = tmp_path / "sdcflows"
    s_dir.mkdir()
    code, sdc_mib = _original_peak(
        [py, "-P", str(script), "sdcflows", call, *inputs, "--env", releases],
        env,
        None,
        _emptier(s_dir),
        s_dir,
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {**env, "PYTHONPATH": os.environ.get("PYTHONPATH", "")}
        code, mib = _peak(
            [sys.executable, "-P", str(script), "larmorx", call, *inputs, "--defects", mode],
            l_env,
            r_dir,
        )
        assert code == 0
        print(f"{name} ({mode}): larmorx {mib:.1f} MiB, sdcflows {sdc_mib:.1f} MiB")
        assert mib <= allowance(sdc_mib), (
            f"{name} ({mode}): larmorx {mib:.0f} MiB, sdcflows {sdc_mib:.0f} MiB"
        )
        if mode == "reproduce":
            assert _stored(s_dir) == _stored(r_dir), name


#: The field maps' part B (2026-11-09): sdcflows' ``brain_masker`` on fMRIPrep's ds002603 magnitude
#: (``masker``'s input, 128 x 128 x 128 once padded: sdcflows' largest Python step, its Laplacian
#: held several times), niworkflows' ``IntensityClip`` on the merged magnitude before it
#: (``clipper_pre``), niworkflows' ``RobustAverage``'s own steps on ds004958's PEPOLAR series
#: (``mc_method=None``: the 3dvolreg it runs otherwise is a program of its own, measured by the
#: 3dvolreg calls) and fMRIPrep's ``ReconstructFieldmap`` onto ds005454's T1w grid through two
#: transforms (``bold_anat_wf.fmap_recon``), the originals in the newest Python environment, each
#: in a process of its own (``validation/investigations/sdcflows-fieldmap-b/memory_call.py``),
#: one thread; larmorx in both D13 modes, its files the originals' byte for byte in
#: ``reproduce`` (decompressed). The originals' runs go through the oracle cache, the releases an
#: argument of the call (as the calls above). The allowance is the memory rule's.
FIELDMAP_B_CASES = [
    ("masker-ds002603", "masker"),
    ("clip-ds002603", "clip"),
    ("ravg-ds004958", "ravg"),
    ("recon-ds005454", "recon"),
]


def _fieldmap_b_inputs(ws: Path, call: str) -> list[str] | None:
    runs = ws / "oracles" / SDCFLOWS_RUNS
    mag = (
        runs / "ds002603-sub-01-noreconall" / "work" / "fmriprep_25_2_wf" / "sub_01_wf"
        / "fmap_preproc_wf" / "wf_auto_00000" / "magnitude_wf"
    )  # fmt: skip
    r54 = runs / "ds005454-sub-16-noreconall"
    data = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
    if call == "masker":
        paths = [mag / "brainextraction_wf" / "clipper_post" / "clipped.nii.gz"]
        extra: list[str] = []
    elif call == "clip":
        paths = [mag / "magmrg" / "sub-01_acq-Mm2_magnitude1_merged.nii.gz"]
        extra = ["35.0", "99.98"]
    elif call == "ravg":
        paths = [
            data / "ds004958" / "sub-fsm64ps" / "fmap" / "sub-fsm64ps_acq-se_dir-AP_epi.nii.gz"
        ]
        extra = []
    else:
        fmap, func = r54 / "out" / "sub-16" / "fmap", r54 / "out" / "sub-16" / "func"
        paths = [
            fmap / "sub-16_fmapid-gre16_desc-coeff_fieldmap.nii.gz",
            fmap / "sub-16_fmapid-gre16_desc-magnitude_fieldmap.nii.gz",
            r54 / "work" / "fmriprep_25_2_wf" / "sub_16_wf" / "bold_task_rest_wf" / "bold_anat_wf"
            / "gen_ref" / "sub-16_desc-preproc_T1w_reference.nii.gz",
            func / "sub-16_task-rest_from-boldref_to-gre16_mode-image_desc-fmap_xfm.txt",
            func / "sub-16_task-rest_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt",
        ]  # fmt: skip
        extra = ["--inverse", "true,false"]
    if not all(p.is_file() for p in paths):
        return None
    return [str(p) for p in paths] + extra


@pytest.mark.parametrize(("name", "call"), FIELDMAP_B_CASES)
def test_fieldmap_steps_need_no_more_memory_than_the_originals(tmp_path, name, call):
    pv = pytest.importorskip("larmorx_validation.parity.python_versions")
    newest = pv.prefix(pv.NEWEST)
    if newest is None:
        pytest.skip("the newest Python environment (oracles/python-newest) was not found")
    if not Path("/usr/bin/time").is_file():
        pytest.skip("/usr/bin/time was not found")
    inputs = _fieldmap_b_inputs(pv.workspace(), call)
    if inputs is None:
        pytest.skip(f"{name}'s oracle run or input series was not found")
    script = (
        Path(__file__).resolve().parents[2]
        / "validation" / "investigations" / "sdcflows-fieldmap-b" / "memory_call.py"
    )  # fmt: skip
    py = str(newest / "bin" / "python")
    env = {
        **{k: v for k, v in os.environ.items() if k != "PYTHONPATH"},
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_NUM_THREADS": "1",
        "MKL_NUM_THREADS": "1",
        "OPENBLAS_CORETYPE": os.environ.get("OPENBLAS_CORETYPE", "Haswell"),
    }
    done = subprocess.run(
        [py, "-P", "-c", f"import runpy; print(runpy.run_path({str(script)!r})['releases']())"],
        capture_output=True, text=True, env=env, check=True,
    )  # fmt: skip
    releases = done.stdout.strip()
    o_dir = tmp_path / "original"
    o_dir.mkdir()
    code, orig_mib = _original_peak(
        [py, "-P", str(script), "original", call, *inputs, "--env", releases],
        env,
        None,
        _emptier(o_dir),
        o_dir,
    )
    assert code == 0
    for mode in ("reproduce", "fix"):
        r_dir = tmp_path / f"larmorx-{mode}"
        r_dir.mkdir()
        l_env = {**env, "PYTHONPATH": os.environ.get("PYTHONPATH", "")}
        code, mib = _peak(
            [py, "-P", str(script), "larmorx", call, *inputs, "--defects", mode], l_env, r_dir
        )
        assert code == 0
        print(f"{name} ({mode}): larmorx {mib:.1f} MiB, the original {orig_mib:.1f} MiB")
        assert mib <= allowance(orig_mib), (
            f"{name} ({mode}): larmorx {mib:.0f} MiB, the original {orig_mib:.0f} MiB"
        )
        if mode == "reproduce":
            assert _stored(o_dir) == _stored(r_dir), name
