# SPDX-License-Identifier: Apache-2.0
"""The inputs fMRIPrep's template registration made for itself and did not keep, made again.

sMRIPrep registers the T1w to MNI152NLin2009cAsym with niworkflows' ``SpatialNormalization``
(``RobustMNINormalization``), a Python interface that writes, in its node directory, the
template times its brain mask (``fixed_masked.nii.gz``), the T1w times its mask
(``moving_masked.nii.gz``), cost-function masks where a lesion mask is given
(``*_cfm.nii.gz``: ds004884's stroke) and, by running ``antsAffineInitializer`` itself, the
initial transform (``transform.mat``); its ``command.txt`` names them, but nipype keeps none.
This module makes them again from the node's recorded inputs (``_report/report.rst``) with
niworkflows' own functions (``niworkflows.interfaces.norm.mask`` and ``create_cfm``, the
workspace's niworkflows), and the initial transform with the ANTs oracle's
``antsAffineInitializer`` at one thread (fMRIPrep's ran at the node's ``NSLOTS``, and
``antsAffineInitializer`` varies above one thread; its arguments are nipype's
``AffineInitializer`` defaults: ``3 fixed moving transform.mat 15.000000 0.100000 0 10``).

The files go to ``<workspace>/oracles/inputs/fmriprep-registration/<run>/`` (a suite's derived
inputs live outside the oracle cache's directory, ``docs/validation/oracle-cache.md``), are
made once and reused; the masked images are deterministic (the same bytes every time), the
transform comes through the oracle cache. Both programs of a comparison get the same files.
"""

from __future__ import annotations

import functools
import os
from pathlib import Path

from larmorx_validation.parity import fmriprep_nodes

#: The node whose inputs this module makes.
NODE = "anat_fit_wf/register_template_wf/registration"
#: nipype's ``AffineInitializer`` defaults, as its command line writes them.
AFFINE_INITIALIZER = ("15.000000", "0.100000", "0", "10")


def inputs_root() -> Path:
    """``<workspace>/oracles/inputs/fmriprep-registration``."""
    runs = fmriprep_nodes.root()
    if runs is None:
        raise FileNotFoundError("no fMRIPrep oracle runs (oracles/fmriprep-25.2.6)")
    return runs.parent / "inputs" / "fmriprep-registration"


def registration_call(run: str) -> fmriprep_nodes.NodeCall:
    """The template registration's node of ``run``."""
    for c in fmriprep_nodes.calls("antsRegistration"):
        if c.run == run and c.norm == NODE:
            return c
    raise FileNotFoundError(f"{run} has no template registration node")


@functools.cache
def node_inputs(run: str) -> dict[str, str]:
    """The registration node's inputs as nipype recorded them, host paths."""
    from larmorx_validation.stages.oracle import read_node

    call = registration_call(run)
    node = read_node(call.work, call.directory)
    r = fmriprep_nodes.oracle_run(run)
    return {k: r.host(v) if v.startswith("/") else v for k, v in node.inputs.items()}


def _template(name: str) -> Path:
    import larmorx_testdata as td

    return Path(td.get(f"templateflow/tpl-MNI152NLin2009cAsym/{name}"))


def _atomic(target: Path, write) -> Path:
    """``write(path)`` into a temporary name beside ``target``, then renamed over it (two
    workers making the same file write the same bytes; the rename makes either one whole)."""
    if not target.exists():
        tmp = target.with_name(f".{os.getpid()}.{target.name}")
        write(str(tmp))
        os.replace(tmp, target)
    return target


def masked(run: str) -> dict[str, Path]:
    """The masked images (and the cost-function masks where a lesion mask is given) the
    registration node made: ``{name in the command: file}``."""
    from niworkflows.interfaces.norm import create_cfm, mask

    out = inputs_root() / run
    out.mkdir(parents=True, exist_ok=True)
    ins = node_inputs(run)
    tpl = _template("tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz")
    tpl_mask = _template("tpl-MNI152NLin2009cAsym_res-01_desc-brain_mask.nii.gz")
    files: dict[str, Path] = {}
    if ins.get("explicit_masking") != "True":
        raise NotImplementedError(f"{run}: only explicit masking is made again")
    fixed = _atomic(out / "fixed_masked.nii.gz", lambda p: mask(str(tpl), str(tpl_mask), p))
    files["fixed_masked.nii.gz"] = fixed
    moving = _atomic(
        out / "moving_masked.nii.gz", lambda p: mask(ins["moving_image"], ins["moving_mask"], p)
    )
    files["moving_masked.nii.gz"] = moving
    lesion = ins.get("lesion_mask", "<undefined>")
    if lesion and lesion != "<undefined>":
        name = Path(ins["moving_mask"]).name.replace(".nii.gz", "_cfm.nii.gz")
        files[name] = _atomic(
            out / name,
            lambda p: create_cfm(
                ins["moving_mask"], lesion_mask=lesion, global_mask=True, out_path=p
            ),
        )
        name = tpl_mask.name.replace(".nii.gz", "_cfm.nii.gz")
        files[name] = _atomic(
            out / name,
            lambda p: create_cfm(str(tpl_mask), lesion_mask=None, global_mask=True, out_path=p),
        )
    return files


def made(run: str) -> dict[str, Path]:
    """The files the registration node's command names and nipype did not keep, made again:
    ``{name in the command: file}`` (:func:`masked`, and the initial transform)."""
    call = registration_call(run)
    files = masked(run)
    files["transform.mat"] = initial_transform(
        run, files["fixed_masked.nii.gz"], files["moving_masked.nii.gz"], inputs_root() / run
    )
    missing = {Path(p).name for p in call.missing()}
    unknown = missing - set(files)
    if unknown:
        raise FileNotFoundError(f"{run}: no way to make {sorted(unknown)} again")
    return files


def affine_initializer_args(fixed: Path, moving: Path, output: Path) -> list[str]:
    """``antsAffineInitializer``'s arguments as nipype's ``AffineInitializer`` writes them."""
    return ["3", str(fixed), str(moving), str(output), *AFFINE_INITIALIZER]


def initial_transform(run: str, fixed: Path, moving: Path, out: Path) -> Path:
    """The initial transform, by the ANTs oracle's ``antsAffineInitializer`` at one thread
    (through the oracle cache)."""
    from larmorx_validation import oracle_cache
    from larmorx_validation.parity.ants_programs import ants_bin

    target = out / "transform.mat"
    if target.exists():
        return target
    b = ants_bin()
    if b is None:
        raise FileNotFoundError("the ANTs oracle (oracles/ants-2.6.5)")
    env = {k: v for k, v in os.environ.items() if k not in ("ANTS_RANDOM_SEED", "LD_PRELOAD")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = "1"
    tmp = out / f".{os.getpid()}.transform.mat"
    argv = [str(b / "antsAffineInitializer"), *affine_initializer_args(fixed, moving, tmp)]
    p = oracle_cache.run(
        argv,
        env=env,
        policy=oracle_cache.Policy(threads=1, family="itk"),
        runner="fmriprep-registration-inputs",
    )
    if p.returncode != 0 or not tmp.exists():
        raise RuntimeError(f"antsAffineInitializer failed for {run}: {p.stderr[-500:]}")
    tmp.replace(target)
    return target


def mapped_args(call: fmriprep_nodes.NodeCall) -> list[str]:
    """The registration node's arguments with every file nipype did not keep replaced by the
    one made again."""
    import re

    files = made(call.run)
    node_dir = str(call.directory)

    def sub(m: re.Match[str]) -> str:
        p = m.group(0)
        if p.startswith(node_dir + "/") and not Path(p).exists():
            return str(files[Path(p).name])
        return p

    return [re.sub(r"[^\[\],\s]+", sub, a) for a in call.args]


__all__ = [
    "AFFINE_INITIALIZER",
    "NODE",
    "affine_initializer_args",
    "initial_transform",
    "inputs_root",
    "made",
    "mapped_args",
    "masked",
    "node_inputs",
    "registration_call",
]
