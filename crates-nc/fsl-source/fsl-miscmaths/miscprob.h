/*  miscprob.h

    Christian Beckmann & Mark Woolrich, FMRIB Image Analysis Group

    Copyright (C) 1999-2000 University of Oxford  */

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

// Miscellaneous maths functions that rely on libprob build ontop of miscmaths


#if !defined(__miscprob_h)
#define __miscprob_h

#include "miscmaths.h"
#include "cprob/libprob.h"
#include "stdlib.h"

namespace MISCMATHS {

//   ReturnMatrix betarnd(const int dim1, const int dim2,
// 		       const float a, const float b);

  NEWMAT::ReturnMatrix betapdf(const NEWMAT::RowVector& vals,
		       const float a, const float b);

  NEWMAT::ReturnMatrix unifrnd(const int dim1 = 1, const int dim2 = -1,
		       const float start = 0, const float end = 1);

  NEWMAT::ReturnMatrix normrnd(const int dim1 = 1, const int dim2 = -1,
		       const float mu = 0, const float sigma = 1);

  // returns nsamps*nparams matrix:
  NEWMAT::ReturnMatrix mvnrnd(const NEWMAT::RowVector& mu, const NEWMAT::SymmetricMatrix& covar, int nsamp = 1);

  float mvnpdf(const NEWMAT::RowVector& vals, const NEWMAT::RowVector& mu, const NEWMAT::SymmetricMatrix& covar);

  float bvnpdf(const NEWMAT::RowVector& vals, const NEWMAT::RowVector& mu, const NEWMAT::SymmetricMatrix& covar);

  float normpdf(const float val, const float mu = 0, const float var = 1);
  float lognormpdf(const float val, const float mu = 0, const float var = 1);

  NEWMAT::ReturnMatrix normpdf(const NEWMAT::RowVector& vals, const float mu = 0, const float var = 1);

  NEWMAT::ReturnMatrix normpdf(const NEWMAT::RowVector& vals, const NEWMAT::RowVector& mus,
		       const NEWMAT::RowVector& vars);

  NEWMAT::ReturnMatrix normcdf(const NEWMAT::RowVector& vals, const float mu = 0, const float var = 1);

  NEWMAT::ReturnMatrix gammapdf(const NEWMAT::RowVector& vals, const float mu = 0, const float var = 1);

  NEWMAT::ReturnMatrix gammacdf(const NEWMAT::RowVector& vals, const float mu = 0, const float var = 1);

//   NEWMAT::ReturnMatrix gammarnd(const int dim1, const int dim2,
// 			const float a, const float b);

  // returns n! * n matrix of all possible permutations
  NEWMAT::ReturnMatrix perms(const int n);


  class Mvnormrandm
    {
    public:
      Mvnormrandm(){}

      Mvnormrandm(const NEWMAT::RowVector& pmu, const NEWMAT::SymmetricMatrix& pcovar) :
	mu(pmu),
	covar(pcovar)
	{
	  NEWMAT::Matrix eig_vec;
	  NEWMAT::DiagonalMatrix eig_val;
	  EigenValues(covar,eig_val,eig_vec);

	  covarw = sqrt(eig_val)*eig_vec.t();
	}

      NEWMAT::ReturnMatrix next(int nsamp = 1) const
	{
	  NEWMAT::Matrix ret = ones(nsamp, 1)*mu + normrnd(nsamp,mu.Ncols())*covarw;
	  ret.Release();
	  return ret;
	}

      NEWMAT::ReturnMatrix next(const NEWMAT::RowVector& pmu, int nsamp = 1)
	{
	  mu=pmu;

	  NEWMAT::Matrix ret = ones(nsamp, 1)*mu + normrnd(nsamp,mu.Ncols())*covarw;
	  ret.Release();
	  return ret;
	}

      void setcovar(const NEWMAT::SymmetricMatrix& pcovar)
	{
	  covar=pcovar;

	  mu.ReSize(covar.Nrows());
	  mu=0;

	  NEWMAT::Matrix eig_vec;
	  NEWMAT::DiagonalMatrix eig_val;
	  EigenValues(covar,eig_val,eig_vec);

	  covarw = sqrt(eig_val)*eig_vec.t();
	}

    private:

      NEWMAT::RowVector mu;
      NEWMAT::SymmetricMatrix covar;

      NEWMAT::Matrix covarw;

    };
}
#endif
