# SPDX-License-Identifier: Apache-2.0
"""Which FSL installation the FSL suites, the evidence and random testing run (decision log
2026-10-27, "Newest upstream versions, updated in steps"; the model is :mod:`afni_versions`).

Two official FSL installations are kept side by side, neither ever modified: ``~/fsl-6.0.7.23``
(the newest release, which the replicas follow since 2026-10-29) and ``~/fsl`` (FSL 6.0.7.17,
the release the replicas were first made from, kept so that every record can show the step from
one version to the next). FSL is never put on the ``PATH``: its programs are called by full path.

The installation is chosen, in this order, by

- ``LARMORX_FSLDIR`` (any FSL installation, whatever its version: its ``etc/fslversion`` names
  it; ``LARMORX_FSL_DIR``, the BET suite's earlier name, is read too);
- ``LARMORX_FSL_VERSION`` (``6.0.7.23`` or ``6.0.7.17``), which the ``--fsl-version`` option of
  ``parity``, ``evidence`` and ``difftest`` sets (so that ``--jobs`` workers inherit it);
- the default of the implementation under test (:data:`DEFAULT`).

mcflirt's recorded runs (``scripts/run_mcflirt_oracle.py``), which the clean-room original's
suite compares with and the replica's suite replays, are kept per version
(:data:`RECORDINGS`).

The **version step**: where a suite runs the newest release, each case also runs (or reads the
recording of) the previous one on the same arguments, served from the oracle-output cache when
an earlier record ran it, and the record lists the cases whose FSL output changed, with the
reason (:data:`CHANGES`).
"""

from __future__ import annotations

import contextlib
import contextvars
import functools
import os
import re
from collections.abc import Iterator
from pathlib import Path

#: The kept installations, newest first: version -> its directory under the home directory.
INSTALLS = {
    "6.0.7.23": "fsl-6.0.7.23",
    "6.0.7.17": "fsl",
}
NEWEST = "6.0.7.23"
PREVIOUS = "6.0.7.17"
ENV = "LARMORX_FSL_VERSION"
DIR_ENV = "LARMORX_FSLDIR"
#: The BET suite's earlier name for :data:`DIR_ENV`, still read.
DIR_ENV_OLD = "LARMORX_FSL_DIR"
#: The version each implementation is validated against by default: the replicas follow the
#: newest release; the clean-room head-motion correction too (it agrees with mcflirt 6.0.7.23 as
#: with 6.0.7.17 wherever 6.0.7.23 changed nothing it implements: docs/validation/mri-hmc.md).
DEFAULT = {"replica": NEWEST, "original": NEWEST}
#: mcflirt's recorded runs, per version (under the workspace).
RECORDINGS = {
    "6.0.7.23": "oracles/fsl-6.0.7.23/mcflirt",
    "6.0.7.17": "oracles/fsl-6.0.7/mcflirt",
}

#: What changed between FSL 6.0.7.17 and 6.0.7.23 in the code these programs run (the source
#: diff and the binaries: ``validation/investigations/version-fsl/``). Keys are the register's
#: entries, or a short name where the change is not a defect.
CHANGES = {
    "FSL-59": "FSL 6.0.7.23's mcflirt applies -sinc_final and -nn_final with trilinear "
    "interpolation (newimage 2601.0 keeps each volume's interpolation method from its first "
    "access); 6.0.7.17 used the method asked for",
    "FSL-41": "FSL 6.0.7.23's bet2 takes the robust maximum as the median when no voxel lies "
    "between the 2 and 98 % thresholds (a binary image); 6.0.7.17 read before an empty list",
    "FSL-60": "FSL 6.0.7.23 opens an image whose path holds '.img' or '.hdr' before its "
    "extension under another name (NewNifti 5.0.0's replaceSuffix)",
    "output-types": "FSL 6.0.7.23 adds the zstd and bzip2 output types (FSLOUTPUTTYPE "
    "NIFTI_ZST, NIFTI_BZ2, ...) and lists every type in the message for an unknown one",
    "binary-mask": "FSL 6.0.7.23's bet2 has the --binaryMask option (the mesh from a mask)",
    "compressed-input": "FSL 6.0.7.23 reads zstd and bzip2 compressed images",
    "build-string": "newimage 2601.0 writes its own build string into an image's descrip "
    "('2601.0 2025-12-08T15:20:57Z'; 2203.12 wrote '2203.12-dirty 2024-02-01T16:17:47+00:00')",
    "usage-id": "utils 2412.6 prints its build ID in every usage text ('Part of FSL (ID: "
    "2412.6-dirty)'; 2203.5 printed an empty pair of quotes)",
    "arma-lapack": "FSL 6.0.7.23's Armadillo 14.2.1 inverts 4 x 4 matrices and solves with them "
    "through LAPACK (OpenBLAS), where Armadillo 10.5.1 used its closed-form inverse: the last "
    "bits of values computed from nearly equal matrices change",
}

#: The implementation whose case runs now (:func:`using`): its default version applies.
_implementation: contextvars.ContextVar[str | None] = contextvars.ContextVar(
    "fsl_implementation", default=None
)
#: Set by :func:`without_step`: random testing compares with one version only.
_no_step: contextvars.ContextVar[bool] = contextvars.ContextVar("fsl_no_step", default=False)


def add_argument(parser) -> None:
    """``--fsl-version`` for a command that runs FSL (``parity``, ``evidence``, ``difftest``)."""
    parser.add_argument(
        "--fsl-version",
        choices=tuple(INSTALLS),
        default=None,
        help="the FSL installation the original programs run: "
        + " or ".join(f"{v} (~/{d})" for v, d in INSTALLS.items())
        + f"; sets {ENV} (the default: the replicas {DEFAULT['replica']}, the clean-room "
        f"originals {DEFAULT['original']}; {DIR_ENV} names any other installation)",
    )


def apply(args) -> None:
    """``--fsl-version``, for this process and the workers it starts."""
    if getattr(args, "fsl_version", None):
        os.environ[ENV] = args.fsl_version


@functools.cache
def workspace() -> Path | None:
    """The workspace that holds ``oracles/`` (a parent of this file)."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


@contextlib.contextmanager
def using(implementation: str) -> Iterator[None]:
    """FSL's runs within the block are those of ``implementation``'s case: its default version
    applies when neither ``LARMORX_FSLDIR`` nor ``LARMORX_FSL_VERSION`` says otherwise."""
    token = _implementation.set(implementation)
    try:
        yield
    finally:
        _implementation.reset(token)


@contextlib.contextmanager
def without_step() -> Iterator[None]:
    """No version step within the block (random testing: each case runs FSL once)."""
    token = _no_step.set(True)
    try:
        yield
    finally:
        _no_step.reset(token)


def _dir_override() -> Path | None:
    for name in (DIR_ENV, DIR_ENV_OLD):
        if env := os.environ.get(name):
            return Path(env).expanduser()
    return None


def selected(implementation: str | None = None) -> str:
    """The FSL version for ``implementation`` (else the case running now, else the newest)."""
    if env := os.environ.get(ENV):
        if env not in INSTALLS:
            raise SystemExit(f"{ENV}={env!r}: choose one of {', '.join(INSTALLS)}")
        return env
    impl = implementation or _implementation.get()
    return DEFAULT.get(impl, NEWEST) if impl else NEWEST


def fsldir(version: str | None = None) -> Path:
    """The chosen (or the given) version's ``FSLDIR``: ``LARMORX_FSLDIR`` unless a ``version`` is
    asked for, else ``~/<install>``. The directory may be missing (:func:`installed`)."""
    if version is None and (d := _dir_override()) is not None:
        return d
    return Path.home() / INSTALLS[version or selected()]


def installed(version: str | None = None) -> bool:
    return (fsldir(version) / "bin").is_dir()


def program(name: str, version: str | None = None) -> Path:
    """FSL's ``name`` (``bin/<name>``) in the chosen or given installation."""
    return fsldir(version) / "bin" / name


def version_of(directory: Path | None = None) -> str:
    """The FSL version of an installation (``etc/fslversion``: ``6.0.7.23``)."""
    d = directory if directory is not None else fsldir()
    f = d / "etc" / "fslversion"
    if not f.is_file():
        return "not found" if not d.is_dir() else "unknown version"
    return f.read_text(encoding="utf-8").split(":")[0].strip()


def package_versions(directory: Path | None = None, names: tuple[str, ...] = ()) -> str:
    """``fsl-bet2 2111.9, fsl-newimage 2601.0, ...``: the conda packages of an installation."""
    d = directory if directory is not None else fsldir()
    out = []
    for name in names:
        hits = sorted((d / "conda-meta").glob(f"{name}-[0-9]*.json"))
        m = re.match(re.escape(name) + r"-([0-9][^-]*)-", hits[-1].name) if hits else None
        out.append(f"{name} {m.group(1) if m else '?'}")
    return ", ".join(out)


def recording(version: str | None = None) -> Path | None:
    """mcflirt's recorded runs of the chosen (or given) version, if they were recorded."""
    ws = workspace()
    if ws is None:
        return None
    v = version or selected()
    if version is None and _dir_override() is not None:
        v = version_of(_dir_override())
    rel = RECORDINGS.get(v)
    if rel is None:
        return None
    d = ws / rel
    return d if (d / "manifest.json").is_file() else None


def step_from(implementation: str | None = None) -> str | None:
    """The previous version to compare FSL's own output with, or ``None``: only where the run
    uses the newest kept installation (not ``LARMORX_FSLDIR``) and the previous one exists."""
    if _no_step.get() or _dir_override() is not None or selected(implementation) != NEWEST:
        return None
    return PREVIOUS if installed(PREVIOUS) else None


def step_label() -> str:
    return f"FSL {PREVIOUS} → {NEWEST}"


STEP_CHECK = "FSL's own step: " + f"{PREVIOUS} → {NEWEST}"


def reasons(args: list[str], env: dict[str, str] | None = None) -> list[str]:
    """The known changes (:data:`CHANGES`) a call may meet, from its arguments and
    environment."""
    out = []
    if "-sinc_final" in args or "-nn_final" in args:
        out.append("FSL-59")
    if "--binaryMask" in args:
        out.append("binary-mask")
    otype = (env or {}).get("FSLOUTPUTTYPE")
    if otype is not None and not otype.endswith(("NIFTI", "NIFTI_PAIR", "NIFTI2", "_GZ")):
        out.append("output-types")
    if otype is not None and otype.endswith(("_ZST", "_BZ2")):
        out.append("output-types")
    if any(a.endswith((".zst", ".bz2")) for a in args):
        out.append("compressed-input")
    if any(re.search(r"\.(img|hdr)[^/]*/", a) for a in args):
        out.append("FSL-60")
    return sorted(set(out))


#: The lines that name FSL's build or version, which differ between any two installations.
_BUILD_LINE = re.compile(r"FSL [Vv]ersion|fslversion|\bFSL 6\.0\.7\.\d+")


def _without_build_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not _BUILD_LINE.search(line))


#: A text output whose numbers differ by at most 1e-12 or in a NaN's sign: Armadillo 14.2.1's
#: LAPACK path (4 x 4 inverses and solves) in FSL 6.0.7.23.
ARMA_ONLY = "numbers within 1e-12 or NaN signs only (Armadillo 14.2.1's LAPACK arithmetic)"
#: A difference that only FSL's build string makes: an image header whose ``descrip`` alone
#: changed (newimage writes its build string there).
BUILD_STRING_ONLY = "descrip only (FSL's build string)"
#: Changes that are explained by what they are, whatever the call.
_MESSAGE_REASONS = (
    ("Part of FSL (ID:", "usage-id"),
    ("Legal values are:", "output-types"),
)


def step_detail(
    exit_codes: tuple[int, int],
    messages: tuple[str, str],
    outputs: tuple[list[str], list[str]],
    differences: dict[str, str],
    why: list[str],
) -> tuple[bool, str]:
    """(changed, the check's text) for FSL's own step: the previous and the newest version's
    exit status, messages (directories already masked), outputs written and how each output
    differs; ``why``: the known changes the call meets. Every change is explained on its own: a
    header whose ``descrip`` alone changed by FSL's build string, a message line by the usage's
    build ID or the output-type list, anything else by the call's known changes; a change none
    explains is a finding."""
    changes: list[tuple[str, list[str]]] = []
    messages = (_without_build_lines(messages[0]), _without_build_lines(messages[1]))
    if exit_codes[0] != exit_codes[1]:
        changes.append((f"exit {exit_codes[0]} → {exit_codes[1]}", list(why)))
    if messages[0] != messages[1]:
        xs, ys = messages[0].splitlines(), messages[1].splitlines()
        k = next((i for i, (x, y) in enumerate(zip(xs, ys, strict=False)) if x != y), None)
        k = min(len(xs), len(ys)) if k is None else k
        x = xs[k] if k < len(xs) else "(nothing)"
        y = ys[k] if k < len(ys) else "(nothing)"
        own = [r for pattern, r in _MESSAGE_REASONS if pattern in x or pattern in y]
        # the lines after the first difference are explained too only if they all are
        rest = [
            (a, b)
            for a, b in zip(xs[k + 1 :], ys[k + 1 :], strict=False)
            if a != b and not any(p in a or p in b for p, _ in _MESSAGE_REASONS)
        ]
        reasons_here = own if own and not rest and len(xs) == len(ys) else []
        changes.append(
            (f"messages, line {k + 1}: {x[:160]!r} → {y[:160]!r}", reasons_here or list(why))
        )
    if sorted(outputs[0]) != sorted(outputs[1]):
        changes.append((f"outputs {sorted(outputs[0])} → {sorted(outputs[1])}", list(why)))
    for name, d in sorted(differences.items()):
        own = {BUILD_STRING_ONLY: ["build-string"], ARMA_ONLY: ["arma-lapack"]}.get(d)
        changes.append((f"{name}: {d}", own or list(why)))
    if not changes:
        return False, "identical: exit status, messages and outputs"
    used = sorted({r for _, rs in changes for r in rs})
    unexplained = [c for c, rs in changes if not rs]
    known = "; ".join(f"{r}: {CHANGES[r]}" for r in used)
    if unexplained:
        known = (
            (known + "; " if known else "")
            + "no known reason (a finding): "
            + "; ".join(unexplained)
        )
    return True, "; ".join(c for c, _ in changes) + f" [{known}]"


def step_highlights(results: list) -> list[str]:
    """The record's line on FSL's own step between the two versions: how many cases ran both,
    which changed, and why."""
    steps = [(r, c) for r in results for c in r.checks if c.name == STEP_CHECK]
    if not steps:
        return []
    changed = [(r, c) for r, c in steps if c.value == "changed"]
    line = (
        f"- **FSL's own step, {step_label()}** (both installations on the same arguments; the "
        f"previous one's runs from the oracle-output cache or its recording where an earlier "
        f"record made them): {len(steps) - len(changed)} of {len(steps)} cases identical (exit "
        "status, messages, every output)"
    )
    if not changed:
        return [line + "."]
    unknown = [r.case for r, c in changed if "no known reason" in c.detail]
    by_reason: dict[str, list[str]] = {}
    for r, c in changed:
        tail = c.detail.rsplit(" [", 1)[-1]
        for key in CHANGES:
            if tail.startswith(f"{key}:") or f"; {key}:" in tail:
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


def record_line(implementation: str, with_step: bool = True) -> str:
    """The sentence a record's notes carry about the version (and its step, where the suite
    runs it)."""
    d = _dir_override()
    v = f"LARMORX_FSLDIR's installation ({version_of(d)})" if d else selected(implementation)
    step = step_from(implementation) if with_step else None
    text = (
        f"FSL version: {v} (`--fsl-version`, `{ENV}`; installations: "
        + ", ".join(f"{k} `~/{p}`" for k, p in INSTALLS.items())
        + ")."
    )
    if step:
        text += (
            f" Each case also ran FSL {step} on the same arguments (from the oracle-output cache "
            f"or its recording where an earlier record ran it): the check '{STEP_CHECK}' says "
            "whether FSL's own exit status, messages and outputs changed, and why."
        )
    return text
