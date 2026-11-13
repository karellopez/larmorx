// SPDX-License-Identifier: Apache-2.0
// Stand-alone instances of VNL's inline vnl_inverse for float and double, 2 x 2 to 4 x 4, as GCC
// emits them out of line (orders.py reads their object code; the probe's family `order` calls
// them where the unit oracle's libraries hold no instance of their own).
#include <vnl/vnl_inverse.h>

template vnl_matrix_fixed<float, 2, 2> vnl_inverse(vnl_matrix_fixed<float, 2, 2> const &);
template vnl_matrix_fixed<float, 3, 3> vnl_inverse(vnl_matrix_fixed<float, 3, 3> const &);
template vnl_matrix_fixed<float, 4, 4> vnl_inverse(vnl_matrix_fixed<float, 4, 4> const &);
template vnl_matrix_fixed<double, 2, 2> vnl_inverse(vnl_matrix_fixed<double, 2, 2> const &);
template vnl_matrix_fixed<double, 3, 3> vnl_inverse(vnl_matrix_fixed<double, 3, 3> const &);
template vnl_matrix_fixed<double, 4, 4> vnl_inverse(vnl_matrix_fixed<double, 4, 4> const &);
