# SPDX-License-Identifier: Apache-2.0
"""Which AFNI build the AFNI suites, the evidence and random testing run (decision log
2026-10-27, "Newest upstream versions, updated in steps").

Two oracle builds are kept side by side, each from its release tag by
``scripts/build_afni_oracle.sh``: ``oracles/afni-26.3.05`` (the newest release, which the GPL
replicas follow) and ``oracles/afni-25.2.09`` (the release the ports were made from, kept so that
every record can show the step from one version to the next). Neither is ever modified.

The version is chosen, in this order, by

- ``LARMORX_AFNI_BIN`` (a directory of AFNI programs, or one program): that build, whatever its
  version (its ``BUILD_INFO.txt`` names it);
- ``LARMORX_AFNI_VERSION`` (``26.3.05`` or ``25.2.09``), which the ``--afni-version`` option of
  ``parity``, ``evidence`` and ``difftest`` sets (so that ``--jobs`` workers inherit it);
- the default of the implementation under test (:data:`DEFAULT`): the replicas follow the newest
  release; a clean-room original stays on the release its records were made with until a
  clean-room agent aligns it with the newest (``docs/next-steps.md``).

The **version step** (:func:`step_from`): where a suite runs the newest release, each case also
runs the previous one on the same arguments (served from the oracle-output cache when an earlier
record ran it), and the record lists the cases whose AFNI output changed, with the reason.
"""

from __future__ import annotations

import contextlib
import contextvars
import functools
import gzip
import os
import re
import shutil
from collections.abc import Iterator
from pathlib import Path

#: The kept builds, newest first: version -> its directory under the workspace.
BUILDS = {
    "26.3.05": "oracles/afni-26.3.05/bin",
    "25.2.09": "oracles/afni-25.2.09/bin",
}
NEWEST = "26.3.05"
PREVIOUS = "25.2.09"
ENV = "LARMORX_AFNI_VERSION"
#: The version each implementation is validated against by default. The clean-room originals
#: agree with AFNI 26.3.05 on every case their standard tiers hold except where 26.3.05 fixed a
#: defect they reproduce (AFNI-03, AFNI-05: `reproduce` gives 25.2.09's behaviour), so they stay
#: on 25.2.09 until a clean-room agent aligns them (docs/next-steps.md).
DEFAULT = {"replica": NEWEST, "original": PREVIOUS}

#: What changed between AFNI 25.2.09 and 26.3.05 in the code these programs run (the source
#: diff and the machine code of both builds: ``validation/investigations/version-afni/``).
CHANGES = {
    "AFNI-03": "AFNI 26.3.05 reads header slice code 2 (sequential decreasing) as NIfTI "
    "defines it, from slice_end down to slice_start; 25.2.09 gave every slice time 0",
    "AFNI-05": "AFNI 26.3.05 de-means each voxel of a -no_detrend pair with its own mean; "
    "25.2.09 de-meaned the first twice and shifted the second as raw values",
}

#: The implementation whose case runs now (:func:`using`): its default version applies.
_implementation: contextvars.ContextVar[str | None] = contextvars.ContextVar(
    "afni_implementation", default=None
)


def add_argument(parser) -> None:
    """``--afni-version`` for a command that runs AFNI (``parity``, ``evidence``, ``difftest``)."""
    parser.add_argument(
        "--afni-version",
        choices=tuple(BUILDS),
        default=None,
        help="the AFNI build the original programs run: "
        + " or ".join(f"{v} (oracles/afni-{v})" for v in BUILDS)
        + f"; sets {ENV} (the default: the replicas {DEFAULT['replica']}, the clean-room "
        f"originals {DEFAULT['original']}; LARMORX_AFNI_BIN names any other build)",
    )


def apply(args) -> None:
    """``--afni-version``, for this process and the workers it starts."""
    if getattr(args, "afni_version", None):
        os.environ[ENV] = args.afni_version


@functools.cache
def workspace() -> Path | None:
    """The workspace that holds ``oracles/`` (a parent of this file)."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


@contextlib.contextmanager
def using(implementation: str) -> Iterator[None]:
    """AFNI's runs within the block are those of ``implementation``'s case: its default version
    applies when neither ``LARMORX_AFNI_BIN`` nor ``LARMORX_AFNI_VERSION`` says otherwise."""
    token = _implementation.set(implementation)
    try:
        yield
    finally:
        _implementation.reset(token)


def selected(implementation: str | None = None) -> str:
    """The AFNI version for ``implementation`` (else the case running now, else the newest)."""
    if env := os.environ.get(ENV):
        if env not in BUILDS:
            raise SystemExit(f"{ENV}={env!r}: choose one of {', '.join(BUILDS)}")
        return env
    impl = implementation or _implementation.get()
    return DEFAULT.get(impl, NEWEST) if impl else NEWEST


def bin_dir(version: str | None = None) -> Path | None:
    """The chosen (or the given) version's ``bin`` directory, if it is built."""
    ws = workspace()
    if ws is None:
        return None
    d = ws / BUILDS[version or selected()]
    return d if d.is_dir() else None


def binary(program: str, version: str | None = None) -> str | None:
    """AFNI's ``program``: ``LARMORX_AFNI_BIN`` (unless a ``version`` is asked for), else the
    chosen version's build, else on the PATH."""
    if version is None and (env := os.environ.get("LARMORX_AFNI_BIN")):
        path = Path(env)
        directory = path if path.is_dir() else path.parent
        if not path.is_dir() and path.name == program:
            return str(path)
        candidate = directory / program
        return str(candidate) if candidate.is_file() else None
    d = bin_dir(version)
    if d is not None and (d / program).is_file():
        return str(d / program)
    return None if version is not None else shutil.which(program)


def version_of(binary_path: str | None) -> str:
    """The AFNI version of a build, from its ``BUILD_INFO.txt`` (``AFNI_26.3.05``)."""
    if binary_path is None:
        return "not found"
    info = Path(binary_path).resolve().parent.parent / "BUILD_INFO.txt"
    if info.is_file():
        m = re.search(r"AFNI version: (\S+)", info.read_text(encoding="utf-8"))
        if m:
            return m.group(1)
    return "unknown version"


def step_from(implementation: str | None = None) -> str | None:
    """The previous version to compare AFNI's own output with, or ``None``: only where the run
    uses the newest kept build (not ``LARMORX_AFNI_BIN``) and the previous build exists."""
    if os.environ.get("LARMORX_AFNI_BIN") or selected(implementation) != NEWEST:
        return None
    return PREVIOUS if binary_exists(PREVIOUS) else None


def binary_exists(version: str) -> bool:
    d = bin_dir(version)
    return d is not None and any(d.iterdir())


def step_label() -> str:
    return f"AFNI {PREVIOUS} → {NEWEST}"


STEP_CHECK = "AFNI's own step: " + f"{PREVIOUS} → {NEWEST}"


def reasons(args: list[str], inputs: list[Path]) -> list[str]:
    """The known changes (:data:`CHANGES`) a call meets: ``-no_detrend`` (AFNI-05), an input
    whose header says slice code 2 with usable slice fields (AFNI-03)."""
    out = []
    if "-no_detrend" in args:
        out.append("AFNI-05")
    if any(_slice_code_2(p) for p in inputs):
        out.append("AFNI-03")
    return out


def _slice_code_2(path: Path) -> bool:
    """Whether a NIfTI file's header says slice code 2 with the slice fields AFNI uses (a slice
    dimension of 3, a positive duration, a range within the volume)."""
    try:
        if path.suffix == ".gz":
            raw = _gunzip_head(path)
        else:
            with path.open("rb") as f:
                raw = f.read(600)
    except (OSError, EOFError, gzip.BadGzipFile):
        return False
    import numpy as np

    if len(raw) < 348:
        return False
    if raw[:4] == b"\x5c\x01\x00\x00":  # NIfTI-1, little-endian
        dim = np.frombuffer(raw[40:56], "<i2")
        slice_end = int(np.frombuffer(raw[120:122], "<i2")[0])
        slice_code = raw[122]
        dim_info = raw[39]
        duration = float(np.frombuffer(raw[132:136], "<f4")[0])
        slice_start = int(np.frombuffer(raw[74:76], "<i2")[0])
        nz = int(dim[3])
    elif raw[:4] == b"\x1c\x02\x00\x00" and len(raw) >= 540:  # NIfTI-2
        dim = np.frombuffer(raw[16:80], "<i8")
        slice_start = int(np.frombuffer(raw[224:232], "<i8")[0])
        slice_end = int(np.frombuffer(raw[232:240], "<i8")[0])
        duration = float(np.frombuffer(raw[208:216], "<f8")[0])
        slice_code = int(np.frombuffer(raw[496:500], "<i4")[0])
        dim_info = raw[524]
        nz = int(dim[3])
    else:
        return False
    return (
        slice_code == 2
        and (dim_info >> 4) & 3 == 3
        and duration > 0
        and 0 <= slice_start < slice_end < nz
    )


def _gunzip_head(path: Path) -> bytes:
    with gzip.open(path, "rb") as f:
        return f.read(600)


def compare_nifti(a: Path, b: Path) -> str | None:
    """How two of AFNI's NIfTI outputs differ, or ``None`` when they are the same: the header
    (348 or 540 bytes, ``vox_offset`` aside: it follows AFNI's history extension, which holds a
    random identifier and the date) and the data as stored."""
    import nibabel as nib
    import numpy as np

    ia, ib = nib.load(a), nib.load(b)
    out = []
    ha, hb = ia.header.copy(), ib.header.copy()
    ha["vox_offset"] = hb["vox_offset"] = 0
    if ha.binaryblock != hb.binaryblock:
        names = ha.structarr.dtype.names or ()
        fields = [k for k in names if ha.structarr[k].tobytes() != hb.structarr[k].tobytes()]
        out.append("header " + ", ".join(fields[:6]))
    da = np.ascontiguousarray(np.asanyarray(ia.dataobj.get_unscaled()))
    db = np.ascontiguousarray(np.asanyarray(ib.dataobj.get_unscaled()))
    if da.shape != db.shape or da.dtype != db.dtype:
        out.append(f"data {da.dtype}{da.shape} → {db.dtype}{db.shape}")
    elif da.tobytes() != db.tobytes():
        fa, fb = da.astype(np.float64), db.astype(np.float64)
        n = int(np.count_nonzero(~((fa == fb) | (np.isnan(fa) & np.isnan(fb)))))
        with np.errstate(invalid="ignore"):
            d = np.abs(fa - fb)
        m = float(np.nanmax(d)) if np.isfinite(d).any() else float("nan")
        out.append(f"{n} of {da.size} values differ (max |diff| {m:.6g})")
    return "; ".join(out) or None


#: The lines that name AFNI's build (its version banner, its compile date), which differ
#: between any two builds and say nothing of what the program did.
_BUILD_LINE = re.compile(r"AFNI version=|[Cc]ompile date = ")


def _without_build_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not _BUILD_LINE.search(line))


def step_detail(
    exit_codes: tuple[int, int],
    messages: tuple[str, str],
    outputs: tuple[list[str], list[str]],
    differences: dict[str, str],
    why: list[str],
) -> tuple[bool, str]:
    """(changed, the check's text) for AFNI's own step: the previous and the newest version's
    exit status, messages (the build lines and directories already masked), outputs written
    and how each output differs; ``why``: the known changes the call meets."""
    changes = []
    messages = (_without_build_lines(messages[0]), _without_build_lines(messages[1]))
    if exit_codes[0] != exit_codes[1]:
        changes.append(f"exit {exit_codes[0]} → {exit_codes[1]}")
    if messages[0] != messages[1]:
        xs, ys = messages[0].splitlines(), messages[1].splitlines()
        k = next((i for i, (x, y) in enumerate(zip(xs, ys, strict=False)) if x != y), None)
        k = min(len(xs), len(ys)) if k is None else k
        x = xs[k] if k < len(xs) else "(nothing)"
        y = ys[k] if k < len(ys) else "(nothing)"
        changes.append(f"messages, line {k + 1}: {x!r} → {y!r}")
    if outputs[0] != outputs[1]:
        changes.append(f"outputs {outputs[0]} → {outputs[1]}")
    changes += [f"{name}: {d}" for name, d in sorted(differences.items())]
    if not changes:
        return False, "identical: exit status, messages and outputs"
    known = "; ".join(f"{r}: {CHANGES[r]}" for r in why) if why else "no known reason (a finding)"
    return True, "; ".join(changes) + f" [{known}]"


def step_highlights(results: list) -> list[str]:
    """The record's line on AFNI's own step between the two versions: how many cases ran both,
    which changed, and why."""
    steps = [(r, c) for r in results for c in r.checks if c.name == STEP_CHECK]
    if not steps:
        return []
    changed = [(r, c) for r, c in steps if c.value == "changed"]
    line = (
        f"- **AFNI's own step, {step_label()}** (both builds on the same arguments; "
        f"the previous one's runs from the oracle-output cache where an earlier record made them): "
        f"{len(steps) - len(changed)} of {len(steps)} cases identical (exit status, messages, "
        "every output)"
    )
    if not changed:
        return [line + "."]
    unknown = [r.case for r, c in changed if "no known reason" in c.detail]
    by_reason: dict[str, list[str]] = {}
    for r, c in changed:
        for key in CHANGES:
            if f"{key}:" in c.detail:
                by_reason.setdefault(key, []).append(r.case)
    parts = [
        f"{len(v)} by {k} ({CHANGES[k]}): " + ", ".join(f"`{x}`" for x in v)
        for k, v in sorted(by_reason.items())
    ]
    if unknown:
        parts.append(
            f"{len(unknown)} without a known reason (findings): "
            + ", ".join(f"`{x}`" for x in unknown)
        )
    return [line + f"; {len(changed)} changed: " + "; ".join(parts) + "."]


def record_line(implementation: str) -> str:
    """The sentence a record's notes carry about the version (and its step)."""
    v = selected(implementation)
    if os.environ.get("LARMORX_AFNI_BIN"):
        v = "LARMORX_AFNI_BIN's build"
    step = step_from(implementation)
    text = f"AFNI version: {v} (`--afni-version`, `{ENV}`; builds: {', '.join(BUILDS)})."
    if step:
        text += (
            f" Each case also ran AFNI {step} on the same arguments (from the oracle-output cache "
            f"where an earlier record ran it): the check '{STEP_CHECK}' says whether AFNI's own "
            "exit status, messages and outputs changed, and why."
        )
    return text
