# SPDX-License-Identifier: Apache-2.0
"""Parity of the fslstats replica (``larmorx-nc fsl fslstats``, FSL Licence) with FSL 6.0.7.23's
``fslstats``: exit status, standard output and standard error, byte for byte.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fsltest**: FSL's own tests (``src/fsl-avwutils/tests/fslstats/feedsRun``: every option on its
  ``test.nii.gz``, the preoptions on ``test_4d.nii.gz``, and the JSON forms against FSL's
  expected files), checked against FSL's expected outputs as well;
- **fmriprep-goodvoxels**: fMRIPrep's four fslstats calls in its goodvoxels mask
  (``--project-goodvoxels``: ``-M`` and ``-S`` of the coefficient of variation in the ribbon and
  of its modulated form), on the images FSL's own chain made (``fsl-maths-replica``'s
  goodvoxels chain on ds005040's BOLD series; the ribbon is a stand-in, the segmentation's grey
  matter, as the oracle runs have no FreeSurfer ribbon);
- **bet**: BET's ``fslstats <image> -l 0.001 -C`` (the replica's BET runs this program);
- **cli**, **opt**, **order**, **ts**, **types**, **d13**: the command line and its messages, each
  option, options in order, ``-t``, ``-K`` and ``-json``, the data types and the defects (FSL-181
  to FSL-189), one case per path, on small synthetic images made once
  (``oracles/inputs/fsl-stats-replica/``, seeded).

Every case compares FSL with the replica in ``reproduce`` mode (D13), then runs the default
``fix`` mode and checks D13's rule (a defect named exactly when the output changes). Every FSL
run goes through the oracle-output cache (fslstats is single-threaded and deterministic).
"""

from __future__ import annotations

import ast
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.fsl_flirt_replica import (
    _agree_on_errors,
    _environment,
    compare,
    fix_pass,
    fsldir,
    run_replica,
)
from larmorx_validation.parity.fsl_maths_replica import (
    GOODVOXELS_RUN,
    GOODVOXELS_STEPS,
    _affine,
    _goodvoxels_chain,
    _goodvoxels_sources,
    _save,
)
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import Case, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-stats-replica"
PROGRAM = "fslstats"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The oracle run whose anatomical image BET's call runs on.
BET_RUN = "ds005040-sub-002-noreconall"
_MARKER = "v2.done"


def fsl_tests() -> Path:
    """FSL's own fslstats tests and their images."""
    return fsldir() / "src" / "fsl-avwutils" / "tests" / "fslstats"


def oracle_run(args: list[str], *, cwd: Path, settings: dict[str, str | None]):
    """FSL's fslstats with ``args`` in ``cwd``, through the oracle-output cache (single-threaded
    and deterministic); every file in ``cwd`` counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / PROGRAM), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=600,
    )


def run_oracle(args: list[str], *, cwd: Path, settings: dict[str, str | None]) -> Completed:
    p = oracle_run(args, cwd=cwd, settings=settings)
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Inputs


def inputs() -> Path:
    """The synthetic inputs, made once (seeded; the same bytes every time)."""
    ws = workspace()
    base = (ws / "oracles" / "inputs" if ws else Path(tempfile.gettempdir())) / SUITE
    marker = base / _MARKER
    if marker.exists():
        return base
    base.mkdir(parents=True, exist_ok=True)
    _make_inputs(base)
    marker.write_text("1\n", encoding="utf-8")
    return base


def _make_inputs(d: Path) -> None:
    rng = np.random.default_rng(20261104)
    sh = (7, 6, 5)
    a = rng.normal(5.0, 3.0, sh).astype(np.float32)
    a[rng.random(sh) < 0.2] = 0.0
    a[1, 2, 3] = 12.5
    a[5, 1, 0] = -4.25
    _save(d / "a.nii.gz", a, dtype=np.float32)
    _save(d / "an.nii.gz", a, dtype=np.float32, affine=_affine(sh, neuro=True))
    a4 = rng.normal(3.0, 2.0, (*sh, 3)).astype(np.float32)
    a4[rng.random(a4.shape) < 0.25] = 0.0
    _save(d / "a4.nii.gz", a4, dtype=np.float32)
    m3 = (rng.random(sh) < 0.5).astype(np.uint8)
    _save(d / "m3.nii.gz", m3, dtype=np.uint8)
    m4 = (rng.random((*sh, 3)) < 0.5).astype(np.float32) * 0.8
    _save(d / "m4.nii.gz", m4, dtype=np.float32)
    _save(d / "m4s.nii.gz", m4[..., :2], dtype=np.float32)
    _save(d / "m4l.nii.gz", np.concatenate([m4, m4[..., :1]], axis=3), dtype=np.float32)
    _save(d / "mbig.nii.gz", np.ones((8, 6, 5), np.uint8), dtype=np.uint8)
    d3 = rng.normal(1.0, 1.0, sh).astype(np.float32)
    _save(d / "d3.nii.gz", d3, dtype=np.float32)
    d4 = rng.normal(1.0, 1.0, (*sh, 3)).astype(np.float32)
    _save(d / "d4.nii.gz", d4, dtype=np.float32)
    _save(d / "d4s.nii.gz", d4[..., :2], dtype=np.float32)
    _save(d / "dbig.nii.gz", np.ones((7, 6, 4), np.float32), dtype=np.float32)
    lab = rng.integers(0, 6, sh).astype(np.int16)
    lab[lab == 3] = 0
    _save(d / "lab.nii.gz", lab, dtype=np.int16)
    _save(d / "lab4.nii.gz", np.stack([lab, lab], axis=3), dtype=np.int16)
    _save(d / "labneg.nii.gz", -np.abs(lab), dtype=np.int16)
    _save(d / "labbig.nii.gz", np.ones((7, 6, 6), np.int16), dtype=np.int16)
    nan = a.copy()
    nan[0, 0, 0] = np.nan
    nan[3, 3, 3] = np.nan
    _save(d / "nan.nii.gz", nan, dtype=np.float32)
    nan2 = rng.normal(5.0, 3.0, (9, 8, 7)).astype(np.float32)
    nan2[rng.random(nan2.shape) < 0.1] = np.nan
    nan2[0, 0, 0] = 1.0
    _save(d / "nan2.nii.gz", nan2, dtype=np.float32)
    inf = a.copy()
    inf[2, 2, 2], inf[4, 4, 4], inf[0, 1, 0] = np.inf, -np.inf, np.nan
    _save(d / "inf.nii.gz", inf, dtype=np.float32)
    _save(d / "const.nii.gz", np.full(sh, 2.5, np.float32), dtype=np.float32)
    _save(d / "const01.nii.gz", np.full(sh, 0.1, np.float32), dtype=np.float32)
    _save(d / "one.nii.gz", np.full((1, 1, 1), 3.0, np.float32), dtype=np.float32)
    _save(d / "zeros.nii.gz", np.zeros((*sh, 3), np.float32), dtype=np.float32)
    _save(d / "zeros3.nii.gz", np.zeros(sh, np.float32), dtype=np.float32)
    outl = rng.random(sh).astype(np.float32)
    outl[6, 5, 4] = 1e12
    _save(d / "outl.nii.gz", outl, dtype=np.float32)
    _save(d / "u8.nii.gz", np.clip(a * 10, 0, 255).astype(np.uint8), dtype=np.uint8)
    _save(d / "i16.nii.gz", (a * 100).astype(np.int16), dtype=np.int16)
    _save(d / "i16s.nii.gz", (a * 100).astype(np.int16), dtype=np.int16, slope=(0.5, 1.0))
    _save(d / "f64.nii.gz", a.astype(np.float64) / 3.0, dtype=np.float64)
    big = np.zeros((257, 257, 255), np.uint8)
    big[100:110, 100:110, 100:110] = 1
    _save(d / "bigcount.nii.gz", big, dtype=np.uint8)
    _save(d / "anf.nii.gz", a, dtype=np.float32, noform=True)


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: fslstats' arguments (``@name`` an input of this suite, ``%name`` a file of FSL's
    tests, both copied into the run directory under their own names), the environment, and
    ``expected``: FSL's own expected output (its test's text, or its JSON file's name)."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    expected: str = ""
    expected_json: str = ""


def _prepare(run: Run, tmp: Path) -> list[str]:
    out = []
    for a in run.args:
        if a[:1] in "@%":
            src = inputs() if a[0] == "@" else fsl_tests()
            name = a[1:]
            p = src / name if (src / name).exists() else src / f"{name}.nii.gz"
            if not (tmp / p.name).exists():
                shutil.copy(p, tmp / p.name)
            out.append(p.name)
        else:
            out.append(a)
    return out


def _run_case(run: Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = run_oracle(args, cwd=a, settings=settings)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if run.expected:
            checks(
                "FSL's own test: the expected output",
                replica.stdout.rstrip() == run.expected,
                detail=replica.stdout[-200:],
            )
        if run.expected_json:
            want = (fsl_tests() / run.expected_json).read_text(encoding="utf-8")
            checks(
                f"FSL's own test: {run.expected_json}",
                replica.stdout == want,
                detail=replica.stdout[-200:],
            )
        if oracle.code < 0:
            _crash_fix(checks, run, args, settings, replica)
        elif replica.code >= 0:
            fix_pass(
                checks,
                PROGRAM,
                lambda t: _prepare(run, t),
                args,
                settings,
                b,
                replica,
                silent_ok=(),
            )
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _crash_fix(
    checks: CheckList,
    run: Run,
    args: list[str],
    settings: dict[str, str | None],
    reproduced: Completed,
) -> None:
    """FSL crashed (the replica refused in ``reproduce`` mode): the default refuses too, naming
    the same defect, or gives an output (exit 0) naming it."""
    ids = defects.notices_in(reproduced.stderr)
    with tempfile.TemporaryDirectory(prefix="lx-nc-stats-fix-") as c_:
        c = Path(c_)
        _prepare(run, c)
        r = run_replica(PROGRAM, args, cwd=c, settings=settings, mode=defects.FIX)
    named = defects.notices_in(r.stderr)
    checks(
        "D13: FSL crashes; the default refuses or corrects, naming the defect",
        bool(ids) and set(ids) <= set(named) and r.code in (0, 1),
        detail=f"reproduce {ids}, default exit {r.code} {named}",
    )


def _r(*args: str, expected: str = "", expected_json: str = "") -> Run:
    return Run(tuple(args), (), expected, expected_json)


def _fsl_test_cases() -> dict[str, tuple[str, str, Run]]:
    """FSL's ``feedsRun``: its option lists read from the script itself."""
    out: dict[str, tuple[str, str, Run]] = {}
    script = fsl_tests() / "feedsRun"
    if not script.exists():
        return out
    tree = ast.parse(script.read_text(encoding="utf-8"))
    lists = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name):
            name = node.targets[0].id
            if name in ("options", "tests_with_preoptions"):
                lists[name] = ast.literal_eval(node.value)
    for k, o in enumerate(lists.get("options", [])):
        opts = o["option"].split()
        t = "smoke" if k == 0 else "standard"
        out[f"fsltest/{o['name']}/{'_'.join(opts)}"] = (
            t,
            f"FSL's test {o['name']}: fslstats test.nii.gz {o['option']}",
            _r("%test.nii.gz", *[_fsl_name(x) for x in opts], expected=o["expected"]),
        )
        fname = f"test_{o['name']}_{o['option'].replace(' ', '_')}.json".replace("-", "")
        out[f"fsltest/json/{o['name']}/{'_'.join(opts)}"] = (
            "standard",
            f"FSL's JSON test {o['name']}: fslstats -json test_4d.nii.gz {o['option']}",
            _r(
                "-json",
                "%test_4d.nii.gz",
                *[_fsl_name(x) for x in opts],
                expected_json=fname if (fsl_tests() / fname).exists() else "",
            ),
        )
    for o in lists.get("tests_with_preoptions", []):
        pre = o["preoptions"].split()
        mask = [f"%{o['mask']}"] if o["mask"] else []
        opts = o["options"].split()
        key = "_".join(pre + opts)
        out[f"fsltest/{o['name']}/{key}"] = (
            "standard",
            f"FSL's test {o['name']}: fslstats {o['preoptions']} {o['mask']} test_4d.nii.gz "
            f"{o['options']}",
            _r(*pre, *mask, "%test_4d.nii.gz", *opts, expected=o["expected"]),
        )
        fname = (
            f"test_{o['name']}_{o['preoptions'].replace(' ', '_')}_"
            f"{o['options'].replace(' ', '_')}.json"
        ).replace("-", "")
        out[f"fsltest/json/{o['name']}/{key}"] = (
            "standard",
            f"FSL's JSON test {o['name']}",
            _r(
                "-json",
                *pre,
                *mask,
                "%test_4d.nii.gz",
                *opts,
                expected_json=fname if (fsl_tests() / fname).exists() else "",
            ),
        )
    return out


def _fsl_name(x: str) -> str:
    return f"%{x}" if x.endswith(".nii.gz") else x


def _cases() -> dict[str, tuple[str, str, Run]]:
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, desc: str, *args: str, tier: str = "standard") -> None:
        c[name] = (tier, desc, _r(*args))

    # the command line, messages and statuses
    add("cli/usage", "no arguments: the usage (status 1)")
    add("cli/usage-one", "only an input: the usage", "@a")
    add("cli/usage-K", "-K and its mask only: the usage", "-K", "@lab")
    add("cli/usage-t", "-t and an input only: the usage", "-t", "@a")
    add("cli/missing", "an input that does not exist (status 255)", "missing.nii.gz", "-m")
    add("cli/unknown", "an unknown option (status 3)", "@a", "-m", "-q", "-M")
    for o in ("-l", "-u", "-k", "-d", "-p", "-P", "-h", "-H"):
        add(f"cli/no-arg{o}", f"{o} without its argument (status 2)", "@a", "-m", o)
    add("cli/H-no-min", "-H without its minimum", "@a", "-H", "5")
    add("cli/H-no-max", "-H without its maximum", "@a", "-H", "5", "0")
    add("cli/p-above", "-p 101 (status 1)", "@a", "-p", "101")
    add("cli/P-below", "-P -1 (status 1)", "@a", "-P", "-1")
    add("cli/h-zero", "-h 0 (status 1)", "@a", "-h", "0")
    add("cli/h-fraction", "-h 0.5: 0 bins after truncation", "@a", "-h", "0.5")
    add("cli/h-text", "-h abc (atof: 0)", "@a", "-h", "abc")
    add("cli/K-4d", "-K with a 4D index mask (status 3)", "-K", "@lab4", "@a", "-m")
    add(
        "cli/K-size",
        "-K with an index mask of another size (an exception)",
        "-K",
        "@labbig",
        "@a",
        "-m",
    )
    add("cli/K-none", "-K with no positive label: nothing printed", "-K", "@labneg", "@a", "-m")
    add("cli/k-size", "-k with a mask of another size (status 3)", "@a", "-k", "@mbig", "-m")
    add("cli/k-tsize", "-k with a mask of more volumes (status 3)", "@a4", "-k", "@m4l", "-m")
    add("cli/k-missing", "-k naming a missing file", "@a", "-k", "nomask.nii.gz", "-m")
    add("cli/d-size", "-d with an image of another size (status 3)", "@a", "-d", "@dbig", "-m")
    add("cli/d-tsize", "-d with more volumes (status 3)", "@a", "-d", "@d4", "-m")
    add("cli/json-twice", "-json -json", "-json", "-json", "@a", "-m")
    add("cli/json-empty", "-json with only an option that prints nothing", "-json", "@a", "-a")
    add(
        "cli/json-repeat",
        "-json with an option twice (the last kept)",
        "-json",
        "@a",
        "-M",
        "-l",
        "4",
        "-M",
    )
    # each option
    every = [
        "-r",
        "-R",
        "-e",
        "-E",
        "-v",
        "-V",
        "-m",
        "-M",
        "-s",
        "-S",
        "-w",
        "-x",
        "-X",
        "-c",
        "-C",
        "-D",
    ]
    for k, o in enumerate(every):
        add(f"opt/{o}", f"{o} on a 3D image", "@a", o, tier="smoke" if k == 0 else "standard")
        add(f"opt/json{o}", f"-json {o}", "-json", "@a", o)
    for o in ("-w", "-x", "-X", "-C", "-c"):
        add(f"opt/neuro{o}", f"{o} on a neurological image (read mirrored)", "@an", o)
    add("opt/p", "-p 50", "@a", "-p", "50")
    add("opt/p0", "-p 0", "@a", "-p", "0")
    add("opt/p100", "-p 100", "@a", "-p", "100")
    add("opt/P", "-P 30", "@a", "-P", "30")
    add("opt/h", "-h 7", "@a", "-h", "7")
    add("opt/H", "-H 7 0 5 (values outside the limits)", "@a", "-H", "7", "0", "5")
    add("opt/json-h", "-json -h 4", "-json", "@a", "-h", "4")
    add("opt/a", "-a then -m -R", "@a", "-a", "-m", "-R")
    add("opt/n", "-n then -m -R (NaN and infinities)", "@inf", "-n", "-m", "-R", "-s")
    add("opt/inf", "infinities and NaN without -n", "@inf", "-m", "-M", "-v", "-V")
    add(
        "opt/4d",
        "every output on a 4D image",
        "@a4",
        *every,
        "-p",
        "40",
        "-P",
        "40",
        "-h",
        "5",
        "-H",
        "5",
        "0",
        "4",
    )
    # options in order
    add("order/M-l-M", "the usage's example: -M -l 4 -M", "@a", "-M", "-l", "4", "-M")
    add(
        "order/thresholds",
        "-l 2 -u 8 then the outputs",
        "@a",
        "-l",
        "2",
        "-u",
        "8",
        *every,
        "-p",
        "25",
        "-P",
        "25",
        "-h",
        "5",
        "-H",
        "5",
        "0",
        "10",
    )
    add(
        "order/mask",
        "-k then the outputs",
        "@a",
        "-k",
        "@m3",
        *every,
        "-p",
        "25",
        "-P",
        "25",
        "-h",
        "5",
        "-H",
        "5",
        "0",
        "10",
    )
    add(
        "order/mask-then-l",
        "-k then -l (the threshold replaces the mask)",
        "@a",
        "-k",
        "@m3",
        "-l",
        "3",
        "-m",
        "-V",
    )
    add(
        "order/l-then-mask",
        "-l then -k (the mask replaces the threshold)",
        "@a",
        "-l",
        "3",
        "-k",
        "@m3",
        "-m",
        "-V",
    )
    add("order/u-only", "-u only", "@a", "-u", "4", "-m", "-v", "-w")
    add("order/d", "-d then -m, -a, -m", "@a", "-d", "@d3", "-m", "-a", "-m")
    add(
        "order/k3-4d", "a 3D mask over a 4D image", "@a4", "-k", "@m3", "-v", "-V", "-m", "-p", "50"
    )
    add("order/k4-4d", "a 4D mask", "@a4", "-k", "@m4", "-v", "-V", "-m", "-S")
    add(
        "order/k4s-4d",
        "a 4D mask of fewer volumes (its last repeated)",
        "@a4",
        "-k",
        "@m4s",
        "-v",
        "-M",
    )
    add("order/d3-4d", "-d of a 3D image on a 4D one", "@a4", "-d", "@d3", "-m")
    add("order/d4s-4d", "-d of fewer volumes (the last repeated)", "@a4", "-d", "@d4s", "-m")
    add(
        "order/empty-mask",
        "thresholds that keep nothing",
        "@a",
        "-l",
        "100",
        "-m",
        "-M",
        "-s",
        "-S",
        "-v",
        "-V",
        "-R",
        "-r",
        "-p",
        "50",
        "-P",
        "50",
        "-h",
        "4",
        "-w",
        "-c",
    )
    # -t, -K, -json
    add("ts/t", "-t: a line per volume", "-t", "@a4", "-m", "-M", "-s", "-v", "-w")
    add("ts/t-k3", "-t with a 3D mask", "-t", "@a4", "-k", "@m3", "-m", "-V")
    add("ts/t-K", "-t -K: a line per volume and label", "-t", "-K", "@lab", "@a4", "-m", "-v")
    add(
        "ts/K",
        "-K: a line per label (label 3 missing)",
        "-K",
        "@lab",
        "@a",
        "-m",
        "-S",
        "-w",
        "-x",
        "-C",
    )
    add("ts/K-4d", "-K over a 4D image", "-K", "@lab", "@a4", "-M", "-v")
    add("ts/K-u", "-K then a threshold", "-K", "@lab", "@a", "-u", "4", "-m")
    add("ts/json-t-K", "-json -t -K", "-json", "-t", "-K", "@lab", "@a4", "-m", "-R", "-x")
    add("ts/json-K", "-json -K with a missing label", "-json", "-K", "@lab", "@a", "-m", "-v")
    add("ts/K-json", "-K before -json", "-K", "@lab", "-json", "@a", "-m")
    # data types
    for t in ("u8", "i16", "i16s", "f64"):
        add(
            f"types/{t}",
            f"a {t} image (read as float)",
            f"@{t}",
            "-m",
            "-s",
            "-R",
            "-p",
            "50",
            "-v",
            "-r",
        )
    add(
        "types/noform",
        "an image without sform or qform: -c, -C, -x, -w",
        "@anf",
        "-c",
        "-C",
        "-x",
        "-w",
    )
    add("opt/json-nan", "-json of NaN results (null)", "-json", "@nan", "-m", "-s", "-n", "-m")
    # D13
    for o in ("-R", "-x", "-X", "-r", "-e", "-E", "-m", "-M"):
        add(f"d13/nan{o}", f"FSL-181: a NaN first value, {o}", "@nan", o)
    add("d13/nan-p", "FSL-181: -p 50 with NaN values", "@nan", "-p", "50")
    add("d13/nan-P", "FSL-181: -P 30 with NaN values", "@nan", "-P", "30")
    add("d13/nan-h", "FSL-181: -h 5 with NaN values", "@nan", "-h", "5")
    add("d13/nan-H", "FSL-181: -H 5 0 10 with NaN values", "@nan", "-H", "5", "0", "10")
    add("d13/nan2-p", "FSL-181: -p 50 sorting NaN values", "@nan2", "-p", "50")
    add("d13/nan2-R", "FSL-181: NaN values after the first", "@nan2", "-R", "-x", "-r")
    add("d13/nan-n", "FSL-181: -n first (no defect)", "@nan", "-n", "-R", "-p", "50", "-r")
    add("d13/nan-k", "FSL-181: NaN under a mask", "@nan", "-k", "@m3", "-R", "-p", "50", "-e")
    add("d13/const01-s", "FSL-182: -s of a constant image", "@const01", "-s", "-S")
    add("d13/one-s", "FSL-182: -s of one voxel", "@one", "-s", "-S")
    add("d13/S", "FSL-182: -S without a mask", "@a", "-S")
    add("d13/t-k4", "FSL-183: -t with a 4D mask", "-t", "@a4", "-k", "@m4", "-m")
    add("d13/t-d3", "FSL-183: -t with a 3D -d image", "-t", "@a4", "-d", "@d3", "-m")
    add("d13/t-d4", "FSL-183: -t with a 4D -d image", "-t", "@a4", "-d", "@d4", "-m")
    add("d13/x-empty", "FSL-184: -x of an empty mask", "@a", "-l", "1e30", "-x")
    add("d13/X-empty", "FSL-184: -X of an empty mask", "@a", "-m", "-u", "-1e30", "-X")
    add("d13/e-const", "FSL-185: -e of a constant image", "@const", "-e")
    add("d13/E-const", "FSL-185: -m -E -m of a constant image", "@const", "-m", "-E", "-m")
    add("d13/h-const", "-h of a constant image (no range: nothing printed)", "@const", "-h", "4")
    add("d13/H-equal", "FSL-186: -H 5 3 3", "@a", "-H", "5", "3", "3")
    add("d13/H-reversed", "FSL-186: -H 5 4 1", "@a", "-m", "-H", "5", "4", "1", "-M")
    add("d13/H-far", "FSL-187: -H 10 0 1 with a value of 1e12", "@outl", "-H", "10", "0", "1")
    add("d13/e-far", "FSL-187: -e with a value of 1e12", "@outl", "-e", "-E", "-r")
    add("d13/w-zeros4", "FSL-188: -w of an all-zero 4D image", "@zeros", "-w")
    add("d13/w-zeros3", "-w of an all-zero 3D image", "@zeros3", "-w")
    add("d13/json-v-big", "FSL-189: -json -v beyond 2^24 voxels", "-json", "@bigcount", "-v", "-V")
    add(
        "d13/json-v-replaced",
        "FSL-189 not met: the -v beyond 2^24 voxels replaced by a later -v",
        "-json",
        "@bigcount",
        "-v",
        "-l",
        "0.5",
        "-v",
        "-V",
    )
    add("d13/v-big", "-v -V beyond 2^24 voxels (text)", "@bigcount", "-v", "-V")
    c.update(_fsl_test_cases())
    return c


# ------------------------------------------------------------------------------------------------
# fMRIPrep's and BET's calls


def goodvoxels_cases() -> list[tuple[str, str]]:
    """``(run, node)`` of each fslstats call of the goodvoxels workflow."""
    if _goodvoxels_sources(GOODVOXELS_RUN) is None:
        return []
    return [(GOODVOXELS_RUN, n) for n, prog, _ in GOODVOXELS_STEPS if prog == "fslstats"]


def _goodvoxels_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    base = _goodvoxels_chain(run)
    if base is None:
        raise Outcome("skipped", f"the oracle run {run} is not available")
    template = next(t for n, _, t in GOODVOXELS_STEPS if n == node)
    src = template[0][1:-1]
    image = base / src / f"{src}.nii.gz"
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-gv-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-gv-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        for d in (a, b):
            shutil.copy(image, d / image.name)
        args = [image.name, *template[1:]]
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if replica.code == 0:
            fix_pass(
                checks,
                PROGRAM,
                lambda t: shutil.copy(image, t / image.name),
                args,
                {},
                b,
                replica,
                silent_ok=(),
            )
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _bet_image() -> Path | None:
    ws = workspace()
    if ws is None:
        return None
    out = ws / FMRIPREP_RUNS / BET_RUN / "out"
    return next(iter(sorted(out.glob("sub-*/anat/sub-*_desc-preproc_T1w.nii.gz"))), None)


def _bet_case(_payload: object, checks: CheckList) -> None:
    image = _bet_image()
    if image is None:
        raise Outcome("skipped", f"the oracle run {BET_RUN} is not available")
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-bet-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-stats-bet-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        for d in (a, b):
            shutil.copy(image, d / "t1w.nii.gz")
        args = ["t1w.nii.gz", "-l", "0.001", "-C"]
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for k, (run, node) in enumerate(goodvoxels_cases()):
        t = "smoke" if k == 0 else "standard"
        if in_tier(t, tier):
            out.append(
                Case(
                    f"fmriprep-goodvoxels/{run}/{node}",
                    "fmriprep",
                    f"fMRIPrep's goodvoxels mask, node {node} (--project-goodvoxels; the "
                    "ribbon a stand-in: the segmentation's grey matter)",
                    ("goodvoxels", (run, node)),
                )
            )
    if in_tier("standard", tier) and _bet_image() is not None:
        out.append(
            Case(
                f"bet/{BET_RUN}/T1w",
                "bet",
                "BET's call: fslstats <T1w> -l 0.001 -C (fMRIPrep's preprocessed T1w)",
                ("bet", None),
            )
        )
    for name, (t, desc, _run) in _cases().items():
        if in_tier(t, tier):
            out.append(Case(name, name.split("/")[0], desc, ("run", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "goodvoxels":
        _goodvoxels_case(payload, checks)
    elif kind == "bet":
        _bet_case(payload, checks)
    else:
        _run_case(_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="fslstats replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl fslstats` (crate larmorx-nc-core's fslstats, translated from "
        f"FSL {fsl_versions.NEWEST}'s source)",
        reference=f"fslstats from FSL {fsl_versions.NEWEST} (fsl-avwutils 2209.6, fsl-newimage "
        "2601.0, fsl-armawrap 0.7.0, nlohmann/json 3.12.0; the conda build at `~/fsl-6.0.7.23`, "
        "as built: glibc 2.35's maths)",
        thresholds=[
            ("Exit status", "the same (a crash: the replica exits 1)"),
            (
                "Standard output and error",
                "identical (the run directory and FSL's program path replaced; the replica's "
                "`larmorx-nc:` lines left out)",
            ),
            ("FSL's own tests", "the expected outputs of `feedsRun` and its JSON files"),
            ("D13 fix pass", "a defect named exactly when the default's output differs"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), "
            "compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked "
            "against the reproduced run. The goodvoxels calls run on FSL's own chain of "
            "fslmaths outputs (the ribbon a stand-in: the segmentation's grey matter, as the "
            "no-reconall oracle runs have no FreeSurfer ribbon)."
        ),
    )
