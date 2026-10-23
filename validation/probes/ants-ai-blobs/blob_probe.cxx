// SPDX-License-Identifier: Apache-2.0
// A validation probe (not part of the larmorx package): antsAI's blob alignment, ANTs v2.6.5
// `Examples/antsAI.cxx:156-604` (`PatchCorrelation`, `GetBlobCorrespondenceMatrix`,
// `GetTransformFromFeatureMatching`; Copyright ConsortiumOfANTS, Apache-2.0), copied with
// instrumentation and built against the ANTs oracle's ITK, to print what ANTs computes at each
// step: the blobs of each image (`B index... radius value`), the non-zero entries of the
// correspondence matrix (`C i j value`), the pairs (`P i j`), the landmarks (`L pair`) and the
// landmark transform (`T matrix... offset...`), all as C99 hex floats. The parity suite compares
// larmorx's steps with these (docs/findings/ants-ai.md, "Blobs"); the evidence export shows them.
// Usage: blob_probe D fixed moving nExtract nMatch kind (0 affine, 1 rigid). Build: CMakeLists.txt.
#include "itkAffineTransform.h"
#include "itkGradientRecursiveGaussianImageFilter.h"
#include "itkImage.h"
#include "itkImageFileReader.h"
#include "itkLandmarkBasedTransformInitializer.h"
#include "itkLinearInterpolateImageFunction.h"
#include "itkMultiScaleLaplacianBlobDetectorImageFilter.h"
#include "itkNeighborhoodIterator.h"
#include "itkRigid2DTransform.h"
#include "itkVersorRigid3DTransform.h"
#include "vnl/algo/vnl_svd.h"
#include "vnl/algo/vnl_symmetric_eigensystem.h"
#include <cstdio>

static bool g_trace_patch = false;

template <typename TImage, typename TGradientImage, typename TInterpolator, typename TReal>
TReal
PatchCorrelation(itk::NeighborhoodIterator<TImage> fixedNeighborhood,
                 itk::NeighborhoodIterator<TImage> movingNeighborhood,
                 std::vector<unsigned int>         activeIndex,
                 std::vector<TReal>                weights,
                 typename TGradientImage::Pointer  fixedGradientImage,
                 typename TGradientImage::Pointer  movingGradientImage,
                 typename TInterpolator::Pointer   movingInterpolator)
{
  using RealType = TReal;
  using ImageType = TImage;
  const unsigned int ImageDimension = ImageType::ImageDimension;
  using PointType = typename ImageType::PointType;
  using GradientPixelType = itk::CovariantVector<RealType, ImageDimension>;
  using VectorType = vnl_vector<RealType>;
  using IndexType = typename ImageType::IndexType;

  unsigned int           numberOfIndices = activeIndex.size();
  std::vector<PointType> movingImagePatchPoints;
  VectorType             fixedSamples(numberOfIndices, 0);
  VectorType             movingSamples(numberOfIndices, 0);
  vnl_matrix<RealType> fixedGradientMatrix(numberOfIndices, ImageDimension, 0.0);
  vnl_matrix<RealType> movingGradientMatrix(numberOfIndices, ImageDimension, 0.0);
  PointType movingPointCentroid(0.0);
  RealType weight = itk::NumericTraits<RealType>::OneValue() / static_cast<RealType>(numberOfIndices);
  for (unsigned int i = 0; i < numberOfIndices; i++)
  {
    fixedSamples[i] = fixedNeighborhood.GetPixel(activeIndex[i]);
    movingSamples[i] = movingNeighborhood.GetPixel(activeIndex[i]);
    IndexType fixedIndex = fixedNeighborhood.GetIndex(activeIndex[i]);
    IndexType movingIndex = movingNeighborhood.GetIndex(activeIndex[i]);
    if (fixedGradientImage->GetRequestedRegion().IsInside(fixedIndex) &&
        movingGradientImage->GetRequestedRegion().IsInside(movingIndex))
    {
      GradientPixelType fixedGradient = fixedGradientImage->GetPixel(fixedIndex) * weights[i];
      GradientPixelType movingGradient = movingGradientImage->GetPixel(movingIndex) * weights[i];
      for (unsigned int j = 0; j < ImageDimension; j++)
      {
        fixedGradientMatrix(i, j) = fixedGradient[j];
        movingGradientMatrix(i, j) = movingGradient[j];
      }
      PointType movingPoint(0.0);
      movingGradientImage->TransformIndexToPhysicalPoint(movingIndex, movingPoint);
      for (unsigned int d = 0; d < ImageDimension; d++)
      {
        movingPointCentroid[d] += movingPoint[d] * static_cast<typename PointType::CoordinateType>(weight);
      }
      movingImagePatchPoints.push_back(movingPoint);
    }
    else
    {
      return 0.0;
    }
  }
  fixedSamples -= fixedSamples.mean();
  movingSamples -= movingSamples.mean();
  RealType fixedSd = std::sqrt(fixedSamples.squared_magnitude());
  vnl_matrix<RealType> fixedCovariance = fixedGradientMatrix.transpose() * fixedGradientMatrix;
  vnl_matrix<RealType> movingCovariance = movingGradientMatrix.transpose() * movingGradientMatrix;
  vnl_symmetric_eigensystem<RealType> fixedImagePrincipalAxes(fixedCovariance);
  vnl_symmetric_eigensystem<RealType> movingImagePrincipalAxes(movingCovariance);
  vnl_vector<RealType> fixedPrimaryEigenVector;
  vnl_vector<RealType> fixedSecondaryEigenVector;
  vnl_vector<RealType> movingPrimaryEigenVector;
  vnl_vector<RealType> movingSecondaryEigenVector;
  vnl_matrix<RealType> B;
  if (ImageDimension == 2)
  {
    fixedPrimaryEigenVector = fixedImagePrincipalAxes.get_eigenvector(1);
    movingPrimaryEigenVector = movingImagePrincipalAxes.get_eigenvector(1);
    B = outer_product(movingPrimaryEigenVector, fixedPrimaryEigenVector);
  }
  else if (ImageDimension == 3)
  {
    fixedPrimaryEigenVector = fixedImagePrincipalAxes.get_eigenvector(2);
    fixedSecondaryEigenVector = fixedImagePrincipalAxes.get_eigenvector(1);
    movingPrimaryEigenVector = movingImagePrincipalAxes.get_eigenvector(2);
    movingSecondaryEigenVector = movingImagePrincipalAxes.get_eigenvector(1);
    B = outer_product(movingPrimaryEigenVector, fixedPrimaryEigenVector) +
        outer_product(movingSecondaryEigenVector, fixedSecondaryEigenVector);
  }
  vnl_svd<RealType>    wahba(B);
  vnl_matrix<RealType> A = wahba.V() * wahba.U().transpose();
  if (g_trace_patch)
  {
    std::fprintf(stderr, "  fixedSd %a\n  fixedCov", fixedSd);
    for (unsigned i = 0; i < ImageDimension; ++i)
      for (unsigned j = 0; j < ImageDimension; ++j)
        std::fprintf(stderr, " %a", fixedCovariance(i, j));
    std::fprintf(stderr, "\n  movingCov");
    for (unsigned i = 0; i < ImageDimension; ++i)
      for (unsigned j = 0; j < ImageDimension; ++j)
        std::fprintf(stderr, " %a", movingCovariance(i, j));
    std::fprintf(stderr, "\n  A");
    for (unsigned i = 0; i < ImageDimension; ++i)
      for (unsigned j = 0; j < ImageDimension; ++j)
        std::fprintf(stderr, " %a", A(i, j));
    std::fprintf(stderr, "\n  centroid");
    for (unsigned d = 0; d < ImageDimension; ++d)
      std::fprintf(stderr, " %a", movingPointCentroid[d]);
    std::fprintf(stderr, "\n");
  }
  bool patchIsInside = true;
  for (unsigned int i = 0; i < numberOfIndices; i++)
  {
    PointType            movingImagePoint = movingImagePatchPoints[i];
    vnl_vector<RealType> movingImageVector(ImageDimension, 0.0);
    for (unsigned int d = 0; d < ImageDimension; d++)
    {
      movingImagePoint[d] -= movingPointCentroid[d];
      movingImageVector[d] = movingImagePoint[d];
    }
    vnl_vector<RealType> movingImagePointRotated = A * movingImageVector;
    for (unsigned int d = 0; d < ImageDimension; d++)
    {
      movingImagePoint[d] =
        static_cast<typename PointType::CoordinateType>(movingImagePointRotated[d]) + movingPointCentroid[d];
    }
    if (movingInterpolator->IsInsideBuffer(movingImagePoint))
    {
      movingSamples[i] = movingInterpolator->Evaluate(movingImagePoint);
    }
    else
    {
      patchIsInside = false;
      break;
    }
  }
  RealType correlation = 0.0;
  if (patchIsInside)
  {
    movingSamples -= movingSamples.mean();
    RealType movingSd = std::sqrt(movingSamples.squared_magnitude());
    correlation = inner_product(fixedSamples, movingSamples) / (fixedSd * movingSd);
    if (g_trace_patch)
      std::fprintf(stderr, "  movingSd %a ip %a\n", movingSd, inner_product(fixedSamples, movingSamples));
    if (std::isnan(correlation) || std::isinf(correlation))
    {
      correlation = 0.0;
    }
  }
  else if (g_trace_patch)
    std::fprintf(stderr, "  outside\n");
  return correlation;
}

template <typename TImage, typename TBlobFilter>
void
GetBlobCorrespondenceMatrix(typename TImage::Pointer            fixedImage,
                            typename TImage::Pointer            movingImage,
                            typename TBlobFilter::BlobsListType fixedBlobs,
                            typename TBlobFilter::BlobsListType movingBlobs,
                            float                               sigma,
                            unsigned int                        radiusValue,
                            vnl_matrix<float> &                 correspondenceMatrix)
{
  using ImageType = TImage;
  using RealType = float;
  using IndexType = typename ImageType::IndexType;
  using NeighborhoodIteratorType = itk::NeighborhoodIterator<ImageType>;
  const unsigned int ImageDimension = ImageType::ImageDimension;
  using GradientVectorType = itk::CovariantVector<RealType, ImageDimension>;
  using GradientImageType = itk::Image<GradientVectorType, ImageDimension>;
  using GradientImageFilterType = itk::GradientRecursiveGaussianImageFilter<ImageType, GradientImageType>;
  using GradientImageFilterPointer = typename GradientImageFilterType::Pointer;
  typename GradientImageFilterType::Pointer fixedGradientFilter = GradientImageFilterType::New();
  fixedGradientFilter->SetInput(fixedImage);
  fixedGradientFilter->SetSigma(sigma);
  typename GradientImageType::Pointer fixedGradientImage = fixedGradientFilter->GetOutput();
  fixedGradientImage->Update();
  fixedGradientImage->DisconnectPipeline();
  GradientImageFilterPointer movingGradientFilter = GradientImageFilterType::New();
  movingGradientFilter->SetInput(movingImage);
  movingGradientFilter->SetSigma(sigma);
  typename GradientImageType::Pointer movingGradientImage = movingGradientFilter->GetOutput();
  movingGradientImage->Update();
  movingGradientImage->DisconnectPipeline();
  correspondenceMatrix.set_size(fixedBlobs.size(), movingBlobs.size());
  correspondenceMatrix.fill(0);
  typename NeighborhoodIteratorType::RadiusType radius;
  radius.Fill(radiusValue);
  NeighborhoodIteratorType ItF(radius, fixedImage, fixedImage->GetLargestPossibleRegion());
  NeighborhoodIteratorType ItM(radius, movingImage, movingImage->GetLargestPossibleRegion());
  IndexType zeroIndex;
  zeroIndex.Fill(radiusValue);
  ItF.SetLocation(zeroIndex);
  std::vector<unsigned int> activeIndex;
  std::vector<RealType>     weights;
  RealType                  weightSum = 0.0;
  for (unsigned int i = 0; i < ItF.Size(); i++)
  {
    IndexType index = ItF.GetIndex(i);
    RealType  distance = 0.0;
    for (unsigned int j = 0; j < ImageDimension; j++)
    {
      distance += itk::Math::sqr(index[j] - zeroIndex[j]);
    }
    distance = std::sqrt(distance);
    if (distance <= radiusValue)
    {
      activeIndex.push_back(i);
      RealType weight = std::exp(-distance / itk::Math::sqr(radiusValue));
      weights.push_back(weight);
      weightSum += (weight);
    }
  }
  for (float & weight : weights)
  {
    weight /= weightSum;
  }
  std::fprintf(stderr, "PATCH %zu weightSum %a w0 %a\n", activeIndex.size(), weightSum, weights[0]);
  using ScalarInterpolatorType = itk::LinearInterpolateImageFunction<ImageType, RealType>;
  typename ScalarInterpolatorType::Pointer movingInterpolator = ScalarInterpolatorType::New();
  movingInterpolator->SetInputImage(movingImage);
  const char * tp = std::getenv("TRACE_PAIR");
  int tpi = -1, tpj = -1;
  if (tp)
    std::sscanf(tp, "%d,%d", &tpi, &tpj);
  for (unsigned int i = 0; i < fixedBlobs.size(); i++)
  {
    IndexType fixedIndex = fixedBlobs[i]->GetCenter();
    if (fixedImage->GetPixel(fixedIndex) > static_cast<typename ImageType::PixelType>(1.0e-4))
    {
      ItF.SetLocation(fixedIndex);
      for (unsigned int j = 0; j < movingBlobs.size(); j++)
      {
        IndexType movingIndex = movingBlobs[j]->GetCenter();
        if (movingImage->GetPixel(movingIndex) > static_cast<typename ImageType::PixelType>(1.0e-4))
        {
          ItM.SetLocation(movingIndex);
          g_trace_patch = ((int)i == tpi && (int)j == tpj);
          RealType correlation = PatchCorrelation<ImageType, GradientImageType, ScalarInterpolatorType, RealType>(
            ItF, ItM, activeIndex, weights, fixedGradientImage, movingGradientImage, movingInterpolator);
          g_trace_patch = false;
          if (correlation < itk::NumericTraits<RealType>::ZeroValue())
          {
            correlation = itk::NumericTraits<RealType>::ZeroValue();
          }
          correspondenceMatrix(i, j) = correlation;
        }
      }
    }
  }
}

template <typename TImage, typename TTransform>
typename TTransform::Pointer
GetTransformFromFeatureMatching(typename TImage::Pointer fixedImage,
                                typename TImage::Pointer movingImage,
                                unsigned int             numberOfBlobsToExtract,
                                unsigned int             numberOfBlobsToMatch)
{
  using RealType = float;
  using ImageType = TImage;
  using TransformType = TTransform;
  using PointType = typename ImageType::PointType;
  unsigned int radiusValue = 20;
  RealType     gradientSigma = 1.0;
  RealType     maxRadiusDifferenceAllowed = 0.25;
  RealType     distancePreservationThreshold = 0.1;
  RealType     minimumNumberOfNeighborhoodNodes = 3;
  RealType     minScale = std::pow(1.0, 1.0);
  RealType     maxScale = std::pow(2.0, 10.0);
  unsigned int stepsPerOctave = 10;
  using BlobFilterType = itk::MultiScaleLaplacianBlobDetectorImageFilter<ImageType>;
  using BlobPointer = typename BlobFilterType::BlobPointer;
  using BlobPairType = std::pair<BlobPointer, BlobPointer>;
  using BlobsListType = typename BlobFilterType::BlobsListType;
  typename BlobFilterType::Pointer blobFixedImageFilter = BlobFilterType::New();
  blobFixedImageFilter->SetStartT(minScale);
  blobFixedImageFilter->SetEndT(maxScale);
  blobFixedImageFilter->SetStepsPerOctave(stepsPerOctave);
  blobFixedImageFilter->SetNumberOfBlobs(numberOfBlobsToExtract);
  blobFixedImageFilter->SetInput(fixedImage);
  blobFixedImageFilter->Update();
  BlobsListType fixedImageBlobs = blobFixedImageFilter->GetBlobs();
  typename BlobFilterType::Pointer blobMovingImageFilter = BlobFilterType::New();
  blobMovingImageFilter->SetStartT(minScale);
  blobMovingImageFilter->SetEndT(maxScale);
  blobMovingImageFilter->SetStepsPerOctave(stepsPerOctave);
  blobMovingImageFilter->SetNumberOfBlobs(numberOfBlobsToExtract);
  blobMovingImageFilter->SetInput(movingImage);
  blobMovingImageFilter->Update();
  BlobsListType movingImageBlobs = blobMovingImageFilter->GetBlobs();
  for (int k = 0; k < 2; ++k)
  {
    BlobsListType & l = k == 0 ? fixedImageBlobs : movingImageBlobs;
    std::fprintf(stderr, "BLOBS %s %zu\n", k == 0 ? "fixed" : "moving", l.size());
    for (size_t i = 0; i < l.size(); ++i)
    {
      auto c = l[i]->GetCenter();
      std::fprintf(stderr, "B %zu", i);
      for (unsigned d = 0; d < ImageType::ImageDimension; ++d)
        std::fprintf(stderr, " %ld", (long)c[d]);
      std::fprintf(stderr, " %a %a\n", l[i]->GetObjectRadius(), l[i]->GetScaleSpaceValue());
    }
  }
  if (movingImageBlobs.empty() || fixedImageBlobs.empty())
  {
    std::cerr << "The moving image or fixed image blobs list is empty." << std::endl;
    return nullptr;
  }
  vnl_matrix<RealType> correspondenceMatrix;
  GetBlobCorrespondenceMatrix<ImageType, BlobFilterType>(
    fixedImage, movingImage, fixedImageBlobs, movingImageBlobs, gradientSigma, radiusValue, correspondenceMatrix);
  for (unsigned i = 0; i < correspondenceMatrix.rows(); ++i)
    for (unsigned j = 0; j < correspondenceMatrix.cols(); ++j)
      if (correspondenceMatrix(i, j) != 0)
        std::fprintf(stderr, "C %u %u %a\n", i, j, correspondenceMatrix(i, j));
  std::vector<BlobPairType> blobPairs;
  std::vector<std::pair<unsigned, unsigned>> pairIdx;
  BlobPointer bestBlob = nullptr;
  unsigned int matchPoint = 1;
  unsigned int fixedCount = 0;
  while ((matchPoint <= numberOfBlobsToMatch) && (fixedCount < fixedImageBlobs.size()))
  {
    unsigned int maxPair = correspondenceMatrix.arg_max();
    auto         maxRow = static_cast<unsigned int>(maxPair / correspondenceMatrix.cols());
    unsigned int maxCol = maxPair - maxRow * correspondenceMatrix.cols();
    BlobPointer  fixedBlob = fixedImageBlobs[maxRow];
    bestBlob = movingImageBlobs[maxCol];
    if (bestBlob && bestBlob->GetObjectRadius() > 1)
    {
      if (static_cast<RealType>(std::fabs(bestBlob->GetObjectRadius() - fixedBlob->GetObjectRadius())) <
          maxRadiusDifferenceAllowed)
      {
        if (fixedImage->GetPixel(fixedBlob->GetCenter()) > static_cast<typename ImageType::PixelType>(1.0e-4) &&
            movingImage->GetPixel(bestBlob->GetCenter()) > static_cast<typename ImageType::PixelType>(1.0e-4))
        {
          BlobPairType blobPairing = std::make_pair(fixedBlob, bestBlob);
          blobPairs.push_back(blobPairing);
          pairIdx.push_back({ maxRow, maxCol });
          matchPoint++;
        }
      }
    }
    correspondenceMatrix.set_row(maxRow, correspondenceMatrix.get_row(0).fill(0));
    correspondenceMatrix.set_column(maxCol, correspondenceMatrix.get_column(0).fill(0));
    fixedCount++;
  }
  for (auto & p : pairIdx)
    std::fprintf(stderr, "P %u %u\n", p.first, p.second);
  vnl_matrix<RealType> distanceRatios(blobPairs.size(), blobPairs.size(), 0.0);
  for (unsigned int i = 0; i < blobPairs.size(); i++)
  {
    PointType fixedPoint(0.0);
    fixedImage->TransformIndexToPhysicalPoint(blobPairs[i].first->GetCenter(), fixedPoint);
    PointType movingPoint(0.0);
    movingImage->TransformIndexToPhysicalPoint(blobPairs[i].second->GetCenter(), movingPoint);
    for (unsigned int j = 0; j < blobPairs.size(); j++)
    {
      PointType fixedNeighborPoint(0.0);
      fixedImage->TransformIndexToPhysicalPoint(blobPairs[j].first->GetCenter(), fixedNeighborPoint);
      PointType movingNeighborPoint(0.0);
      movingImage->TransformIndexToPhysicalPoint(blobPairs[j].second->GetCenter(), movingNeighborPoint);
      distanceRatios(i, j) = distanceRatios(j, i) = fixedNeighborPoint.SquaredEuclideanDistanceTo(movingNeighborPoint) /
                                                    fixedPoint.SquaredEuclideanDistanceTo(movingPoint);
    }
  }
  typename TransformType::Pointer transform = TransformType::New();
  using TransformInitializerType = itk::LandmarkBasedTransformInitializer<TransformType, ImageType, ImageType>;
  using PointsContainerType = typename TransformInitializerType::LandmarkPointContainer;
  PointsContainerType fixedLandmarks;
  PointsContainerType movingLandmarks;
  for (unsigned int i = 0; i < blobPairs.size(); i++)
  {
    unsigned int neighborhoodCount = 0;
    for (unsigned int j = 0; j < blobPairs.size(); j++)
    {
      if ((j != i) &&
          (std::fabs(distanceRatios(i, j) - itk::NumericTraits<RealType>::OneValue()) < distancePreservationThreshold))
      {
        neighborhoodCount++;
      }
    }
    if (neighborhoodCount >= minimumNumberOfNeighborhoodNodes)
    {
      PointType fixedPoint(0.0);
      fixedImage->TransformIndexToPhysicalPoint(blobPairs[i].first->GetCenter(), fixedPoint);
      fixedLandmarks.push_back(fixedPoint);
      PointType movingPoint(0.0);
      movingImage->TransformIndexToPhysicalPoint(blobPairs[i].second->GetCenter(), movingPoint);
      movingLandmarks.push_back(movingPoint);
      std::fprintf(stderr, "L %u\n", i);
    }
  }
  typename TransformInitializerType::Pointer transformInitializer = TransformInitializerType::New();
  transformInitializer->SetFixedLandmarks(fixedLandmarks);
  transformInitializer->SetMovingLandmarks(movingLandmarks);
  transformInitializer->SetTransform(transform);
  transformInitializer->InitializeTransform();
  return transform;
}

template <unsigned int D, typename TTransform>
int run(const char * f, const char * m, unsigned ne, unsigned nm)
{
  using ImageType = itk::Image<float, D>;
  auto rf = itk::ImageFileReader<ImageType>::New();
  rf->SetFileName(f);
  rf->Update();
  auto rm = itk::ImageFileReader<ImageType>::New();
  rm->SetFileName(m);
  rm->Update();
  typename ImageType::Pointer fi = rf->GetOutput();
  typename ImageType::Pointer mi = rm->GetOutput();
  auto t = GetTransformFromFeatureMatching<ImageType, TTransform>(fi, mi, ne, nm);
  if (!t)
    return 2;
  auto mat = t->GetMatrix();
  auto off = t->GetOffset();
  std::printf("T");
  for (unsigned i = 0; i < D; ++i)
    for (unsigned j = 0; j < D; ++j)
      std::printf(" %a", mat[i][j]);
  for (unsigned i = 0; i < D; ++i)
    std::printf(" %a", off[i]);
  std::printf("\n");
  return 0;
}

int main(int argc, char ** argv)
{
  if (argc < 7)
    return 1;
  int D = std::atoi(argv[1]);
  unsigned ne = std::atoi(argv[4]), nm = std::atoi(argv[5]);
  int kind = std::atoi(argv[6]);
  try
  {
    if (D == 3 && kind == 0)
      return run<3, itk::AffineTransform<double, 3>>(argv[2], argv[3], ne, nm);
    if (D == 3)
      return run<3, itk::VersorRigid3DTransform<double>>(argv[2], argv[3], ne, nm);
    if (D == 2 && kind == 0)
      return run<2, itk::AffineTransform<double, 2>>(argv[2], argv[3], ne, nm);
    return run<2, itk::Rigid2DTransform<double>>(argv[2], argv[3], ne, nm);
  }
  catch (itk::ExceptionObject & e)
  {
    std::printf("EXC %s\n", e.GetDescription());
    return 3;
  }
}
