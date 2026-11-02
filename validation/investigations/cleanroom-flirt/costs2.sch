setscale 8
setoption smoothing 0
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
setrowparams UA 0.05 -0.03 0.02 2 -3 1 1.02 0.98 1.01 0.01 0 0
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
print U
setoption smoothing 1
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
print U
setscale 4
setoption smoothing 8
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
print U
setoption smoothing 16
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
print U
