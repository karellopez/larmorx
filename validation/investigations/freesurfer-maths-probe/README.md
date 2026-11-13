# FreeSurfer 8.2.0's mathematics, as a black box

The probe behind the **(observed)** marks and the validation plan of
[`specs/freesurfer-maths.md`](../../../specs/freesurfer-maths.md) (`fs.maths`: the float and double
matrices, the 4 x 4 affine helpers, the histograms, the element-wise operations, regions, vectors,
rotations, printing, Gaussian matrices, inverses, determinants, QR factors, primes, Gaussian draws,
the minimizers, Numerical Recipes' arrays, the eigensystems and the SVD, the matrix files, the
random-field specifications). **Open to the clean-room implementer of that spec**: the files here
hold only calls of FreeSurfer's functions (and of the C library's) by name, the reading and
printing of their arguments and results, the probe's own test costs, a guard allocator and a
comparison loop; no FreeSurfer code, nothing written from its source. The implementer runs the
prebuilt probe (below) and the comparison loop; building the probe reads the unit oracle's build
tree, which is closed to FreeSurfer's clean-room implementers, so they never build it.

| File | What it is |
|---|---|
| `fs_maths_probe.cpp` | the probe: a batch of calls from a file, each in a child process of its own; its commands below |
| `guard_malloc.c` | the guard allocator of the guard runs (`LD_PRELOAD`): in each child, every allocation on pages of its own, with an inaccessible page right after its last byte (`MALLOC_GUARD_` unset or `over`) or right before its first (`MALLOC_GUARD_=under`), its other bytes a fill byte (`MALLOC_PERTURB_`'s value); no other program's code |
| `build.sh` | builds both with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>`, the allocator as `guard_malloc.so` beside the output (the spec writer's or the orchestrator's step, not the implementer's) |
| `compare.py` | the comparison loop of the spec's §23: 24 groups of seeded cases, run on the probe (four times: plainly, plainly again in another process and in two guard runs, through the oracle-output cache) and on a candidate program that takes the same commands; `list`, `refs`, `compare` |
| `standin.py` | a stand-in candidate that checks the loop's own rules: the probe's saved lines (`refs --save`) with the refusals and NaN signs the spec allows, or (`--break`) without them |

The prebuilt probe and allocator: `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe`
and `guard_malloc.so` beside it. Their build (the larmorx commit, this folder's `build.sh`, the
SHA-256s) is recorded in that folder's `BUILD_INFO.txt` for the orchestrator's check; the
implementer only runs them (every other access to the unit oracle's folder, reading or hashing a
file in it included, counts as opening FreeSurfer's build tree).

## Running it

```sh
fs_maths_probe batch <file>      # or - for standard input
```

Each line of the file is one call: `[@<seconds>] [+gdiag=<hex>] <command> <arguments>`. The probe
reads every line first, then runs each in a child process of its own, in the working directory,
with standard output and standard error captured and an alarm of 2 seconds (`@<seconds>` sets
another: the loop gives the minimizers 60 and the decompositions of non-finite matrices 1).
`+gdiag=<hex>` sets FreeSurfer's diagnostic bits before the call (`10000008`: write and verbose).
One line is printed per call (an empty line for an empty one):

```
<result> | <standard output> <standard error>
```

the two streams as strings (below). `<result>` is the command's printed values, or `EXIT <status>`
when FreeSurfer's code ended the program (its error functions; the streams hold its messages),
`CRASH <signal>` when it died of a signal, `TIMEOUT` when the alarm ended it (FreeSurfer never
returned). VNL's messages name their source file: the probe writes the official build's ITK folder,
`/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK`, before `/Modules/ThirdParty/VNL/` on every such
line. With `MALLOC_PERTURB_` set, the child fills 256 KiB of its stack with that byte before the
call.

## Encodings

| Kind | Form |
|---|---|
| integer | decimal |
| float | the 8 hex digits of its IEEE 754 bits (`3f800000` is 1.0) |
| double | the 16 hex digits of its bits (`3ff0000000000000` is 1.0) |
| string | `x` followed by the hex digits of its bytes (`x` alone is the empty string) |
| histogram (argument) | `<alloc> <nbins> <bin_size> <min> <max> <nbins bin values> <nbins counts>` (floats but the first two): allocated with `alloc` bins, then `nbins` set; `null` is none |
| histogram (printed) | `<nbins> <max_bins> <bin_size> <min> <max> <bin values> <counts>`; `null` |
| float matrix (argument) | `<type> <rows> <cols> <numbers>`: rows × cols floats row by row (two per element, real and imaginary, for type 2); `null` |
| float matrix (printed) | `<type> <rows> <cols>` and every float of its array from float 0 (the two unused floats first, spec §2.1) to the last number; `null` |
| double matrix | `<rows> <cols> <rows × cols doubles>` (argument and printed); `null` |
| output argument | `new` (none given), `self` (the first input object), `other` (the second), or an object; the printed result is the returned object (`given` when it is the given output itself), then the given output's state |
| stream | `out`, `err`, `null` |

## Commands

The spec's sections in brackets; `O` is an output argument; `H`, `M`, `D` a histogram, float
matrix, double matrix; `f`, `d`, `i`, `x`, `s` a float, double, integer, string, stream.

| Command | Calls | Prints |
|---|---|---|
| `halloc i` | `HISTOalloc` [§5.1] | H |
| `hrealloc H i` | `HISTOrealloc` | result, H |
| `hfree H` | `HISTOfree` | its value, `none` or `set` (the pointer after) |
| `hcopy`, `hclear`, `hclearcounts`, `hpdf`, `habs`, `hcdf`, `hrcdf`, `hcumsum` `H O` | `HISTOcopy`, `HISTOclear`, `HISTOclearCounts`, `HISTOmakePDF`, `HISTOabs`, `HISTOmakeCDF`, `HISTOmakeReverseCDF`, `HISTOcumsum` [§5.2, §5.3] | result, O, then H (unless O is H) |
| `hinit O i d d` | `HISTOinit` (nbins, lo, hi) | result, O |
| `hbins i d d` | `HISTObins` | H |
| `hadd`, `hsub` `H H O` | `HISTOadd`, `HISTOsubtract` | result, O, then each input that is not O |
| `hscale H O f f` | `HISTOlinearScale` (scale, offset) | result, O |
| `hclearbins H O i i` | `HISTOclearBins` | result, O |
| `hsoap H O i` | `HISTOsoapBubbleZeros` | result, O |
| `hsmooth H O f` | `HISTOsmooth` [§5.4] | result, O |
| `hsumnorm`, `hfill`, `hclearzero` `H` | `HISTOsumNorm`, `HISTOfillHoles`, `HISTOclearZeroBin` | its value, H |
| `hlastpeak`, `hfirstpeak` `H i f` | `HISTOfindLastPeak`, `HISTOfindFirstPeak` (window, fraction) [§5.5] | i |
| `hvalley H i i i` | `HISTOfindValley` (window, I0, I1) | i |
| `hlastpeakin H i f i i` | `HISTOfindLastPeakInRegion` | i |
| `hhighest H i i` | `HISTOfindHighestPeakInRegion` | i |
| `hprevvalley`, `hnextvalley` `H i` | `HISTOfindPreviousValley`, `HISTOfindNextValley` | i |
| `hnextpeak`, `hprevpeak` `H i i` | `HISTOfindNextPeak`, `HISTOfindPreviousPeak` (b0, half window) | i |
| `hstartpeak`, `hendpeak` `H i f` | `HISTOfindStartOfPeak`, `HISTOfindEndOfPeak` | i |
| `hcurrentpeak H i i f` | `HISTOfindCurrentPeak` (b0, window, fraction) | i |
| `hfindbin H f` | `HISTOfindBin`, `HISTOgetCount`, `HISTOfindBinWithCount` | i d i |
| `hvaltobin H d` | `HISTOvalToBin` | i |
| `hcount H n d...` | `HISTOcount` of n samples | its value, H |
| `haddsample H n f... f f` | `HISTOaddSample` of each of n values (lo, hi) [§5.6] | each bin number, H |
| `htotal H i i` | `HISTOtotalInRegion`, `HISTOtotal` | f f |
| `hthresh H H f` | `HISTOthreshSum` (mask, source, threshold) | f |
| `hmedian`, `hentropy` `H` | `HISTOfindMedian`, `HISTOgetEntropy` | d |
| `hcorrelate H H` | `HISTOcorrelate` | d |
| `hlinfit H H d d d d` | `HISTOfindLinearFit` (a0, a1, b0, b1) | d f f (the best, a, b; -7 when not set) |
| `hfwhm H i` | `HISTOcomputeFWHM` | f |
| `hrobust H d` | `HISTOrobustGaussianFit` [§5.7] | status, offset, σ (doubles; -7 when not set), H, then the files `ha.plt`, `hc.plt`, `h.plt`, `hs.plt`, `hz.plt`, `hp.plt` (`nofile` when absent) |
| `hplot`, `hwrite` `H x` | `HISTOplot`, `HISTOwriteTxt` to the file named x [§5.8] | its value, the file's bytes as a string (`nofile`) |
| `h2d i i f f f f <bins1> <bins2> <counts> f f` | `HISTO2DfindBin1`, `HISTO2DfindBin2`, `HISTO2DgetCount` of a two-dimensional histogram (its bin counts, bin sizes, minima, bin values, counts row by row; v1, v2) | i i d |
| `malloc i i i` | `MatrixAlloc` (rows, cols, type) [§2.2]; `FREESURFER_MatrixAlloc_old` in the environment chooses the old allocator | M |
| `malloc2 i i i` | `MatrixAlloc2` with a small buffer, then `MatrixFree` | M, in the buffer (1 or 0), the free's value, `none`/`set` |
| `mfree M` | `MatrixFree` | its value, `none`/`set` |
| `mcopy M O` | `MatrixCopy` [§2.4] | result, O |
| `mmuld`, `mmul` `M M O` | `MatrixMultiplyD`, `MatrixMultiply` [§2.5, §8.1, §16] | result, O, then each input that is not O |
| `affine M M f f f f` | `SetAffineMatrix` of both, `SetAffineVector` (x, y, z) and its 4 floats, its fourth float set to the last f, `AffineMV`, `GetAffineVector`, `GetFloorAffineVector`, `AffineMM`, `GetAffineMatrix` [§3] | the vector, the product vector, its three floats, the floor's three integers, the 16 floats of the matrix product, that product as a 4 x 4 M |
| `affineset M` | `SetAffineMatrix` | 16 floats |
| `affineget M f×16` | `GetAffineMatrix` into M | M |
| `dalloc i i i` | `DMatrixAlloc` [§4] | D |
| `dcheck D D i s x` | `DMatrixCheckDims` (kind, stream, prefix) | i |
| `daddmul D D O d d` | `DMatrixAddMul` (v1, v2) [§4, §12] | result, O |
| `dadd`, `dsub` `D D O` | `DMatrixAdd`, `DMatrixSubtract` | result, O |
| `dscale D O d` | `DMatrixScalarMul` | result, O |
| `dcopy`, `dtranspose` `D O` | `DMatrixCopy`, `DMatrixTranspose` | result, O |
| `dprint x D` | `DMatrixPrintFmt` to standard output (`DMatrixPrint` for the empty format) | i |
| `dmul D D O` | `DMatrixMultiply` | result, O |
| `ddot D D` | `DVectorDot` | d |
| `dcmpf M D` | `DMatrixCompareFMatrix` | d |
| `dcopyf M O` | `DMatrixCopyFMatrix` | result, O |
| `dconst d i i O`, `dzero i i O` | `DMatrixConstVal`, `DMatrixZero` | result, O |
| `dmaxabs D` | `DMatrixMaxAbs` | d |
| `dliststats n d...` | `DListStats` [§12] | 5 doubles (count, mean, deviation, minimum, maximum) |
| `midentity i O` | `MatrixIdentity` [§8.2] | result, O |
| `mtranspose`, `msquare`, `mdiag` `M O` | `MatrixTranspose`, `MatrixSquareElts`, `MatrixDiag` | result, O |
| `mclone M` | `MatrixClone` (a copy, then zero over its own sizes) | M |
| `madd`, `msub` `M M O` | `MatrixAdd`, `MatrixSubtract` | result, O |
| `mscale M O f` | `MatrixScalarMul` | result, O |
| `mclear M` | `MatrixClear` | result, M |
| `msymmetric`, `miszero`, `mcheck` `M` | `MatrixIsSymmetric`, `MatrixIsZero`, `MatrixCheck` | i |
| `mtrace M`, `msum M` | `MatrixTrace`, `MatrixSumElts` | f / d |
| `mconst f i i O`, `mzero i i O` | `MatrixConstVal`, `MatrixZero` | result, O |
| `mreshape M O i i` | `MatrixReshape` | result, O |
| `mcolscale M O` | `MatrixNormalizeColScale` | result, O |
| `mnormcol M O O` | `MatrixNormalizeCol` (output, scale) | result, O, the scale when given |
| `mregion`, `mrealregion`, `mimagregion` `M O i×6` | `MatrixCopyRegion`, `MatrixCopyRealRegion`, `MatrixCopyImagRegion` (start row, start column, rows, columns, destination row, destination column) [§8.3] | result, O |
| `mr2c M M O` | `MatrixRealToComplex` | result, O |
| `mrotation i f i O` | `MatrixAllocRotation` (O new), `MatrixReallocRotation` (O given) [§8.4] | result, O |
| `vlen M`, `vdot M M`, `vdist M M` | `VectorLen`, `VectorDot`, `VectorDistance` [§8.5] | f |
| `vcross M M O` | `VectorCrossProduct` | result, O |
| `vtriple M M M` | `VectorTripleProduct` | f |
| `v3angle M M` | `Vector3Angle` | d |
| `xyznorm f f f` | `XYZ_NORMALIZED_LOAD` | x y z length |
| `xyzangle f×3 f×3 f` | `XYZApproxAngle_knownLength` (the unit vector, the other's x, y, z, the norm) | f |
| `mprint s M i` | `MatrixPrint` (stream, precision) [§8.6] | i |
| `mprintfmt x M` | `MatrixPrintFmt` to standard output | i |
| `msimilarity M M O` | `MatrixSimilarityTransform` [§8.7] | result, O |
| `mgaussian i f i O`, `mgaussian2 i f f f i O` | `GaussianMatrix` (n, σ, norm), `GaussianMatrix2` (n, σ1, σ2, w1, norm) | result, O |
| `minverse M O`, `mdet M` | `MatrixInverse`, `MatrixDeterminant` [§8.8] | result, O / f |
| `mqr M M M` | `MatrixQRdecomposition` (input, Q, R) | i, Q, R |
| `primes i`, `pfactors i` | `GetPrimes`, `GetPrimeFactors` [§9] | the count and the numbers, or `null` |
| `pclosest`, `pclosestless` `i i` | `GetClosestPrimeFactor`, `GetClosestPrimeFactorLess` | i |
| `gaussdraws i i` | `srand48(seed)`, then `PDFgaussian` k times [§10] | k doubles |
| `todseed` | `PDFtodSeed` | its value (the clock: never cached, its digits masked) |
| `powell <cost> f×n 1 f×n² f i` | `OpenPowell` [§13.4]: the cost (below), the n parameters, 1 and the n × n directions row by row (or 0 and none: no directions), the tolerance, the iteration callback (1 set, 0 not) | iterations, final value, the parameters, the directions, then the cost's counts and hash |
| `powell2 <cost> f×n 1 f×n² f f i i i` | `OpenPowell2` [§13.5] (after the directions: `ftol`, the line tolerance, the maximum iterations, `Gdiag_no` before the call, the callback) | its value, then as `powell` |
| `dfpmin <cost> f×n f f i i i` | `OpenDFPMin` [§13.6] (after the parameters: the tolerance, the final value passed in, verbose (`DIAG_VERBOSE` in `Gdiag`), the step function (1/0), the user callback (1/0)) | its value, iterations, final value, the parameters, then the counts and hash |
| `nrvector i i`, `nrmatrix i i i i` | `vector(nl, nh)`, `matrix(nrl, nrh, ncl, nch)`, each element set to its index (`100 r + c` for the matrix) and read back, freed [§14] | the floats |
| `oeigen`, `oeigenns` `n f×n²` | `OpenEigenSystem`, `OpenNonSymmetricEigenSystem` [§17.1, §17.2] | i, n values, n² vector floats (7.0 where not written) |
| `meigen M O`, `meigenhelper M i O` | `MatrixEigenSystem`; the eigensystem helper with its symmetric flag (1 the symmetric path, 0 the other) [§17.3] | result, O, the values (7.0 where not written) |
| `msvd M M O` | `MatrixSVD` (A, w, V) [§17.4, §17.5] | result, O, then A and w after |
| `msvdvalues M i` | `MatrixSVDEigenValues` with an array of i floats (7.0 at first) | f, the i values |
| `mcond`, `mnscond` `M` | `MatrixConditionNumber`, `MatrixNSConditionNumber` | f |
| `msvdinverse`, `mpinverse`, `msvdpinverse` `M O` | `MatrixSVDInverse`, `MatrixPseudoInverse`, `MatrixSVDPseudoInverse` | result, O |
| `mreadtxt x x` | the second string written to the file named by the first, then `MatrixReadTxt` [§18.2] | M, `errno` |
| `mreadtxtmissing x` | `MatrixReadTxt` of a file that does not exist | M, `errno` |
| `mreadasc`, `mreadbin` `x O` | the string written to `fs-maths-probe.dat`, opened, then `MatrixAsciiReadFrom`, `MatrixReadFrom` | result, O, `errno`, the bytes read (`ftell`) |
| `mwriteasc`, `mwritebin` `M` | `MatrixAsciiWriteInto`, `MatrixWriteInto` into `fs-maths-probe.dat` | i, the file's bytes |
| `mwritetxt x M` | `MatrixWriteTxt` to the file named x | i, the file's bytes |
| `rfname x` | `RFname2Code` of a specification with that name [§19] | its value, the code stored |
| `rfinit i i` | `RFspecInit(seed, the default generator)`, then `RFspecSetSeed` with the second | name, code, parameters, mean, deviation, seed, the set's value, the seed after |

## The test costs

`<cost>` is `<kind> <n> <f0> <b> <q> <a[1..n]> <c[1..n]>` (floats but the first two; n from 1 to
32). The costs take FreeSurfer's 1-based float parameters `p[1..n]`; every operation is a float
operation, evaluated left to right as written:

- **kind 0** (a quadratic): `s = f0`; for i = 1..n: `d = p[i] - c[i]`, `s = s + a[i] * d * d`; then
  for i = 1..n-1: `s = s + b * ((p[i] - c[i]) * (p[i+1] - c[i+1]))`. Gradient: `g[i] = 2 * a[i] *
  (p[i] - c[i])`, then `+ b * (p[i-1] - c[i-1])` when i > 1, then `+ b * (p[i+1] - c[i+1])` when
  i < n.
- **kind 1** (Rosenbrock's): `s = f0`; for i = 1..n-1: `t = p[i+1] - p[i] * p[i]`, `u = 1 - p[i]`,
  `s = s + 100 * t * t + u * u`. Gradient: all 0, then for i = 1..n-1: `g[i] = g[i] + (-400 * t *
  p[i] - 2 * u)`, `g[i+1] = g[i+1] + 200 * t`.
- **kind 2**: kind 0's value, then `s = floorf(s * q) / q`; kind 0's gradient.
- **kind 3** (weighted absolute values): `s = f0`, then for i = 1..n `s = s + a[i] * |p[i] - c[i]|`;
  gradient `a[i] * sign(p[i] - c[i])` (0 at 0).

Every call is folded into a 64-bit FNV-1a hash (offset `0xcbf29ce484222325`, prime
`0x100000001b3`) of 32-bit words, each word's four bytes from the lowest: the value: the word 1,
the parameters' bits `p[1..n]`, the value's bits; the gradient: 2, the parameters, the gradient's
`g[1..n]`; the iteration callback: 3, n, then `p[0..n]` (element 0 included); the step function: 4,
the iteration number, the value's bits, `p[1..n]`; the user callback: 5, `p[1..n]`. The counts
printed are the values, gradients, iteration callbacks, step calls and user callbacks.

## The comparison loop

```sh
# the cases, one line each (and the calls per group)
python -P compare.py list --scratch <scratch>/maths
# the probe's reference hashes per group (the spec's §23 quotes them)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-maths-probe/compare.py refs \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe --scratch <scratch>/maths
# a candidate against the probe, in each mode
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-maths-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe \
  --candidate <the candidate> --scratch <scratch>/maths --mode reproduce   # then --mode fix
```

**The candidate** is the clean room's own program taking `batch <file>` with the lines above and
printing the same lines (its library behind the commands; each line's streams its own, a line that
ends the program `EXIT <status>`). It runs with `LARMORX_UPSTREAM_DEFECTS` set to the mode, in the
same working directory and environment as the probe. Its standard error may hold larmorx's defect
notices (`upstream defect <ID> ...` lines): the loop takes them out before comparing and keeps their
IDs. The cases are generated from `--seed` (default 7) into `<scratch>/batch`; files are written in
`<scratch>/work`, the working directory.

**The guard runs.** Each batch runs on the probe four times: plainly; plainly again, in another
process, with `MALLOC_PERTURB_` 0 (glibc's allocator as it is, the stack below each call zeroed);
and with `LD_PRELOAD` set to `guard_malloc.so`, `MALLOC_GUARD_=over` and `MALLOC_PERTURB_` 90, then
`MALLOC_GUARD_=under` and 165. A line whose four results are not the same read memory FreeSurfer did
not write or does not own (past an array's end: a crash in the first guard run, the fill byte in
the second; before its start: the other way round; memory never written or freed: the fill bytes,
a crash; memory whose contents follow the process's addresses: the second plain run; stack memory
nothing wrote: the stack's fill): a **memory** line. The clean room
gives 0 for such reads (spec §1), so these lines may differ. A memory line, or one where
FreeSurfer crashed (often a read further outside an array, which may or may not reach mapped
memory), counts in the reference hash by its place only (`<memory>`). One known case: `mpinverse`
of a matrix with fewer rows than columns and a given output of its own shape, which FreeSurfer
frees (FS-45): the printed state of that output is memory.

**What may differ.** A line is compared byte for byte (the result, both streams), the clock's
numbers masked (`todseed`, `rfinit 0`). A difference is allowed on a line where (1) FreeSurfer did
not finish (`TIMEOUT`, `CRASH`) and the candidate did; in the groups `eigen` and `svd` a `TIMEOUT`
line also needs the spec's refusal on the candidate's standard error (`never returns on this
matrix`; VNL's `suspicious return value (<info>) from SVDC`), in both modes (the batch's `refusal`);
(2) in the group `svd`, a line whose input holds an infinite or NaN float and that differs only in
the bits of NaNs (signs and payloads, in the printed numbers and in VNL's printed matrix: `nan` and
`-nan` taken as one): the official build's LINPACK kernels pass NaNs on in another order than the
main package's float SVD (spec §17.4; the batch's `nanbits`), counted as "NaN bits only"; (3) a
**memory** line; (4) in `--mode fix`, a line where the candidate noted a defect. Every other
difference is unexplained: exit status 1, the first three of each group printed. The loop prints,
per group, the identical lines and those of each allowed kind, and the IDs the candidate noted on
the allowed lines.

**The batches of both kinds** (2026-11-13, spec §17): `eigen-nonfinite` (symmetric
matrices of 1 to 5 rows with 1 to 3 infinite or NaN entries, on the diagonal, mirrored, in the upper
triangle only or below the diagonal, which the routine never reads; `oeigen`, the helper's symmetric
path, `meigen` where the matrix is symmetric without a NaN; and 11 picked matrices),
`svd-nonfinite` (matrices of up to 6 x 5 with such entries: `msvd`, `msvdvalues`, `mcond`,
`msvdpinverse`; 6 picked matrices; 16 condition numbers of finite rows and columns whose products
overflow) hold matrices on which FreeSurfer never returns and matrices it finishes: with `--mode
reproduce` the finished ones are compared as every other line. Each batch has a generator of its
own, so the groups' other batches keep their cases.

## Results (2026-11-13, the reference machine: glibc 2.35, Linux 6.8, x86-64)

- `refs`, seed 7, the prebuilt probe (commit 9d8409f9) and guard allocator (commit b43077c4): the
  hashes quoted in the spec's §23.2 (26,707 calls; 1,327 memory or crashed lines, 41 that end the
  program, 172 where FreeSurfer never returns); identical through the oracle-output cache (cold),
  from it (warm) and without it, in separate processes, which also classify every line alike.
- `compare` with the probe as its own candidate, `--mode reproduce`: no unexplained difference;
  26,610 lines identical, 71 memory lines whose contents changed from process to process, 26 where
  the probe crashed in one process and not in the other (`svd`'s `mpinverse` with a given output
  FreeSurfer frees).
- 2026-11-13 (track E, relay 4): the batches `eigen-nonfinite` and `svd-nonfinite` added; `refs
  --groups eigen,svd` cold, warm and without the cache give the spec's new hashes
  (`oracles/logs/fs-maths-probe-r4-refs-2026-11-13.log`); the groups' other batches unchanged.
  `standin.py` as the candidate (`--mode reproduce`, groups `eigen` and `svd`): no unexplained
  difference, 295 lines where FreeSurfer did not finish (each with the refusal), 164 "NaN bits only";
  with `--break refusal` the 295 are unexplained, with `--break nanbits` 86 lines of `eigen`:

  ```sh
  printf '#!/bin/sh\nexec <python 3.12> -P %s --save %s "$@"\n' \
    "$PWD/validation/investigations/freesurfer-maths-probe/standin.py" <scratch>/save > <scratch>/standin.sh
  chmod +x <scratch>/standin.sh   # after: compare.py refs ... --groups eigen,svd --save <scratch>/save
  PYTHONPATH=validation/src python -P validation/investigations/freesurfer-maths-probe/compare.py compare \
    --probe <the probe> --candidate <scratch>/standin.sh --scratch <scratch>/maths --groups eigen,svd
  ```
- About 40 minutes for a cold `refs` on the reference machine (most of it the guard runs of `eigen`
  and `svd`), seconds warm; `compare` about 10 minutes more (the candidate's runs).
