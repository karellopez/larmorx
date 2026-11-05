setscale 4 force
clear U
clear UU
setrow UU 1.2 0 0 0  0.1 1 0 0  0 0 0.9 0  0 0 0 1
optimise 6 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
print U
optimise 7 UU:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
print U
