# SPDX-License-Identifier: Apache-2.0
"""SDC-13 measured (rule D): what sdcflows' fieldmap-less estimation gives when its
``Deoblique`` acts as meant (the threshold 0.5 in degrees, not radians; the plumb grid keeping
the image's centre in millimetres and each voxel axis on its closest world axis) against
sdcflows' own behaviour (``Deoblique`` a pass-through below 28.6 degrees), on one of
fMRIPrep's ``--use-syn-sdc`` estimators, with the estimation's own spread (another ANTs seed)
as the scale.

    python -P probe_sdc13.py <syn_preprocessing folder of a run> <out folder> [--threads N]
        [--seeds 50495,1234]

Three runs of larmorx's ``syn_preprocessing_workflow`` + ``syn_sdc_workflow`` on the
estimator's own inputs (fMRIPrep's T1w, mask, MNI transform, BOLD and SBRef): A = sdcflows'
(first seed), B = ``Deoblique`` as meant (first seed), C = sdcflows' (second seed). Each
voxel of the EPI reference is the same voxel in the three (B's plumb grid only moves the
voxels in space), so the field maps (Hz) are compared voxel by voxel inside the anatomical
mask (brought onto the EPI's grid), and each unwarped EPI reference is compared with its run's
anatomical reference on the EPI's grid (normalised mutual information, 64 bins, and Pearson's
r inside the mask), against the distorted reference's. Prints the numbers and a verdict line.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import check_syn_preprocessing as csp  # noqa: E402


def deoblique_as_meant(in_file, in_affine=None, *, out_dir=".", n_threads: int = 1) -> str:
    """``_deoblique`` with the threshold in degrees and a plumb grid that keeps the image's
    centre (mm) and puts each voxel axis on its closest world axis (nibabel's orientation)."""
    from larmorx import _nipype
    from larmorx.image import Image
    from larmorx.io.nibabel_header import io_orientation, load_nibabel, set_xform
    from larmorx.io.nibabel_writer import save_nibabel
    from larmorx.io.nifti import NiftiHeader
    from larmorx.sdcflows import obliquity

    in_file = os.fspath(in_file)
    nii = load_nibabel(in_file, n_threads=n_threads)
    aff = np.asarray(nii.affine, dtype=np.float64)
    if np.all(obliquity(aff) < np.deg2rad(0.5)):
        return in_file
    shape = np.array(nii.shape[:3], dtype=np.float64)
    zooms = np.asarray(nii.header.pixdim, dtype=np.float32)[1:4].astype(np.float64)
    rot = np.zeros((3, 3))
    for vox, (world, sign) in enumerate(io_orientation(aff)):
        rot[int(world), vox] = sign * zooms[vox]
    centre = (shape - 1) / 2
    newaff = np.eye(4)
    newaff[:3, :3] = rot
    newaff[:3, 3] = aff[:3, :3] @ centre + aff[:3, 3] - rot @ centre
    fields = set_xform(nii.header.to_dict(), "qform", newaff, 1)
    fields = set_xform(fields, "sform", newaff, 1)
    os.makedirs(out_dir, exist_ok=True)
    out = _nipype.fname_presuffix(in_file, suffix="_plumb", newpath=out_dir)
    save_nibabel(Image(nii.data, newaff, NiftiHeader.from_dict(fields)), out, n_threads=n_threads)
    return out


def on_epi(src: str, epi: str, interp: str, cwd: Path, nt: int) -> np.ndarray:
    """``src`` (anatomical reference or mask, on the EPI's sampling grid) on the EPI's own grid."""
    from larmorx.io.nibabel_header import load_nibabel
    from larmorx.sdcflows.syn import _apply_transforms

    cwd.mkdir(parents=True, exist_ok=True)
    out, _ = _apply_transforms(str(cwd), src, epi, ["identity"], None, interp, "probe", "fix", nt)
    return np.asarray(load_nibabel(out).data, dtype=np.float64)


def nmi(a: np.ndarray, b: np.ndarray, bins: int = 64) -> float:
    h, _, _ = np.histogram2d(a, b, bins=bins)
    p = h / h.sum()
    px, py = p.sum(1), p.sum(0)

    def ent(q):
        q = q[q > 0]
        return float(-(q * np.log(q)).sum())

    return (ent(px) + ent(py)) / ent(p.ravel())


def _3d(path: str) -> np.ndarray:
    from larmorx.io.nibabel_header import load_nibabel

    a = np.asarray(load_nibabel(path).data, dtype=np.float64)
    return a.reshape(a.shape[:3])


def one(name: str, inp: dict, out: Path, seed: int, nt: int, as_meant: bool) -> dict:
    """One run (its files' paths kept in ``<out>/<name>/paths.json``: a later call reuses
    them), and its numbers."""
    import json

    from larmorx import sdcflows as sd
    from larmorx.sdcflows import syn as synmod

    record = out / name / "paths.json"
    if record.exists():
        paths = json.loads(record.read_text(encoding="utf-8"))
    else:
        original = synmod.deoblique
        if as_meant:
            synmod.deoblique = deoblique_as_meant
        try:
            t0 = time.perf_counter()
            pre = sd.syn_preprocessing_workflow(
                **inp,
                out_dir=out / name,
                auto_bold_nss=True,
                omp_nthreads=nt,
                ants_random_seed=seed,
                n_threads=nt,
            )
            res = sd.syn_sdc_workflow(
                pre.epi_ref,
                pre.epi_mask,
                pre.anat_ref,
                pre.anat_mask,
                pre.sd_prior,
                out_dir=out / name,
                omp_nthreads=nt,
                ants_random_seed=seed,
                n_threads=nt,
            )
            dt = time.perf_counter() - t0
        finally:
            synmod.deoblique = original
        paths = {
            "time": dt,
            "epi": pre.epi_ref[0],
            "reference": pre.epi_reference.epi_ref_file,
            "anat_ref": pre.anat_ref,
            "anat_mask": pre.anat_mask,
            "fmap": res.fmap,
            "fmap_ref": res.fmap_ref,
        }
        record.write_text(json.dumps(paths, indent=1), encoding="utf-8")
    epi = paths["epi"]
    anat = on_epi(paths["anat_ref"], epi, "Linear", out / name / "probe_anat", nt)
    mask = on_epi(paths["anat_mask"], epi, "MultiLabel", out / name / "probe_mask", nt) > 0
    anat, mask = anat.reshape(mask.shape[:3]), mask.reshape(mask.shape[:3])
    dist, corr, field = _3d(epi), _3d(paths["fmap_ref"]), _3d(paths["fmap"])
    a = anat[mask]
    return {
        "time": paths["time"],
        "plumb": epi != paths["reference"],
        "field": field,
        "mask": mask,
        "nmi_dist": nmi(dist[mask], a),
        "nmi_corr": nmi(corr[mask], a),
        "r_dist": float(np.corrcoef(dist[mask], a)[0, 1]),
        "r_corr": float(np.corrcoef(corr[mask], a)[0, 1]),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("preproc")
    ap.add_argument("out")
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--seeds", default="")
    a = ap.parse_args()
    wf = Path(a.preproc)
    inp = csp.workflow_inputs(wf)
    from check_bold_fmap import ants_seed

    seeds = [int(s) for s in a.seeds.split(",") if s] or [ants_seed(wf / "epi2anat"), 20261109]
    out = Path(a.out)
    runs = {
        "A_sdcflows": one("A_sdcflows", inp, out, seeds[0], a.threads, False),
        "B_as_meant": one("B_as_meant", inp, out, seeds[0], a.threads, True),
        "C_sdcflows_seed2": one("C_sdcflows_seed2", inp, out, seeds[1], a.threads, False),
    }
    for k, r in runs.items():
        print(
            f"{k}: plumb EPI {r['plumb']}; {r['time']:.0f} s; NMI distorted {r['nmi_dist']:.4f}"
            f" -> unwarped {r['nmi_corr']:.4f}; r {r['r_dist']:.4f} -> {r['r_corr']:.4f}; "
            f"|field| p95 {np.percentile(np.abs(r['field'][r['mask']]), 95):.2f} Hz"
        )
    m = runs["A_sdcflows"]["mask"] & runs["B_as_meant"]["mask"] & runs["C_sdcflows_seed2"]["mask"]
    fa, fb, fc = (runs[k]["field"][m] for k in runs)
    for what, d in (("A vs B (Deoblique as meant)", fa - fb), ("A vs C (another seed)", fa - fc)):
        d = np.abs(d)
        print(
            f"{what}: field |diff| mean {d.mean():.3f} Hz, p95 {np.percentile(d, 95):.3f} Hz, "
            f"max {d.max():.3f} Hz ({m.sum()} voxels)"
        )
    gain = runs["B_as_meant"]["nmi_corr"] - runs["A_sdcflows"]["nmi_corr"]
    spread = abs(runs["C_sdcflows_seed2"]["nmi_corr"] - runs["A_sdcflows"]["nmi_corr"])
    sdc = runs["A_sdcflows"]["nmi_corr"] - runs["A_sdcflows"]["nmi_dist"]
    print(
        f"VERDICT NMI gain of B over A {gain:+.4f} (the correction's own gain over the distorted "
        f"reference {sdc:+.4f}; another seed moves it by {spread:.4f})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
