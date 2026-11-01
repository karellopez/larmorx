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
| `cases_makerot.py` | `makerot` | 50 | 47 identical; 3 with a NaN or infinite angle or axis print `-nan` where FSL prints `nan` (part 3) |

```bash
LARMORX_NC_BIN=<larmorx-nc> python -P probe.py <work dir> cases_makerot.py --rep --quiet
MODE=fix LARMORX_NC_BIN=<larmorx-nc> python -P probe.py <work dir> cases_makerot.py --rep
```

The inputs are the parity suite's (`<workspace>/oracles/inputs/fsl-flirt-replica`, made by
`python -m larmorx_validation parity fsl-flirt-replica`; `LARMORX_FLIRT_INPUTS` elsewhere) and FSL's
standard images.
