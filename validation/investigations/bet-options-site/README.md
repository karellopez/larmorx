# The site's BET and fslmaths pages after BET's options part (2026-11-04)

What the documentation site's `fsl-bet` and `fsl-maths` pages were checked with when BET's
`-S`, `-B`, `-A`, `-A2` and standard_space_roi got their worked examples
(`larmorx_validation.evidence.fsl_bet_options`) and fslmaths' replica evidence was exported again
from its record of 422 cases (`fsl_maths.refresh_replica`: the replica's entries of the figures
cache made again, FSL's runs served by the oracle-output cache, the previous entries kept under
`<cache>/fsl-maths/.superseded/`).

- `inventory-2026-11-04.json`: the two pages' counts before (site `main` 51d5465) and after
  (branch `bet-options-pages`, exported from larmorx 65543af7), from
  `../cleanroom-glibc-dbl/site_inventory.py` (every built page and evidence file of the site
  compared: `inventory-compare-2026-11-04.txt`), and the number and viewer checks
  (`../panel-ants/site_shots.py`: `LX_EVIDENCE.check()` and `checkViewers()`) before and after:
  BET 4 examples, 100 viewers, 247 numbers before; 11 examples, 180 viewers, 416 numbers after,
  every one agreeing; fslmaths 3 examples, 152 viewers, 451 numbers before, 450 after.
- The one count that fell: fslmaths' page shows one number fewer, the replica agreement chart's
  "expected divergence: 1" of the random-testing category, as that case
  (`difftest/fft-twiddles-glibc-sincos`) passes since the replica's reproduce takes glibc's
  `sincos` (relay 1 of the part): a count of zero is not drawn. Nothing shown before is gone.
- `shoot_example.sh`: one example photographed after its viewer has drawn (1440 and 390 pixels,
  light and dark were looked at for the new examples).

Run the checks again: serve the site (`tools/build.py --serve --port 8772 --main-repo
<larmorx>`), then `python -P ../panel-ants/site_shots.py <site> <url> <out> tools/fsl-bet/
tools/fsl-maths/` and `python -P ../cleanroom-glibc-dbl/site_inventory.py <site> --out after.json`.
