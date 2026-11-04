# SPDX-License-Identifier: Apache-2.0
for x in "a b" "a -b" "a -x" "a -maskNONE" "-d -maskNONE" "a -a" "a -o" "a b c" "a -b c" "a = b" "a -eq b" "a !" "a (" "a )" "-d -b" "a -nt" "a -ef" "a -z" "a -n" "-x -y -z"; do
  [ _$x != _ ] 2>&1; echo "[$x] s=$?"
done
