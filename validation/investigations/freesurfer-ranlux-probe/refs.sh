#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The reference numbers quoted in specs/ranlux.md (§2.5, §4, §9), from fs_rng_probe.
# Usage: refs.sh <fs_rng_probe>
set -euo pipefail
P=${1:?probe}
echo "== raw: first 12, values 24-27, and SHA-256 of the first 100000 (decimal, one per line)"
for s in 53 0 1 2 1234 2147483562 2147483564 4294967295 4294967296 9223372036854775807 9223372036854775808 18446744073709551615 -53; do
  first=$("$P" raw "$s" 28 | tr '\n' ' ')
  sha=$("$P" raw "$s" 100000 | sha256sum | cut -c1-64)
  echo "seed $s: $(echo "$first" | cut -d' ' -f1-12) | 24-27: $(echo "$first" | cut -d' ' -f25-28) | $sha"
done
echo "== dist: first 3 draws (bits) and SHA-256 of 20000 draws (hex lines)"
while read -r name a b; do
  first=$("$P" dist "$name" 53 3 $a $b | tr '\n' ' ')
  sha=$("$P" dist "$name" 53 20000 $a $b | sha256sum | cut -c1-64)
  echo "$name($a $b) seed 53: $first| $sha"
done <<'EOF'
uni
uni_pos
flat -3.5 7.25
gaussian 2.5
ugauss
exponential 0.75
gamma 0 1
gamma 0.4 1.5
gamma 1 1
gamma 3 2
gamma 7.3 0.5
gamma 11 1
gamma 12 1
gamma 25 1
gamma 31.6 1
chisq 5
fdist 4 9
tdist 1.5
tdist 2
tdist 7
EOF
echo "== mixed: SHA-256 of 14000 draws"
for s in 53 1 4294967296; do
  echo "mixed seed $s: $("$P" mixed "$s" 14000 | sha256sum | cut -c1-64)"
done
