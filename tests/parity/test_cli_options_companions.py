# SPDX-License-Identifier: Apache-2.0
"""larmorx's own options with the real companion programs (this machine only: ``larmorx-nc``
and ``larmorx-gpl`` built in ``crates-nc/`` and ``crates-gpl/``, or named by ``LARMORX_NC_BIN``
and ``LARMORX_GPL_BIN``): through the console script (``python -m larmorx``) and the standalone
program (``target/release/larmorx``), ``--n-threads``, ``--upstream-defects`` and
``--implementation replica`` give the same exit status, messages and output bytes as the
variables they replace, and win over them. The replica programs are run as black boxes."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import fsl_mcflirt_replica, tedana_t2smap_replica

import larmorx as lx

ROOT = Path(__file__).resolve().parents[2]
NC = fsl_mcflirt_replica.replica_binary()
GPL = tedana_t2smap_replica.replica_binary()
STANDALONE = ROOT / "target" / "release" / ("larmorx.exe" if os.name == "nt" else "larmorx")

#: The variables the options replace, removed before each run.
VARIABLES = ("LARMORX_IMPLEMENTATION", "LARMORX_UPSTREAM_DEFECTS", "OMP_NUM_THREADS")

COMMAND_LINES = {
    "console-script": [sys.executable, "-m", "larmorx"],
    "standalone": [str(STANDALONE)],
}


def image(path: Path, shape: tuple[int, ...], scale: float = 1.0) -> Path:
    i = np.arange(int(np.prod(shape)))
    data = ((i * 7919) % 97).astype(np.float32).reshape(shape, order="F")
    inner = tuple(slice(s // 4, s - s // 4) for s in shape[:3])
    data[inner] += 1000.0 * scale
    lx.save((np.asfortranarray(data), np.diag([2.0, 2.0, 2.0, 1.0])), path)
    return path


def run(command: list[str], cwd: Path, env: dict[str, str], outputs: list[str]):
    full = {k: v for k, v in os.environ.items() if k not in VARIABLES}
    full.update(env)
    p = subprocess.run(command, cwd=cwd, env=full, capture_output=True, check=False)
    files = []
    for name in outputs:
        f = cwd / name
        files.append(f.read_bytes() if f.is_file() else None)
        if f.is_file():
            f.unlink()
    return (p.returncode, p.stdout, p.stderr), files


def same(cli, cwd, base, option, variable, args, outputs):
    a = run([*cli, *option, *args], cwd, base, outputs)
    b = run([*cli, *args], cwd, {**base, **variable}, outputs)
    assert a == b, (option, variable, a[0], b[0])
    return a


@pytest.fixture(params=sorted(COMMAND_LINES))
def cli(request):
    if request.param == "standalone" and not STANDALONE.is_file():
        pytest.skip(f"the standalone program is not built ({STANDALONE})")
    return COMMAND_LINES[request.param]


@pytest.mark.skipif(NC is None or os.name == "nt", reason="the larmorx-nc program is not built")
def test_larmorx_nc_gets_the_options(cli, tmp_path):
    image(tmp_path / "in.nii.gz", (16, 16, 12))
    base = {"LARMORX_NC_BIN": str(NC), "FSLOUTPUTTYPE": "NIFTI_GZ"}
    maths = ["fsl", "fslmaths", "in.nii.gz", "-s", "1", "out"]
    for option, variable in (
        (["--n-threads", "3"], {"OMP_NUM_THREADS": "3"}),
        (["--n-threads", "0"], {}),
        (["--implementation", "replica"], {"LARMORX_IMPLEMENTATION": "replica"}),
        (["--upstream-defects", "reproduce"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}),
    ):
        (code, _, err), files = same(cli, tmp_path, base, option, variable, maths, ["out.nii.gz"])
        assert code == 0, err
        assert files[0] is not None
    # the option wins over its variable
    (code, _, err), files = run(
        [*cli, "--upstream-defects", "fix", "--implementation", "replica", *maths],
        tmp_path,
        {**base, "LARMORX_UPSTREAM_DEFECTS": "bogus", "LARMORX_IMPLEMENTATION": "clean-room"},
        ["out.nii.gz"],
    )
    assert code == 0 and files[0] is not None, err
    # the implementation that ran is the one asked for: the program, through a wrapper that
    # notes each start, runs for "replica" and never for "clean-room" (or "original")
    log = tmp_path / "started.txt"
    wrapper = tmp_path / "larmorx-nc"
    wrapper.write_text(f'#!/bin/sh\necho started >> "{log}"\nexec "{NC}" "$@"\n', "utf-8")
    wrapper.chmod(0o755)
    bet = ["fsl", "bet", "in.nii.gz", "brain"]
    for implementation, starts in (("replica", True), ("clean-room", False), ("original", False)):
        log.unlink(missing_ok=True)
        (code, _, err), _ = same(
            cli,
            tmp_path,
            {**base, "LARMORX_NC_BIN": str(wrapper)},
            ["--implementation", implementation],
            {"LARMORX_IMPLEMENTATION": implementation},
            bet,
            ["brain.nii.gz"],
        )
        assert code == 0, err
        n = len(log.read_text().splitlines()) if log.is_file() else 0
        assert (n > 0) == starts, (implementation, n)


@pytest.mark.skipif(GPL is None, reason="the larmorx-gpl program is not built")
def test_larmorx_gpl_gets_the_options(cli, tmp_path):
    for e, scale in ((1, 1.0), (2, 0.6), (3, 0.35)):
        image(tmp_path / f"e{e}.nii.gz", (8, 8, 6, 3), scale)
    mask = np.zeros((8, 8, 6), np.uint8)
    mask[2:6, 2:6, 1:5] = 1
    lx.save((mask, np.diag([2.0, 2.0, 2.0, 1.0])), tmp_path / "mask.nii.gz")
    t2smap = ["tedana", "t2smap", "-d", "e1.nii.gz", "e2.nii.gz", "e3.nii.gz", "-e", "14", "30", "46", "--mask", "mask.nii.gz"]  # fmt: skip
    outputs = ["T2starmap.nii.gz", "S0map.nii.gz", "desc-optcom_bold.nii.gz"]
    base = {"LARMORX_GPL_BIN": str(GPL)}
    for option, variable in (
        (["--n-threads", "2"], {"OMP_NUM_THREADS": "2"}),
        (["--upstream-defects", "reproduce"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}),
        (["--implementation", "replica"], {"LARMORX_IMPLEMENTATION": "replica"}),
    ):
        out = tmp_path / "o"
        args = [*t2smap, "--out-dir", str(out)]
        a = run([*cli, *option, *args], tmp_path, base, [])
        fa = sorted((p.relative_to(out).as_posix(), p.read_bytes()) for p in out.rglob("*") if p.is_file())  # fmt: skip
        subprocess.run(["rm", "-rf", str(out)], check=True)
        b = run([*cli, *args], tmp_path, {**base, **variable}, [])
        fb = sorted((p.relative_to(out).as_posix(), p.read_bytes()) for p in out.rglob("*") if p.is_file())  # fmt: skip
        subprocess.run(["rm", "-rf", str(out)], check=True)
        assert a == b, (option, a[0][2][-500:])
        assert a[0][0] == 0, a[0][2][-500:]
        assert fa == fb and any(name in dict(fa) for name in outputs), sorted(dict(fa))
    # --implementation clean-room: there is no clean-room original of t2smap yet (exit 127)
    (code, _, err), _ = run([*cli, "--implementation", "clean-room", *t2smap], tmp_path, base, [])
    assert code == 127 and b"no clean-room original of tedana t2smap" in err
