# FLIRT's clean-room original: development probes (part 1, 2026-11-02)

The tools behind [`docs/findings/fsl-flirt-original.md`](../../../docs/findings/fsl-flirt-original.md)
and the original's record ([`docs/validation/fsl-flirt.md`](../../../docs/validation/fsl-flirt.md)).
They run FSL 6.0.7.23 as a black box (through the oracle cache, under the `fsl-flirt-replica`
suite's keys) and the clean-room original (`LARMORX_FLIRT_BIN`, else `python -m larmorx`); each
inside `scripts/orchestration/boxed.sh`, with `PYTHONPATH=python:validation/src`, run with
`python -P`.

| File | What it shows | How to run |
|---|---|---|
| `probe.py` | one suite case, any command line or a random-testing finding with FSL and the original into two kept directories: the streams, the matrices (displacement over the reference's brain), the images' headers and differing values | `probe.py <dir> --case real/dof6`, `probe.py <dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat"`, `probe.py <dir> --finding <finding.json> [--whole]` |
| `costs.py`, `costs.sch`, `costs2.sch`, `costs3.sch` | the costs of two matrices at each pair (`costs3.sch`: the 4 mm pair and the two scales that only reset the input), every cost function, FSL against the original (the findings' "The costs") | `costs.py [--schedule costs3.sch] [-- -noresample]` |
| `corratio_synth.py`, `corratio_sparse.py` | `corratio` and `woods` on synthetic images: which bin counts agree (`corratio_synth.py`), and a reference bin holding 1 to 12 points on a 24³ or 10³ image (`corratio_sparse.py <dir> [small]`): the sparse bins' rule of the findings | `corratio_synth.py <dir>`, `corratio_sparse.py <dir> small` |
| `centre.sch` | the image pair's centre after each `setscale`: two rotations' parameters (`printparams`), whose translations give the centre (the findings' "The image pair's centre") | `probe.py <dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat -schedule <path>/centre.sch"` |
| `costs4.sch` | the weighted costs of scaled matrices at 8 mm (the scales of the search's first line search), with `-refweight` and `-inweight` | `probe.py <dir> --args "... -refweight @@w.nii -schedule <path>/costs4.sch"` |
| `bbrcost.sch` | the boundary-based cost of the initial matrix with `bbrstep` 1, 2, 200 and 201 (the findings' "The boundary-based cost") | `probe.py <dir> --args "-in @@epi -ref @@t1 -init @@m -wmseg @@wm -cost bbr -omat o.mat -schedule <path>/bbrcost.sch"` |
| `stages.py` | the default schedule with `print` after the search and every `measurecost` and `optimise`, FSL against the original: where a registration's path leaves FSL's (the findings' "The search and the optimisation") | `stages.py <dir> [flirt args, default -dof 6]` |
| `sinc_kernel.py` | which construction of the windowed sinc reproduces FSL's `-applyxfm -interp sinc` values (the findings' "The pyramid and the outputs") | `sinc_kernel.py <probe dir of real/interp-sinc> [voxels]` |
| `programs_fsl.py`, `programs_cases.txt` | part 2 (relay 1): FSL 6.0.7.23's `avscale`, coordinate programs, `rmsdiff` and `makerot` on command lines beyond the suite's (`makerot -o`'s last bits, small angles, NaN axes; `avscale`'s precisions and failures; the coordinate programs' default space), through the oracle cache (the findings' "FLIRT's other programs") | `programs_fsl.py programs_cases.txt` |
| `programs_cmp.py` | the same command lines with FSL and the original (`reproduce`), the differences of exit status, streams and files | `programs_cmp.py programs_cases.txt [fix]` |
| `programs_stdin.sh` | the coordinate programs reading standard input, FSL run directly (the oracle cache does not key standard input) against the original | `programs_stdin.sh <dir with epi.nii, t1.nii> <larmorx binary>` |
| `programs2_fsl.py`, `programs2_cases.txt` | part 2 (relay 2): FSL 6.0.7.23's `midtrans`, `pointflirt`, `applyxfm4D`, the scripts and `convert_xfm` on odd matrix files, beyond the suite's cases (an `env:NAME=value` word sets a variable, `@FSL` FSL's directory: `aff2rigid` needs FSL's `bin` on the `PATH`), through the oracle cache (the findings' "FLIRT's other programs and scripts") | `programs2_fsl.py programs2_cases.txt`; `programs_cmp.py programs2_cases.txt` for FSL against the original |
| `programs2_images.py`, `programs2_images.txt` | the images two such command lines write, FSL's against the original's: the header fields that differ, the values | `programs2_images.py programs2_images.txt` |
| `programs2_integer.py` | `applyxfm4D` on generated int16, uint16 and uint8 series (spline with FSL-79, sinc, trilinear, `-v`'s method) and the blur question (a 1.7 mm T1w onto a 3 mm grid): FSL against the original in both modes | `programs2_integer.py` |
| `speed.py` | the informal speed table of the API page: FSL, the replica (`LARMORX_NC_BIN`) and the original at 1 and 8 threads, one run each | `speed.py` |
| `verbose_probe.py`, `verbose_cases.txt` | part 2 (relay 3): FSL 6.0.7.23's `flirt` at the verbose levels 2 to 7 and with `-debug`/`-debugsave` on short schedules and options beyond the suite's (`-noclamp`, `-noresample`, `-2D`, `-applyxfm`, `-out`, forced and input-only scales, the boundary-based cost set up by `setoption`, short tolerance lists), through the oracle cache, against the original with the suite's checks (the findings' "The verbose levels and -debug"; most became the suite's own cases) | `verbose_probe.py verbose_cases.txt [--streams]` |
| `same_bits.py` | part 2 (relay 3): every `flirt` case of the suite run by two builds of the original and at several thread counts, results compared byte for byte (the speed changes left every result as it was) | `same_bits.py --a <before> --b <after> --threads 1,7` |
| `profile.sh`, `profile.gdb` | part 2 (relay 3): where the original's time goes, by gdb backtraces every 20 ms (the machine does not allow `perf`) | `profile.sh <out> <larmorx> fsl flirt <args>` |
| `bench.py` | part 2 (relay 3): the benchmark report [`docs/benchmarks/fsl-flirt.md`](../../../docs/benchmarks/fsl-flirt.md): FSL, the replica and the original at 1 and 14 threads, one run each, on the standard tier's real cases | `bench.py --larmorx <larmorx> --replica <larmorx-nc> --threads 1,14 --json <report.json>` |

FSL's `flirt -verbose 7` (every cost evaluation with its parameters) was the reference for the
evaluation-by-evaluation comparison of the findings; the original printed its own evaluations
through a development hook that is no longer in the code.
| `trial5_tcat.py <trial dir> [kind] [signature] [n]` | part 2, relay 5: a random-testing trial's differing cases by (kind, first failed checks), with examples |
| `trial5_stderrcat.py`, `trial5_codestat.py`, `trial5_stdcodes.py`, `trial5_a4d.py` (`<trial dir>`) | the trial's standard-error differences by their lines; form-code differences by the reference's unset form; the coordinate programs' by the images' codes; applyxfm4D's by interpolation and stored type |
| `trial5_dcmp.py <seed> <n> <dir>` | one of the trial's cases by hand (FSL with the cache off, the original in `reproduce`), both directories and streams kept, header fields and voxels that differ |
| `trial5_isomodel.py <dcmp dirs>` | models of `-applyisoxfm`'s written sform against FSL's (the voxel flip, §4.1's swaps by both grids) |
| `trial5_bbre.py <dir>` | BBR with an empty `-wmseg` on five small schedules: FSL's and the original's error and NaN lines |
| `relay6_dloop.py <seed> <dir> <n>...` | part 2, relay 6: the trial's cases by hand (FSL through the oracle cache, `use`; the original in `reproduce`): one line per case (exit statuses, streams, every file: images by header and values, others by bytes) |
| `relay6_u8probe.py <dir> [names]` | applyxfm4D and `flirt -applyxfm` on uint8 and int16 profiles (nearest, trilinear, sinc, spline): FSL-111's signed char and FSL-79's x86 conversions |
| `relay6_fprobe.py <dir> [names]`, `relay6_fprobe4d.py <dir>` | a reference with neither form: the output forms of applyxfm4D, `-applyxfm` and a registration for five inputs and two matrices; applyxfm4D's per-volume matrices (the first volume's decides) |
| `relay6_mprobe.py <seed> <n> <dir> [words]`, `relay6_mseq.py <merged text>` | a trial case with standard output and error merged through `sh` (FSL cached as its own entry): where `Erroneous dof` is printed, command by command |
