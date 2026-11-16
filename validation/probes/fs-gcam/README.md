# fs-gcam: FreeSurfer 8.2.0's GCA morph helpers

**Closed to FreeSurfer clean-room implementers** (it reads FreeSurfer's source).

- `cma_names.py` (track A relay 38): generates `crates-freesurfer/larmorx-freesurfer-gca/src/cma_names.rs`, the
  table behind fs.gca's `cma_label_to_name`, from FreeSurfer 8.2.0's `utils/gca.cpp` (its chain of `if (label ==
  <name>) return ("<text>");` tests, in their order: the first equal label wins). The names are `include/cma.h`'s
  label constants as fs.volume's `cma.rs` transcribes them. It checks that every test of the function was matched.

      python3 -P validation/probes/fs-gcam/cma_names.py <workspace>/tags/freesurfer-8.2.0 \
          crates-freesurfer/larmorx-freesurfer-gca/src/cma_names.rs && (cd crates-freesurfer && cargo fmt --all)

The morph reader itself is compared by the fs-io probe's piece 31 (`validation/probes/fs-io/README.md`) and by
fs.gca's unit tests (`crates-freesurfer/larmorx-freesurfer-gca/tests/gcam_read.rs`).
