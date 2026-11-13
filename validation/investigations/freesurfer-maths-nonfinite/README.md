# fs.maths' matrices with infinite or NaN entries, and its files cut short

The measurements behind the **(observed)** marks of [`specs/freesurfer-maths.md`](../../../specs/freesurfer-maths.md)
§8.8 (the VNL ports' NaNs), §17.1, §17.2 and §17.4 (FS-47, ITK-112, FS-48: which matrices with
infinite or NaN entries make FreeSurfer hang and what it gives on the others) and §18.2 (FS-49: the
bytes of a binary matrix file cut short), track E's relay 4 (2026-11-13). **Open to the clean-room
implementer of that spec**: nothing here comes from FreeSurfer's source; it runs the prebuilt probe
(a black box, [`../freesurfer-maths-probe/`](../freesurfer-maths-probe/)) and the main package's open
VNL ports, and states §18.2's rule as the spec does.

| File | What it is |
|---|---|
| `Cargo.toml`, `src/main.rs` | `fs-maths-nonfinite`: the main package's ports (`larmorx_core::vnl_eigen`'s symmetric and real eigensystems, `vnl_svd_f32_full`, `vnl_qr::Qr<f32>`, `vnl_fixed`'s determinants and inverses) on matrices given as text, one line out per line in (its doc comment lists the commands); its own workspace, never part of larmorx's build |
| `check.py` | `saved`: the probe's runs saved by `compare.py refs --save` (the batches `eigen-nonfinite`, `svd-nonfinite`, `minverse`, `mfiles`, `mfiles-cut`) against the ports and §18.2's rule; `explore`: the spec's explorations (400 symmetric matrices for `oeigen` and `meigen`, 400 for `oeigenns`, 400 for `msvd`, `mcond` and `msvdpinverse`, seeded) run once on the probe through the oracle-output cache, each in its own working directory, and compared |

## Running it

```sh
source scripts/orchestration/env.sh
CARGO_TARGET_DIR=<scratch>/nf cargo build --release \
  --manifest-path validation/investigations/freesurfer-maths-nonfinite/Cargo.toml
# the saved runs of the probe's groups (after compare.py refs ... --groups eigen,svd,mfiles,minverse --save <scratch>/save)
python -P validation/investigations/freesurfer-maths-nonfinite/check.py saved --save <scratch>/save \
  --batches <the refs scratch>/batch --ports <scratch>/nf/release/fs-maths-nonfinite
# the explorations (about 10 minutes cold, three batches at once; seconds from the cache)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-maths-nonfinite/check.py explore \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe --scratch <scratch>/explore \
  --ports <scratch>/nf/release/fs-maths-nonfinite
```

Each prints its counts per command and kind; a line that disagrees is printed in full and the exit
status is 1 (also where FreeSurfer never returned and the ports say VNL returns). The probe's
working directory holds its capture files: never run two probes in one directory (four at once in
one directory mixed their standard errors in a first trial).

## Results (2026-11-13, the reference machine)

- `saved`, the batches of the spec's §23 (seed 7): `oeigen` 42 never returned, each where the port
  says VNL never returns, 69 finished, each identical to the port bit for bit; the eigensystem
  helper's symmetric path and `meigen` likewise (44 and 81); `msvd` 12 never returned (the port's),
  51 LINPACK failures (each with the port's `info`), 23 decompositions (9 identical, 14 differing
  only in U's NaNs' bits, W and V identical); `minverse`: 20 inverses into a given matrix, 20
  determinants, 22 QR factorizations with infinite or NaN inputs, each with the ports' bits;
  `mreadbin` (the groups `mfiles` and `mfiles-cut`): every number whose bytes come from the file or
  earlier numbers is §18.2's (128), every memory line takes a byte from memory by §18.2 (28; 12 more
  such lines happen to be stable), 46 files make no matrix.
- `explore`: see `oracles/logs/fs-maths-nonfinite-explore-2026-11-13.log` (the spec quotes it).
