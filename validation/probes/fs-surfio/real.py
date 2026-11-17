# SPDX-License-Identifier: Apache-2.0
"""mriflood.cpp's surface functions on the recorded heads: FreeSurfer 8.2.0's official code
against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fssurfio
is a black-box run of FreeSurfer's code).

Runs the calls of the pipeline's programs on three heads' recorded surfaces and volumes
(``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/subjects/<subject>/``) through
``fssurfio`` (its ``flr`` command) by the oracle-output cache at one thread and through the port
with ``reproduce`` and ``fix``: the interior of each hemisphere's white and pial surfaces in a copy
of ``aseg.mgz`` (``MRISfillInterior(surface, 0, volume)``, mri_surf2volseg's call), the ribbon
between each hemisphere's white and pial surfaces in ``orig.mgz``'s geometry (``MRISribbon``,
mris_volmask's), and the white surface's interior in its bounding volume at 0.5 mm
(``MRISmakeBoundingVolume`` then ``MRISfillInterior``, mris_place_surface's T2 and FLAIR steps).
Compares the result lines (extent, type, frames, geometry, the count of voxels that are not 0 and
the FNV-1a of the voxels). With ``--curv`` (relay 35 of track D): each head's recorded per-vertex
files (``CURVS``) read into its white surface by ``MRISreadCurvatureFile``, written by
``MRISwriteCurvature`` in the new binary format, MGZ, compressed NIfTI, GIFTI and VTK, the new-format
file compared with the recorded one byte for byte, and the MGZ and new-format files read back. With
``--pvw`` (relay 36 of track D): each head's recorded per-vertex files (``VALS``) read into its
white surface's values by ``MRISreadValues``, the surface's patch (``MRISwritePatch``), normals
(``MRISwriteNormals``) and values into a volume (``MRISwriteIntoVolume``) written, the normals and
the values read back from that volume (``MRISreadNormals``, ``MRISreadValues``). With ``--vps``
(relay 36 of track D): each head's white surface with its sphere's canonical coordinates
(``MRISreadCanonicalCoordinates``), smoothwm's original properties (``MRISreadOriginalProperties``),
the pial and white coordinates and the pial positions (``MRISreadVertexPositions``). With ``--ann``
(relay 36 of track D): each head's recorded annotations (``ANNOTS``) read into its white surface
(``MRISreadAnnotation``), written as an annotation file, a segmentation and GIFTI
(``MRISwriteAnnotation``), the annotation file compared with the recorded one byte for byte, each
read back. With ``--mpx`` (relay 37 of track D): each head's white surface with its sphere's
canonical coordinates: the principal-curvature and normal-dot distributions
(``mrisComputePrincipalCurvatureDistributions``, ``mrisComputeNormalDotDistribution``, 100 bins, a
100 x 100 volume), Ashburner's sampled energy at 64 vertices of the sphere
(``mrisSampleAshburnerTriangleEnergy``) and the white surface moved to Talairach space in place
(``MRIStalairachTransform``). With ``--lbl`` (relay 37 of track D): each head's recorded labels
(``LABELS``) on its white surface: ``LabelArea``, ``LabelMarkSurface``, ``LabelErode`` and
``LabelDilate`` by 2, ``MRISlabel2Mask``, ``MRISlabelInvert``. With ``--scl`` (relay 38 of
track D): the same labels' island and hole removal (``LabelRemoveIslandsSurf``,
``LabelRemoveHolesSurf``, ``LabelRemoveHolesAndIslandsSurf``), the clusters of ``|x| >= 20`` mm
(``sclustMapSurfClusters``) with their table, the clusterwise-p values, the old summary and a growth
by distance; and ``mris_place_surface --area-map`` of each white and pial surface
(``MRISwriteArea``), the file against the recorded ``?h.area`` and ``?h.area.pial``. With
``--mki`` (relay 39 of track D): ``mrisMarkIntersections`` without and with hole filling on each
head's ``orig.nofix``, ``orig``, ``white``, ``pial`` and ``inflated`` surfaces. With ``--vlr``
(relay 41 of track D): mris_ca_label's mode filter (``MRISmodeFilterAnnotations``, 10 passes, the
table set from the annotation's colour table first) on each head's recorded annotations, and
mri_binarize's ``--dilate-vertex`` disks (``MRISdilateVertexToSum`` with the areas, pi r^2 for r of
5, 20 and 50 mm) around vertices 0, 1000 and 50000 of each white surface. Prints the
counts;
the lines name machine paths, so no hash is recorded from them.

    python -P validation/probes/fs-surfio/real.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import struct
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

HEADS = ("ds000005-sub-01", "ds000228-sub-pixar008", "ds002422-sub-02")
# relay 35 of track D: the per-vertex files recon-all writes with MRISwriteCurvature (the new binary
# format, and the MGH overlay of mri_segstats' grey/white contrast)
CURVS = (
    "curv",
    "sulc",
    "thickness",
    "area",
    "curv.pial",
    "jacobian_white",
    "volume",
    "w-g.pct.mgh",
)
# relay 36 of track D: the per-vertex files programs read as values (MRISreadValues: the curvature
# format and the MGH overlay)
VALS = ("thickness", "curv", "sulc", "area", "w-g.pct.mgh")
# relay 36 of track D: the annotations recon-all writes (mris_ca_label) and reads
ANNOTS = ("aparc.annot", "aparc.a2009s.annot", "aparc.DKTatlas.annot", "BA_exvivo.annot")
# relay 37 of track D: the labels recon-all writes (mri_label2label's) and its cortex label
LABELS = ("cortex.label", "BA1_exvivo.label", "BA44_exvivo.thresh.label", "MT_exvivo.label")


def dx(v: float) -> str:
    return struct.pack(">d", v).hex()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument(
        "--curv",
        action="store_true",
        help="relay 35 of track D: the curvature files (flr crv): each head's recorded curvature "
        "files read into the white surface, written in five formats, read back",
    )
    ap.add_argument(
        "--pvw",
        action="store_true",
        help="relay 36 of track D: the value, patch and normal files (flr pvw): each head's "
        "recorded per-vertex files read as values, the patch and normals written, read back",
    )
    ap.add_argument(
        "--vps",
        action="store_true",
        help="relay 36 of track D: the vertex-position readers (flr vps) on each head's white "
        "surface: sphere, smoothwm, pial, white",
    )
    ap.add_argument(
        "--lbl",
        action="store_true",
        help="relay 37 of track D: label.cpp's surface-label functions (flr lbl) on each head's "
        "recorded labels and white surface",
    )
    ap.add_argument(
        "--scl",
        action="store_true",
        help="relay 38 of track D: surfcluster.cpp and the label clean-ups (flr scl) on each "
        "head's recorded labels and white surface",
    )
    ap.add_argument(
        "--vlr",
        action="store_true",
        help="relay 41 of track D: mris_ca_label's mode filter of each head's recorded annotations "
        "(flr maf) and mri_binarize's disks of 5, 20 and 50 mm around three vertices (flr dvs)",
    )
    ap.add_argument(
        "--sbl",
        action="store_true",
        help="relay 42 of track D: the soap bubbles (flr sbl) on each head's white and orig "
        "surfaces, the vertices of its cortex label free and the others fixed, 25 passes",
    )
    ap.add_argument(
        "--top",
        action="store_true",
        help="relay 42 of track D: mrisurf_topology.cpp's rest (flr top) on each head's "
        "orig.premesh, orig.nofix, orig and white surfaces with its cortex label: the Euler "
        "counts (the premesh ones against recon-all.log's mris_euler_number lines), the surface "
        "check, the removal of the vertices outside the label, the renumbering, the eversion",
    )
    ap.add_argument(
        "--mki",
        action="store_true",
        help="relay 39 of track D: mrisMarkIntersections (flr mki) on each head's orig.nofix, "
        "orig, white, pial and inflated surfaces, without and with hole filling",
    )
    ap.add_argument(
        "--mpx",
        action="store_true",
        help="relay 37 of track D: group (e) (flr mpx) on each head's white surface and sphere: "
        "the two distributions, Ashburner's sampled energy, the move to Talairach space",
    )
    ap.add_argument(
        "--ann",
        action="store_true",
        help="relay 36 of track D: the annotation files (flr ann): each head's recorded "
        "annotations read, written in three formats, read back",
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(a.jobs)
    lines = []
    for head in HEADS:
        sub = head.split("-", 1)[1]
        s = ws / "oracles" / "freesurfer-8.2.0" / "runs" / f"{head}-classic" / "subjects" / sub
        for hemi in ("lh", "rh"):
            if a.sbl:
                for surf in ("white", "orig"):
                    lines.append(
                        f"flr sbl {s / 'surf' / f'{hemi}.{surf}'} "
                        f"{s / 'label' / f'{hemi}.cortex.label'} 25"
                    )
                continue
            if a.top:
                for surf in ("orig.premesh", "orig.nofix", "orig", "white"):
                    lines.append(
                        f"flr top {s / 'surf' / f'{hemi}.{surf}'} "
                        f"{s / 'label' / f'{hemi}.cortex.label'}"
                    )
                continue
            if a.vlr:
                for c in ANNOTS:
                    lines.append(
                        f"flr maf {s / 'surf' / f'{hemi}.white'} {s / 'label' / f'{hemi}.{c}'}"
                    )
                for vno in (0, 1000, 50000):
                    for r in (5.0, 20.0, 50.0):
                        lines.append(f"flr dvs {s / 'surf' / f'{hemi}.white'} {vno} {dx(r)}")
                continue
            if a.mki:
                for surf in ("orig.nofix", "orig", "white", "pial", "inflated"):
                    for fill in (0, 1):
                        lines.append(f"flr mki {s / 'surf' / f'{hemi}.{surf}'} {fill}")
                continue
            if a.scl:
                for c in LABELS:
                    lines.append(
                        f"flr scl {s / 'surf' / f'{hemi}.white'} {s / 'label' / f'{hemi}.{c}'}"
                    )
                # mris_place_surface --area-map (MRISwriteArea) against the recorded files
                for surf, rec in (("white", "area"), ("pial", "area.pial")):
                    lines.append(
                        f"flr war {s / 'surf' / f'{hemi}.{surf}'} {s / 'surf' / f'{hemi}.{rec}'}"
                    )
                continue
            if a.lbl:
                for c in LABELS:
                    lines.append(
                        f"flr lbl {s / 'surf' / f'{hemi}.white'} {s / 'label' / f'{hemi}.{c}'}"
                    )
                continue
            if a.mpx:
                sd = s / "surf"
                lines.append(f"flr mpx {sd / f'{hemi}.white'} {sd / f'{hemi}.sphere'}")
                continue
            if a.ann:
                for c in ANNOTS:
                    lines.append(
                        f"flr ann {s / 'surf' / f'{hemi}.white'} {s / 'label' / f'{hemi}.{c}'}"
                    )
                continue
            if a.vps:
                sd = s / "surf"
                lines.append(
                    f"flr vps {sd / f'{hemi}.white'} {sd / f'{hemi}.sphere'} "
                    f"{sd / f'{hemi}.smoothwm'} {sd / f'{hemi}.pial'}"
                )
                continue
            if a.pvw:
                for c in VALS:
                    lines.append(
                        f"flr pvw {s / 'surf' / f'{hemi}.white'} {s / 'surf' / f'{hemi}.{c}'}"
                    )
                continue
            if a.curv:
                for c in CURVS:
                    lines.append(
                        f"flr crv {s / 'surf' / f'{hemi}.white'} {s / 'surf' / f'{hemi}.{c}'}"
                    )
                continue
            for surf in ("white", "pial"):
                lines.append(f"flr fil {s / 'surf' / f'{hemi}.{surf}'} {s / 'mri' / 'aseg.mgz'}")
            lines.append(
                f"flr rib {s / 'surf' / f'{hemi}.white'} {s / 'surf' / f'{hemi}.pial'} "
                f"{s / 'mri' / 'orig.mgz'}"
            )
            lines.append(f"flr bbf {s / 'surf' / f'{hemi}.white'} {dx(0.5)}")
    inputs = ws / "oracles" / "inputs" / "fs-surfio"
    inputs.mkdir(parents=True, exist_ok=True)
    kind = (
        "sbl"
        if a.sbl
        else "top"
        if a.top
        else "vlr"
        if a.vlr
        else "mki"
        if a.mki
        else "scl"
        if a.scl
        else "lbl"
        if a.lbl
        else "mpx"
        if a.mpx
        else "ann"
        if a.ann
        else "vps"
        if a.vps
        else "pvw"
        if a.pvw
        else "curv"
        if a.curv
        else "flood"
    )
    cases_path = inputs / f"real-{kind}-cases.txt"
    cases_path.write_text("".join(x + "\n" for x in lines), encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    env.update(freesurfer_env())
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=36000,
        runner="fs-surfio-probe-real",
    )
    t_official = time.monotonic() - t0
    if done.returncode != 0:
        raise SystemExit(f"fssurfio failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()
    rows = {}
    for mode in ("reproduce", "fix"):
        t0 = time.monotonic()
        text = subprocess.run(
            [str(port), "run", str(cases_path), mode], check=True, capture_output=True, text=True
        ).stdout
        rows[mode] = ([x.partition("\t") for x in text.splitlines()], time.monotonic() - t0)
    same = differ = fix_same = 0
    bad = []
    notices: dict[str, int] = {}
    fs901_crashes = 0
    for case, c, (r, _, rids), (f, _, ids) in zip(
        lines, official, rows["reproduce"][0], rows["fix"][0], strict=True
    ):
        for x in filter(None, ids.split(",")):
            notices[x] = notices.get(x, 0) + 1
        if c == r:
            same += 1
        elif c.startswith("CRASH 11 ") and "FS-901" in rids.split(","):
            # relay 39 of track D: a cluster growth deeper than FreeSurfer's stack (FS-901): the
            # official build crashes, the port grows it in full in both modes (never reproduced)
            fs901_crashes += 1
        else:
            differ += 1
            names = " ".join(Path(x).name for x in case.split()[2:])
            bad.append({"case": f"{case.split()[1]} {names}", "o": c[:2000], "p": r[:2000]})
        if f == c:
            fix_same += 1
    # relay 42 of track D: the port's Euler counts of each orig.premesh against recon-all.log's
    # mris_euler_number line (`euler # = v-e+f = 2g-2: <v> - <e> + <f> = <euler> --> ...`)
    euler_log = {"checked": 0, "same": 0}
    if a.top:
        for case, (r, _, _) in zip(lines, rows["reproduce"][0], strict=True):
            surf = Path(case.split()[2])
            if not surf.name.endswith(".orig.premesh"):
                continue
            log = (surf.parent.parent / "scripts" / "recon-all.log").read_text(errors="replace")
            want = None
            take = False
            for ln in log.splitlines():
                if ln.strip() == f"mris_euler_number ../surf/{surf.name}":
                    take = True
                elif take and ln.startswith("euler # = v-e+f = 2g-2: "):
                    nums = ln.split(":", 1)[1].split("-->")[0].replace("=", " ").split()
                    v, e, f, eu = int(nums[0]), int(nums[2]), int(nums[4]), int(nums[5])
                    want = (eu, v, f, e)
                    break
            got = tuple(int(x) for x in r.split()[1:5])
            euler_log["checked"] += 1
            euler_log["same"] += int(want == got)
    summary = {
        "cases": len(lines),
        "euler_vs_recon_all_log": euler_log,
        "served": done.cached,
        "reproduce_identical": same,
        "reproduce_differ": differ,
        "official_crash_fs901": fs901_crashes,
        "fix_identical": fix_same,
        "fix_notices": notices,
        "seconds_official": round(t_official, 1),
        "seconds_port_reproduce": round(rows["reproduce"][1], 1),
        "seconds_port_fix": round(rows["fix"][1], 1),
        "differ": bad,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
