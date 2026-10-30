// SPDX-License-Identifier: Apache-2.0
//! Brain extraction compatible with FSL's `bet2` (`specs/bet.md` §4): a tessellated sphere
//! deformed onto the brain's surface (S. M. Smith, "Fast robust automated brain extraction",
//! Human Brain Mapping 17(3):143-155, 2002), and the brain, mask, outline, mesh and skull made
//! from it.
//!
//! All geometry is in millimetres along the voxel axes of the image in its radiological frame
//! (§3.2): voxel `(i, j, k)` is the point `(i dx, j dy, k dz)`.

pub mod image;
pub mod mesh;
pub mod params;
pub mod quatern;
pub mod raster;
pub mod skull;
pub mod surface;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use rayon::prelude::*;

pub use mesh::{Mesh, V3};
pub use params::Initial;
pub use surface::Deformation;

/// One volume in the radiological frame: x fastest, then y, then z.
#[derive(Clone, Debug, PartialEq)]
pub struct Vol {
    pub shape: [usize; 3],
    /// The voxel sizes in mm.
    pub voxel: [f64; 3],
    pub data: Vec<f32>,
}

impl Vol {
    pub fn new(shape: [usize; 3], voxel: [f64; 3], data: Vec<f32>) -> Vol {
        assert_eq!(shape.iter().product::<usize>(), data.len());
        Vol { shape, voxel, data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The same grid with other values.
    pub fn with_data(&self, data: Vec<f32>) -> Vol {
        Vol::new(self.shape, self.voxel, data)
    }
}

/// What one pass of the deformation measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    /// The self-intersection total (`None` with the exact test).
    pub total: Option<f64>,
    /// Whether it counts as self-intersecting.
    pub intersecting: bool,
}

/// The brain's surface found on an image.
#[derive(Clone, Debug)]
pub struct Found {
    pub init: Initial,
    pub mesh: Mesh,
    /// The final vertex positions.
    pub points: Vec<V3>,
    /// The passes run (the first, then the reruns).
    pub passes: Vec<Pass>,
}

/// Finds the brain's surface on `vol` (§4.3-§4.5): the initial parameters (with the radius
/// and the centre in mm when given), then passes of the deformation until the surface does
/// not self-intersect or the last pass.
pub fn find_surface(
    vol: &Vol,
    radius: Option<f64>,
    centre: Option<V3>,
    opts: &Deformation,
    n_threads: usize,
) -> Found {
    larmorx_core::parallel::with_threads(n_threads, || find_surface_in(vol, radius, centre, opts))
        .unwrap_or_else(|_| find_surface_in(vol, radius, centre, opts))
}

fn find_surface_in(
    vol: &Vol,
    radius: Option<f64>,
    centre: Option<V3>,
    opts: &Deformation,
) -> Found {
    let init = params::initial(vol, radius, centre);
    let mesh = Mesh::sphere(4);
    let start = surface::initial_surface(&mesh, &init);
    let mut passes = Vec::new();
    let mut pass = 0u32;
    loop {
        let points = surface::deform(&mesh, &start, vol, &init, opts, pass);
        let (total, intersecting) = if opts.exact {
            (None, surface::intersects(&mesh, &points))
        } else {
            let t = surface::self_intersection(&mesh, &points, &start);
            (Some(t), t > surface::THRESHOLD)
        };
        passes.push(Pass {
            total,
            intersecting,
        });
        if !intersecting || pass >= surface::LAST_PASS {
            return Found {
                init,
                mesh,
                points,
                passes,
            };
        }
        pass += 1;
    }
}

/// Why a mask cannot be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskError {
    /// The surface's centroid is not a number (the surface diverged; FSL-43).
    NotANumber,
}

/// The brain mask of a surface (§4.6): 1 inside and on the surface. FSL-43: a centroid outside
/// the image starts the fill at the nearest voxel inside (default) or as FSL does by the linear
/// index (`reproduce`); a centroid that is not a number is an error (default) or no fill.
pub fn brain_mask(
    mesh: &Mesh,
    points: &[V3],
    shape: [usize; 3],
    voxel: [f64; 3],
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<Vec<u8>, MaskError> {
    let n: usize = shape.iter().product();
    let mut grid = vec![1u8; n];
    raster::draw(mesh, points, shape, voxel, raster::Rounding::Floor, |k| {
        grid[k] = 0
    });
    let centroid = mesh::scale(
        points.iter().fold([0.0; 3], |a, p| mesh::add(a, *p)),
        1.0 / points.len().max(1) as f64,
    );
    match raster::voxel_of(centroid, voxel) {
        None => {
            if mode.fix() {
                return Err(MaskError::NotANumber);
            }
            defects::push_once(
                notices,
                Notice::new(
                    &defects::FSL_43,
                    mode,
                    "the surface's centroid is not a number: no fill (FSL writes far outside the volume there and crashes); the mask is the drawn surface",
                ),
            );
        }
        Some(start) => {
            let inside = (0..3).all(|a| start[a] >= 0 && (start[a] as usize) < shape[a]);
            if inside {
                let s = start.map(|x| x as usize);
                raster::fill(&mut grid, shape, s, 1, 0);
            } else if mode.fix() {
                let s = [0, 1, 2].map(|a| start[a].clamp(0, shape[a] as i64 - 1) as usize);
                raster::fill(&mut grid, shape, s, 1, 0);
                defects::push_once(
                    notices,
                    Notice::new(
                        &defects::FSL_43,
                        mode,
                        format!(
                            "the mask's fill starts at the voxel inside the image nearest the surface's centroid ({:?}, outside the image); FSL starts outside the volume",
                            start
                        ),
                    ),
                );
            } else {
                let beyond = raster::fill_by_index(&mut grid, shape, start);
                let detail = if beyond > 0 {
                    format!(
                        "the mask's fill starts outside the image at voxel {start:?}, and FSL's fill reaches places by their linear index; FSL also reads memory beyond the volume ({beyond} reads, taken as not part of the mask)"
                    )
                } else {
                    format!(
                        "the mask's fill starts outside the image at voxel {start:?}, and FSL's fill reaches places by their linear index"
                    )
                };
                defects::push_once(notices, Notice::new(&defects::FSL_43, mode, detail));
            }
        }
    }
    grid.par_iter_mut()
        .with_min_len(1 << 16)
        .for_each(|g| *g = 1 - *g);
    Ok(grid)
}

/// The surface drawn on a copy of `vol` at its maximum (the outline, §4.7).
pub fn outline(mesh: &Mesh, points: &[V3], vol: &Vol) -> Vec<f32> {
    let max = params::range(vol).1;
    let mut out = vol.data.clone();
    raster::draw(
        mesh,
        points,
        vol.shape,
        vol.voxel,
        raster::Rounding::Trunc,
        |k| out[k] = max,
    );
    out
}

/// The CT conversion from Hounsfield to Cormack units (§4.8).
pub fn cormack(v: f32) -> f32 {
    let mut v = v.max(-1024.0);
    v += 1024.0;
    let e1 = (v - 900.0).max(0.0);
    let e9 = e1.min(200.0) * 9.0;
    v = v + e1 + e9;
    if v > 3000.0 { 100.0 } else { v }
}

/// The brain image: the input times the mask (float32).
pub fn masked(values: &[f32], mask: &[u8]) -> Vec<f32> {
    values
        .par_iter()
        .zip(mask.par_iter())
        .with_min_len(1 << 16)
        .map(|(&v, &m)| v * f32::from(m))
        .collect()
}
