# freesurfer-surfio: FreeSurfer 8.2.0's generated surface views, tabled

**Closed to FreeSurfer clean-room implementers** (it reads FreeSurfer's source and prints its
functions).

`accessors.py <freesurfer-tag> summary|json <out>|rust <views.rs>` reads the bodies of
`include/mrisurf_SurfaceFrom{MRIS,MRIS_MP,MRISPV}_generated_suffix.h` (the `Face`, `Vertex` and
`Surface` views of each representation at its 13 access levels) and classifies each as a read, a
write or a write with `cheapAssert`:

- `summary`: the counts per level and class, every member function whose body differs between
  levels, every body that is neither a read nor a write, and each level's `which_coords` cases.
- `json <out>`: the whole table.
- `rust <views.rs>`: the replica's `MetricSurface` trait (`crates-freesurfer/larmorx-freesurfer-surf`)
  against the `MRIS_MP` views' `AllM` accessors (the ones left out on purpose listed in `EXCLUDED`).

Measured 2026-11-13 on the `v8.2.0` tag: 2,778 bodies for `MRIS`, 817 for `MRIS_MP`, 2,736 for
`MRISPV`; 406, 120 and 400 distinct member functions, none whose body differs between levels, none
but `which_coords` that is not a plain read or write; the replica's trait has all 114 `MRIS_MP`
accessors it should. The findings: `docs/findings/freesurfer-surfio.md` §1-3.

    python -P validation/investigations/freesurfer-surfio/accessors.py <workspace>/tags/freesurfer-8.2.0 summary
