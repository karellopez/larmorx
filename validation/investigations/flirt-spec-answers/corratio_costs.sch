setscale 8
setoption smoothing 8
setoption costfunction corratio
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
setrow UB 1 0 0 0  0 1 0 0  0 0 1 0  2.5 -1.5 3.25 1
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
measurecost 12 UB 0 0 0 0 0 0 rel
print U
setoption smoothing 0
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
measurecost 12 UB 0 0 0 0 0 0 rel
print U
setoption costfunction woods
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
measurecost 12 UB 0 0 0 0 0 0 rel
print U
