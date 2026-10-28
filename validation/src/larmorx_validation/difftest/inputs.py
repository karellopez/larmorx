# SPDX-License-Identifier: Apache-2.0
"""The input files of random cases and permanent cases, built from their recipes on demand.

A recipe's inputs are specs (:mod:`larmorx_validation.difftest.gen`); the parity suites name
them by a key that this module resolves to a file, building it the first time:

- ``run/<digest>/<name>``: an input of a case running in this process (registered with
  :func:`register` before the case runs);
- ``case/<tool>/<case id>/<name>``: an input of a permanent case
  (``validation/difftest/cases/<tool>/<case id>.json``), read from its recipe, so a parity
  worker process finds it without any registration.

The suites reach this module through their own input placeholders: ``{gen:dt:<key>}`` in the
ANTs suites (:func:`larmorx_validation.parity.ants_inputs.path`), ``dt:<key>`` as a catalog
path in the 3dTshift, antsApplyTransforms, NIfTI and resampling suites, and an AFNI tool spec's
``inputs`` function. Files are written into this process's temporary directory and removed by
:func:`release` when a random case ends (permanent cases' files stay for the process's life).
"""

from __future__ import annotations

import functools
import json
import shutil
import tempfile
from pathlib import Path

from larmorx_validation.difftest import gen

PREFIX = "dt:"
#: The permanent cases, in the repository (``validation/difftest/cases/<tool>/<id>.json``).
CASES = Path(__file__).resolve().parents[3] / "difftest" / "cases"

_REGISTERED: dict[str, dict] = {}


@functools.cache
def _dir() -> Path:
    # The prefix ends in "_": mkdtemp's random part (letters, digits, "_") then never follows a
    # dash, so the inputs' paths hold no option-like text such as "-h" (bet2 reads the text of
    # its one argument: parity.fsl_bet.BET2_HELP_TEXT; machine-move-proof.md).
    return Path(tempfile.mkdtemp(prefix="lx-difftest-inputs_"))


def register(inputs: dict[str, dict]) -> str:
    """Registers a case's input specs; returns the key prefix (``run/<digest>``) its names are
    found under."""
    prefix = f"run/{gen.digest(inputs)}"
    for name, spec in inputs.items():
        _REGISTERED[f"{prefix}/{name}"] = spec
    return prefix


def key(prefix: str, name: str) -> str:
    return f"{prefix}/{name}"


def _spec(k: str) -> dict:
    if k in _REGISTERED:
        return _REGISTERED[k]
    if k.startswith("case/"):
        _, tool, rest = k.split("/", 2)
        case_id, name = rest.rsplit("/", 1)
        recipe = json.loads((CASES / tool / f"{case_id}.json").read_text(encoding="utf-8"))[
            "recipe"
        ]
        return recipe["inputs"][name]
    raise KeyError(f"unknown difftest input {k!r}")


def suffix_of(spec: dict) -> str:
    if spec.get("kind", "nifti") == "nifti":
        return spec.get("suffix", ".nii.gz")
    return spec.get("suffix", "")


def path(k: str) -> Path:
    """The file of input ``k`` (``run/...`` or ``case/...``), built on first use."""
    k = k.removeprefix(PREFIX)
    spec = _spec(k)
    target = _dir() / k.replace(":", "_")
    target = target.with_name(target.name + suffix_of(spec))
    if not target.exists():
        materialize(spec, target)
    return target


def materialize(spec: dict, target: Path) -> Path:
    """Writes the file of ``spec`` at ``target``."""
    target.parent.mkdir(parents=True, exist_ok=True)
    kind = spec.get("kind", "nifti")
    if kind == "nifti":
        return gen.write_nifti(spec, target)
    if kind == "text":
        target.write_text(spec["text"], encoding="utf-8")
        return target
    if kind == "itk-text":
        target.write_text(gen.itk_text(spec["transforms"]), encoding="utf-8")
        return target
    if kind == "itk-mat":
        gen.write_itk_mat(spec["transform"], target)
        return target
    if kind == "itk-h5":
        import larmorx as lx

        parts = [_itk_part(t) for t in spec["transforms"]]
        lx.transforms.write(target, parts)
        return target
    raise ValueError(f"unknown input kind {kind!r}")


def _itk_part(t: dict):
    """An ``lx.transforms`` part from a transform spec (the HDF5 writer's input)."""
    import numpy as np

    import larmorx as lx

    return lx.transforms.ItkTransform(
        t["type"],
        np.asarray(t["parameters"], dtype=np.float64),
        np.asarray(t["fixed"], dtype=np.float64),
    )


def release(prefix: str) -> None:
    """Removes the files of a random case's inputs (``run/<digest>``) and forgets them."""
    d = _dir() / prefix
    if d.is_dir():
        shutil.rmtree(d, ignore_errors=True)
    for k in [k for k in _REGISTERED if k.startswith(prefix + "/")]:
        del _REGISTERED[k]


__all__ = ["CASES", "PREFIX", "key", "materialize", "path", "register", "release", "suffix_of"]
