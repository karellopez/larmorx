// SPDX-License-Identifier: Apache-2.0
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: mirrors FreeSurfer 8.2.0's source.
//
// The ITK part of FreeSurfer 8.2.0's AntsN4BiasFieldCorrectionFs (its lines 190-270): the same
// filter types on itk::Image<float, 3> (FreeSurfer's ITKImageType, include/mri.h 92) and the same
// calls, so that GCC 8.4.0 instantiates the same ITK 4.13.2 templates as the official program.
// Compiled only, never run (gcc84_n4_dumps.sh): its GIMPLE dumps show whether GCC 8.4.0 folds
// away the float roundings of ITK's B-spline fit (GCC bug 110182, docs/findings/ants-n4.md).

#include "itkBSplineControlPointImageFilter.h"
#include "itkDivideImageFilter.h"
#include "itkExpImageFilter.h"
#include "itkExtractImageFilter.h"
#include "itkImage.h"
#include "itkImageRegionIterator.h"
#include "itkN4BiasFieldCorrectionImageFilter.h"
#include "itkShrinkImageFilter.h"

typedef itk::Image<float, 3> ITKImageType;

ITKImageType::Pointer n4_like_freesurfer(ITKImageType::Pointer inputImage, ITKImageType::Pointer maskImage,
                                         int shrinkFactor, std::vector<int> numIters)
{
  typedef itk::N4BiasFieldCorrectionImageFilter<ITKImageType, ITKImageType, ITKImageType> CorrecterType;
  CorrecterType::Pointer correcter = CorrecterType::New();
  CorrecterType::VariableSizeArrayType maximumNumberOfIterations(numIters.size());
  for (unsigned int d = 0; d < numIters.size(); d++) maximumNumberOfIterations[d] = numIters[d];
  correcter->SetMaximumNumberOfIterations(maximumNumberOfIterations);
  correcter->SetNumberOfFittingLevels(numIters.size());
  correcter->SetConvergenceThreshold(0.0);

  typedef itk::ShrinkImageFilter<ITKImageType, ITKImageType> ShrinkerType;
  ShrinkerType::Pointer shrinker = ShrinkerType::New();
  shrinker->SetInput(inputImage);
  shrinker->SetShrinkFactors(shrinkFactor);
  ShrinkerType::Pointer maskshrinker = ShrinkerType::New();
  maskshrinker->SetInput(maskImage);
  maskshrinker->SetShrinkFactors(shrinkFactor);

  correcter->SetInput(shrinker->GetOutput());
  correcter->SetMaskImage(maskshrinker->GetOutput());
  correcter->Update();

  typedef itk::BSplineControlPointImageFilter<CorrecterType::BiasFieldControlPointLatticeType,
                                              CorrecterType::ScalarImageType>
    BSplinerType;
  BSplinerType::Pointer bspliner = BSplinerType::New();
  bspliner->SetInput(correcter->GetLogBiasFieldControlPointLattice());
  bspliner->SetSplineOrder(correcter->GetSplineOrder());
  bspliner->SetSize(inputImage->GetLargestPossibleRegion().GetSize());
  bspliner->SetOrigin(inputImage->GetOrigin());
  bspliner->SetDirection(inputImage->GetDirection());
  bspliner->SetSpacing(inputImage->GetSpacing());
  bspliner->Update();

  ITKImageType::Pointer logField = ITKImageType::New();
  logField->SetLargestPossibleRegion(inputImage->GetLargestPossibleRegion());
  logField->SetBufferedRegion(inputImage->GetBufferedRegion());
  logField->SetRequestedRegion(inputImage->GetRequestedRegion());
  logField->SetSpacing(inputImage->GetSpacing());
  logField->SetOrigin(inputImage->GetOrigin());
  logField->SetDirection(inputImage->GetDirection());
  logField->Allocate();
  itk::ImageRegionIterator<CorrecterType::ScalarImageType> ItB(bspliner->GetOutput(),
                                                               bspliner->GetOutput()->GetLargestPossibleRegion());
  itk::ImageRegionIterator<ITKImageType> ItF(logField, logField->GetLargestPossibleRegion());
  for (ItB.GoToBegin(), ItF.GoToBegin(); !ItB.IsAtEnd(); ++ItB, ++ItF) ItF.Set(ItB.Get()[0]);

  typedef itk::ExpImageFilter<ITKImageType, ITKImageType> ExpFilterType;
  ExpFilterType::Pointer expFilter = ExpFilterType::New();
  expFilter->SetInput(logField);
  expFilter->Update();

  typedef itk::DivideImageFilter<ITKImageType, ITKImageType, ITKImageType> DividerType;
  DividerType::Pointer divider = DividerType::New();
  divider->SetInput1(inputImage);
  divider->SetInput2(expFilter->GetOutput());
  divider->Update();

  ITKImageType::RegionType inputRegion;
  inputRegion.SetIndex(inputImage->GetLargestPossibleRegion().GetIndex());
  inputRegion.SetSize(inputImage->GetLargestPossibleRegion().GetSize());
  typedef itk::ExtractImageFilter<ITKImageType, ITKImageType> CropperType;
  CropperType::Pointer cropper = CropperType::New();
  cropper->SetInput(divider->GetOutput());
  cropper->SetExtractionRegion(inputRegion);
  cropper->SetDirectionCollapseToSubmatrix();
  cropper->Update();
  return cropper->GetOutput();
}
