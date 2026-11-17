# Provenance: larmorx-freesurfer-deform

**Replica: translated from FreeSurfer's source.** This crate holds `fs.deform` (wave 3.4) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): surface deformation: the time steps, the integration, the gradients and the SSE terms. It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

| Rust module | FreeSurfer file | Functions and data |
|---|---|---|
| `project` | `utils/mrisurf_deform.cpp` 142-386 (`mris_project_point_into_face`, `mrisProjectOntoSurface`, `mrisSurfaceProjector<MRIS>` and `<MRIS_MP>` with `canDo` and `project`, `mrisProjectSurface` for `MRIS` and `MRIS_MP`, `mrisProjectSurface_CanDo`), 2172-2335 (`MRISflattenPatch`); `utils/mrisurf_deform.h` 48-61 (`transform`) (queue item 3.4-06, taken early, relay 44 of track D) | the projection of a surface back onto its kind, the flattening of a patch (the official build's `float` arithmetic and operand orders, `docs/findings/freesurfer-surfio.md` §43; FS-1001 to FS-1005) |
| `intersect` | `utils/mrisurf_deform.cpp` 4009-4083 (`MAX_INT_REMOVAL_NEIGHBORS`, `MRISremoveIntersections`) (queue item 3.4-06, taken early, relay 44 of track D) | the removal of self-intersections by soap-bubble smoothing (fs.surf's `mrisMarkIntersections`, `MRISsoapBubbleVertexPositions`) |

No third-party code in these lines (CLAUDE.md rule 3: FreeSurfer's own, Bruce Fischl's and Bevin
Brett's).

## Changes from the originals

- C's pointers are Rust values, as in `larmorx-freesurfer-surf` (its PROVENANCE): the surface's face
  table `mris->mht` is fs.surf's `Opaque` holding an `Mht`; a read through a null pointer that
  FreeSurfer would make is a noted defect (an error by default, status 139 with `reproduce`).
- The `MRIS_MP` projection and `canDo`'s `static int shown` are a function returning FreeSurfer's
  failed assertion and a mask the caller keeps.
