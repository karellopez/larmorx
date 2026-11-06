/*  unwarpfns.h

    Mark Jenkinson, FMRIB Image Analysis Group

    Copyright (C) 2000 University of Oxford  */

/*  Part of FSL - FMRIB's Software Library
    http://www.fmrib.ox.ac.uk/fsl
    fsl@fmrib.ox.ac.uk

    Developed at FMRIB (Oxford Centre for Functional Magnetic Resonance
    Imaging of the Brain), Department of Clinical Neurology, Oxford
    University, Oxford, UK


    LICENCE

    FMRIB Software Library, Release 6.0 (c) 2018, The University of
    Oxford (the "Software")

    The Software remains the property of the Oxford University Innovation
    ("the University").

    The Software is distributed "AS IS" under this Licence solely for
    non-commercial use in the hope that it will be useful, but in order
    that the University as a charitable foundation protects its assets for
    the benefit of its educational and research purposes, the University
    makes clear that no condition is made or to be implied, nor is any
    warranty given or to be implied, as to the accuracy of the Software,
    or that it will be suitable for any particular purpose or for use
    under any specific conditions. Furthermore, the University disclaims
    all responsibility for the use which is made of the Software. It
    further disclaims any liability for the outcomes arising from using
    the Software.

    The Licensee agrees to indemnify the University and hold the
    University harmless from and against any and all claims, damages and
    liabilities asserted by third parties (including claims for
    negligence) which arise directly or indirectly from the use of the
    Software or the sale of any products based on the Software.

    No part of the Software may be reproduced, modified, transmitted or
    transferred in any form or by any means, electronic or mechanical,
    without the express permission of the University. The permission of
    the University is not required if the said reproduction, modification,
    transmission or transference is done without financial return, the
    conditions of this Licence are imposed upon the receiver of the
    product, and all original and amended source code is included in any
    transmitted product. You may be held legally responsible for any
    copyright infringement that is caused or encouraged by your failure to
    abide by these terms and conditions.

    You are not permitted under this Licence to use this Software
    commercially. Use for which any financial return is received shall be
    defined as commercial use, and includes (1) integration of all or part
    of the source code or the Software into a product for sale or license
    by or on behalf of Licensee to third parties or (2) use of the
    Software or any derivative of it for research with the final aim of
    developing software products for sale or license to a third party or
    (3) use of the Software or any derivative of it for research with the
    final aim of developing non-software products for sale or license to a
    third party, or (4) use of the Software to provide any service to an
    external organisation for which payment is received. If you are
    interested in using the Software commercially, please contact Oxford
    University Innovation ("OUI"), the technology transfer company of the
    University, to negotiate a licence. Contact details are:
    fsl@innovation.ox.ac.uk quoting Reference Project 9564, FSL.*/

#if !defined(__unwarpfns_h)
#define __unwarpfns_h

#include <string>
#include <iostream>
#include <fstream>
#include <vector>
#include <map>
#include <algorithm>
#include "armawrap/newmat.h"
#include "newimage/newimageall.h"

namespace FUGUE {

  //////////////////////////////////////////////////////////////////////////

  int maximum_position(const NEWMAT::ColumnVector& vec);

  int maximum_position(const NEWMAT::Matrix& vec);

  void delete_row(NEWMAT::Matrix& mat, int r);

  void delete_column(NEWMAT::Matrix& mat, int c);

  void insert_row(NEWMAT::Matrix& mat, int r, const NEWMAT::Matrix& nrow);

  void insert_column(NEWMAT::Matrix& mat, int c, const NEWMAT::Matrix& ncol);

  NEWMAT::ColumnVector pava(const NEWMAT::ColumnVector& data, const NEWMAT::ColumnVector& weights);

  NEWMAT::ColumnVector pava(const NEWMAT::ColumnVector& data);

  //////////////////////////////////////////////////////////////////////////

  template <class T>
  void simple_erode(NEWIMAGE::volume<T>& binaryvol);

  template <class T>
  void simple_dilate(NEWIMAGE::volume<T>& binaryvol);

  NEWIMAGE::volume<float> despike_filter2D(const NEWIMAGE::volume<float>& vol, float threshold);

  NEWIMAGE::volume<float> median_filter2D(const NEWIMAGE::volume<float>& vol);

  NEWIMAGE::volume<float> masked_despike_filter2D(const NEWIMAGE::volume<float>& vol,
                                                  const NEWIMAGE::volume<float>& mask,
                                                  float threshold);

  NEWIMAGE::volume<float> masked_median_filter2D(const NEWIMAGE::volume<float>& vol,
                                                 const NEWIMAGE::volume<float>& mask);

  float basic_mask_threshold(const NEWIMAGE::volume<float>& absmap);

  NEWIMAGE::volume<float> make_basic_head_mask(const NEWIMAGE::volume<float>& absmap,
                                               float thresh);

  NEWIMAGE::volume<float> make_head_mask(const NEWIMAGE::volume<float>& absmap,
                                         float thresh);
  NEWIMAGE::volume<float> make_head_mask2D(const NEWIMAGE::volume<float>& absmap,
                                           float thresh);

  NEWIMAGE::volume<float> make_filled_head_mask(const NEWIMAGE::volume<float>& absmap);

  NEWIMAGE::volume<float> fill_head_mask(const NEWIMAGE::volume<float>& origmask);

  void fill_holes(NEWIMAGE::volume<float>& vol,
                  const NEWIMAGE::volume<float>& holemask,
                  const NEWIMAGE::volume<float>& mask);

  NEWIMAGE::volume<float> extrapolate_volume(const NEWIMAGE::volume<float>& datavol,
                                             const NEWIMAGE::volume<float>& origmask,
                                             const NEWIMAGE::volume<float>& enlargedmask);

  NEWIMAGE::volume<float> polynomial_extrapolate(const NEWIMAGE::volume<float>& datavol,
                                                 const NEWIMAGE::volume<float>& validmask,
                                                 int n, bool verbose=false);

  NEWIMAGE::volume<float> fourier_extrapolate(const NEWIMAGE::volume<float>& datavol,
                                              const NEWIMAGE::volume<float>& validmask,
                                              int n, bool verbose=false);

  void connection_matrices(const NEWIMAGE::volume<float>& phasemap,
                           const NEWIMAGE::volume<int>& label,
                           NEWMAT::Matrix& Qab,
                           NEWMAT::Matrix& Pab,
                           NEWMAT::Matrix& Nab,
                           double& K);

  void connection_matrices(const NEWIMAGE::volume<float>& phasemap,
                           const NEWIMAGE::volume<int>& label,
                           NEWMAT::Matrix& Kab);

  float calc_cost(const NEWMAT::Matrix& Mab,
                  const NEWMAT::Matrix& Qab,
                  const NEWMAT::Matrix& Pvec,
                  float K);

  float calc_delta_cost(const NEWMAT::Matrix& Mab,
                        const NEWMAT::Matrix& Qab,
                        const NEWMAT::Matrix& Pvec,
                        int j, float d);

  float calc_delta_cost(const NEWMAT::Matrix& Mab,
                        const NEWMAT::Matrix& Qab,
                        const NEWMAT::Matrix& Pvecon2PI,
                        int signedj);

  float wrap(float theta);

  void remove_linear_ramps(const NEWMAT::ColumnVector& ramp,
                           NEWIMAGE::volume<float>& ph,
                           const NEWIMAGE::volume<float>& mask);

  void restore_linear_ramps(const NEWMAT::ColumnVector& ramp,
                            NEWIMAGE::volume<float>& uph,
                            const NEWIMAGE::volume<float>& mask);

  NEWMAT::ColumnVector estimate_linear_ramps(const NEWIMAGE::volume<float>& ph,
                                             const NEWIMAGE::volume<float>& mask);

  ////////////////////////////////////////////////////////////////////////////

  NEWIMAGE::volume<float> apply_unwrapping(const NEWIMAGE::volume<float>& phasemap,
                                           const NEWIMAGE::volume<int>& label,
                                           const NEWMAT::Matrix& label_offsets);

  NEWIMAGE::volume<int> find_phase_labels(const NEWIMAGE::volume<float>& phasemap,
                                          const NEWIMAGE::volume<float>& mask,
                                          int n=4);

  NEWIMAGE::volume<int> find_phase_labels2D(const NEWIMAGE::volume<float>& phasemap,
                                            const NEWIMAGE::volume<float>& mask,
                                            int n, bool unique3Dlabels);

  NEWIMAGE::volume<float> unwrap(const NEWIMAGE::volume<float>& phasemap,
                                 const NEWIMAGE::volume<int>& label,
                                 bool verbose=false);

  NEWIMAGE::volume<float> unwrap2D(const NEWIMAGE::volume<float>& phasemap,
                                   const NEWIMAGE::volume<int>& label,
                                   bool verbose=false);

  NEWIMAGE::volume<float> calc_fmap(const NEWIMAGE::volume<float>& phase1,
                                    const NEWIMAGE::volume<float>& phase2,
                                    const NEWIMAGE::volume<float>& mask1,
                                    const NEWIMAGE::volume<float>& mask2,
                                    float asym_time);

  float fmap2pixshift_factor(const NEWIMAGE::volume<float>& fmap,
                             float pe_dwell_time,
                             const std::string& dir);

  NEWIMAGE::volume<float> yderiv(const NEWIMAGE::volume<float>& vol);

  NEWIMAGE::volume<float> limit_pixshift(const NEWIMAGE::volume<float>& pixshift,
                                         const NEWIMAGE::volume<float>& mask,
                                         float mindiff);

  NEWIMAGE::volume<float> apply_pixshift(const NEWIMAGE::volume<float>& epivol,
                                         const NEWIMAGE::volume<float>& pixshiftmap);

  ////////////////////////////////////////////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////


  // TEMPLATE BODIES

  template <class T>
  void simple_erode(NEWIMAGE::volume<T>& binaryvol)
  {
    binaryvol.setpadvalue(1.0);
    binaryvol.setextrapolationmethod(NEWIMAGE::constpad);
    NEWMAT::ColumnVector structelement(3);
    structelement = 1.0/3.0;
    binaryvol = convolve_separable(binaryvol,structelement,
                                   structelement,structelement);
    binaryvol.binarise(1.0 - 0.5/27.0);
  }

  template <class T>
  void simple_dilate(NEWIMAGE::volume<T>& binaryvol)
  {
    binaryvol.setpadvalue(0.0);
    binaryvol.setextrapolationmethod(NEWIMAGE::constpad);
    NEWMAT::ColumnVector structelement(3);
    structelement = 1.0/3.0;
    binaryvol = convolve_separable(binaryvol,structelement,
                                   structelement,structelement);
    binaryvol.binarise(0.5/27.0,100.0);
  }

}
#endif
