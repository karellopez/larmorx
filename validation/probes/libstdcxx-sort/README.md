# libstdc++'s `std::sort` with NaN values

FSL's `calc_bval` (the background value, mcflirt's resampling padding) sorts a volume's 2-voxel
border with `std::sort` and `operator<`. With NaN values that is no strict weak order, so C++
leaves the result undefined (FSL-22); libstdc++'s introsort still gives one deterministic order.
`larmorx_nc_core::stdsort::sort_f32_checked` reproduces it move for move in `reproduce`, with
every index checked against the vector's ends (a read outside would be memory that is not the
vector's: then NaN values are sorted last and the notice says so).

`sort_probe.cc` sorts 768 seeded vectors (sizes 0 to 5000; none, 1 %, 10 %, 50 %, 90 % and all
NaN, of both signs; few or many ties) with the compiler's own `std::sort`, once with `operator<`
and once with an equivalent comparator that records any read outside the vector (guard cells of
NaN around it stop every unguarded loop), and prints an FNV-1a hash of the results. The Rust unit
test `checked_sort_gives_libstdcxx_order_with_nan` (`crates-nc/larmorx-nc-core/src/stdsort.rs`)
recomputes that hash from the emulation.

2026-10-26: GCC 11.4 (Ubuntu 22.04) and conda-forge GCC 14.3 both print
`cases 768 outside 0 nan_inside 495 mismatch 0 hash 63bfd9ba2c340dd2`, the unit test's values
(FSL 6.0.7.17's mcflirt was built with GCC 13.2). None of the 768 made libstdc++ read outside
the vector (with `operator<` a NaN stops each unguarded loop as a guard element does:
`!(x < pivot)`, `!(pivot < x)` and `!(val < x)` all hold for it); the checks stay, as this is
evidence, not a proof for every input.

```
g++ -O2 -std=c++17 sort_probe.cc -o sort_probe && ./sort_probe          # the hash
./sort_probe --dump 200                                                 # one case's input and output bits
```

It holds no code from libstdc++: it calls `std::sort`.
