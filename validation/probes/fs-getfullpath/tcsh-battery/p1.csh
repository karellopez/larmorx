#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
set fname = $argv[1];
echo "n=$#fname"
foreach w ($fname:q)
  echo "w=[$w]"
end
set dirname = `dirname $fname`;
echo "nd=$#dirname"
foreach w ($dirname:q)
  echo "d=[$w]"
end
set base = `basename $fname`;
echo "nb=$#base"
