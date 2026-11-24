#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
foreach v (x 1.5 08 010 -1 "" 3a +2)
  set e = $v
  echo "== $v"
  if($e > 0) echo positive
end
