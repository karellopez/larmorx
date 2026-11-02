# FLIRT's other programs: probe calls

`probe.py` runs FSL 6.0.7.23's program and the replica (`larmorx-nc fsl <program>`) on lists of
argument lines and compares their exit status, standard output and error and the files they
write. The lists behind part 2's statements (2026-11-01; `docs/findings/fsl-flirt.md`):

| File | Program | Calls | Result (reproduce) |
|---|---|---|---|
| `cases_avscale.py` | `avscale` | 17 | 16 identical; a 5 x 5 matrix refused (FSL computes part of it first) |
| `cases_avscale_rand.py` | `avscale` | 400 random matrices (rotations, scales, skews, reflections, any) | 400 identical |
| `cases_img2img.py` | `img2imgcoord` | 21 | 21 identical |
| `cases_std.py` | `std2imgcoord`, `img2stdcoord` | 21 each | identical |
| `cases_rmsdiff.py` | `rmsdiff` | 40 | 40 identical |
| `cases_makerot.py` | `makerot` | 50 | 50 identical (part 3: the 3 with a NaN or infinite angle or axis too, since `make_rot`'s operand order) |

```bash
LARMORX_NC_BIN=<larmorx-nc> python -P probe.py <work dir> cases_makerot.py --rep --quiet
MODE=fix LARMORX_NC_BIN=<larmorx-nc> python -P probe.py <work dir> cases_makerot.py --rep
```

Part 3's probes (each keeps FSL's outputs in its work directory and reuses them; `--fresh` runs
FSL again):

| File | What | Result |
|---|---|---|
| `makerot_nan.py` | `makerot` with NaN and infinite angles, axes and centres: the signs of its NaNs | 10 of 10 identical |
| `aff2rigid_mat.py` | FSL's `aff2rigid` run in FSL's Python with `mat` restored (FSL-81), against the replica's default | 22 of 22 matrices identical |
| `extracttxt_ranges.py` | `extracttxt` on every combination of 5 words, 8 counts and 11 starts (signed, negative, zero): dc's and sed's messages | 390 of 390 identical |

```bash
LARMORX_NC_BIN=<larmorx-nc> python -P makerot_nan.py <work dir>
LARMORX_NC_BIN=<larmorx-nc> python -P aff2rigid_mat.py <work dir>
LARMORX_NC_BIN=<larmorx-nc> python -P extracttxt_ranges.py <work dir>
```

The inputs are the parity suite's (`<workspace>/oracles/inputs/fsl-flirt-replica`, made by
`python -m larmorx_validation parity fsl-flirt-replica`; `LARMORX_FLIRT_INPUTS` elsewhere) and FSL's
standard images.
