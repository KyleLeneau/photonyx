//! Rectangular N-dimensional subset selection (ADR 006 D5). A [`Region`] is
//! `{ start, shape }` in FITS axis order (fastest-varying axis first). The
//! run planner decomposes it into `shape[1..].product()` contiguous runs of
//! `shape[0]` elements, each satisfiable by a single positioned read — for a
//! 2D rectangle, one read per image row of the subset.
//!
//! This is cfitsio's `fits_read_subset` model: the number of bytes touched
//! is bounded by the region size, not the image size, and is exactly
//! assertable (`tests/region_conformance.rs`).

use crate::error::FitsError;

/// A rectangular subset of an image's pixel grid. Axis order matches
/// `NAXIS1..NAXISn` — the fastest-varying axis is index 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    start: Vec<usize>,
    shape: Vec<usize>,
}

/// One contiguous run of the plan: `len` elements starting at element
/// `src_elem` of the data unit, decoded into the region-shaped output
/// starting at element `dst_elem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Run {
    pub src_elem: u64,
    pub dst_elem: usize,
    pub len: usize,
}

impl Region {
    /// A region from explicit per-axis `start` and `shape`, both in FITS
    /// axis order. The two must have the same length; that it also matches
    /// the target image's dimensionality is checked at read time.
    pub fn new(start: impl Into<Vec<usize>>, shape: impl Into<Vec<usize>>) -> Region {
        Region {
            start: start.into(),
            shape: shape.into(),
        }
    }

    /// The 2D convenience case: a `w × h` rectangle whose lower corner is at
    /// pixel `(x, y)` — `x` along `NAXIS1`, `y` along `NAXIS2`.
    pub fn rect(x: usize, y: usize, w: usize, h: usize) -> Region {
        Region {
            start: vec![x, y],
            shape: vec![w, h],
        }
    }

    pub fn start(&self) -> &[usize] {
        &self.start
    }

    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Total element count (product of `shape`). Zero if any axis extent is
    /// zero.
    pub fn len(&self) -> usize {
        if self.shape.is_empty() {
            0
        } else {
            self.shape.iter().product()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Validates this region against an image `dims` (`NAXIS1..NAXISn`).
    /// Errors if the dimensionality disagrees or any axis runs past its
    /// bound.
    pub(crate) fn validate(&self, dims: &[usize]) -> Result<(), FitsError> {
        let oob = |dims: &[usize]| {
            FitsError::RegionOutOfBounds(
                format!("start={:?} shape={:?}", self.start, self.shape),
                dims.iter().map(|&d| d as u64).collect(),
            )
        };
        if self.start.len() != dims.len() || self.shape.len() != dims.len() {
            return Err(oob(dims));
        }
        for (axis, &dim) in dims.iter().enumerate() {
            let end = self.start[axis]
                .checked_add(self.shape[axis])
                .ok_or(FitsError::NaxisOverflow)?;
            if end > dim {
                return Err(oob(dims));
            }
        }
        Ok(())
    }

    /// Decomposes the region into contiguous element runs against an image
    /// of `dims`. Callers must have run [`validate`](Self::validate) first;
    /// this assumes matching dimensionality and in-bounds extents.
    ///
    /// Runs are yielded in output order, so `dst_elem` is simply
    /// `run_index * shape[0]`.
    pub(crate) fn plan_runs(&self, dims: &[usize]) -> Vec<Run> {
        let n = dims.len();
        if self.is_empty() {
            return Vec::new();
        }

        // Element strides for each axis (axis 0 is contiguous).
        let mut stride = vec![1u64; n];
        for axis in 1..n {
            stride[axis] = stride[axis - 1] * dims[axis - 1] as u64;
        }

        let run_len = self.shape[0];
        let num_runs: usize = self.shape[1..].iter().product::<usize>().max(1);
        let mut runs = Vec::with_capacity(num_runs);

        // Mixed-radix counter over the slower axes (1..n), axis 1 fastest.
        let mut idx = vec![0usize; n.saturating_sub(1)];
        for run_index in 0..num_runs {
            let mut src_elem = self.start[0] as u64;
            for axis in 1..n {
                src_elem += (self.start[axis] + idx[axis - 1]) as u64 * stride[axis];
            }
            runs.push(Run {
                src_elem,
                dst_elem: run_index * run_len,
                len: run_len,
            });

            for axis in 1..n {
                idx[axis - 1] += 1;
                if idx[axis - 1] < self.shape[axis] {
                    break;
                }
                idx[axis - 1] = 0;
            }
        }
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_start_shape_in_fits_order() {
        let r = Region::rect(3, 7, 10, 20);
        assert_eq!(r.start(), &[3, 7]);
        assert_eq!(r.shape(), &[10, 20]);
        assert_eq!(r.len(), 200);
    }

    #[test]
    fn validate_rejects_wrong_dimensionality() {
        let r = Region::rect(0, 0, 2, 2);
        assert!(matches!(
            r.validate(&[10, 10, 10]),
            Err(FitsError::RegionOutOfBounds(..))
        ));
    }

    #[test]
    fn validate_rejects_out_of_bounds_extent() {
        let r = Region::rect(8, 0, 4, 4); // 8 + 4 > 10 on axis 0
        assert!(matches!(
            r.validate(&[10, 10]),
            Err(FitsError::RegionOutOfBounds(..))
        ));
    }

    #[test]
    fn validate_accepts_full_extent() {
        let r = Region::new([0, 0], [10, 10]);
        assert!(r.validate(&[10, 10]).is_ok());
    }

    #[test]
    fn plan_1d_is_a_single_run() {
        let r = Region::new([5], [12]);
        assert_eq!(
            r.plan_runs(&[100]),
            vec![Run {
                src_elem: 5,
                dst_elem: 0,
                len: 12
            }]
        );
    }

    #[test]
    fn plan_2d_rect_is_one_run_per_subset_row() {
        // 4x4 window at (2,1) in a 10-wide image: rows y=1,2,3,4, cols 2..6.
        let r = Region::rect(2, 1, 4, 4);
        let runs = r.plan_runs(&[10, 10]);
        assert_eq!(runs.len(), 4);
        for (row, run) in runs.iter().enumerate() {
            assert_eq!(run.len, 4);
            assert_eq!(run.dst_elem, row * 4);
            assert_eq!(run.src_elem, (2 + (1 + row as u64) * 10));
        }
    }

    #[test]
    fn plan_3d_iterates_axis1_fastest_then_axis2() {
        // dims: NAXIS1=8, NAXIS2=8, NAXIS3=4. Window: start (1,2,1) shape (3,2,2).
        let r = Region::new([1, 2, 1], [3, 2, 2]);
        let runs = r.plan_runs(&[8, 8, 4]);
        // 2 (axis1) * 2 (axis2) = 4 runs, each 3 elements.
        assert_eq!(runs.len(), 4);
        let stride2 = 8u64;
        let stride3 = 64u64;
        // src = start0 + (start1 + i1)*stride2 + (start2 + i2)*stride3,
        // with (i1, i2) cycling (0,0) (1,0) (0,1) (1,1).
        let src = |i1: u64, i2: u64| 1 + (2 + i1) * stride2 + (1 + i2) * stride3;
        let expected = [src(0, 0), src(1, 0), src(0, 1), src(1, 1)];
        for (i, run) in runs.iter().enumerate() {
            assert_eq!(run.len, 3);
            assert_eq!(run.dst_elem, i * 3);
            assert_eq!(run.src_elem, expected[i], "run {i}");
        }
    }

    #[test]
    fn plan_4d_run_count_is_product_of_slower_axes() {
        let r = Region::new([0, 0, 0, 0], [2, 3, 4, 5]);
        let runs = r.plan_runs(&[10, 10, 10, 10]);
        assert_eq!(runs.len(), 3 * 4 * 5);
        assert!(runs.iter().all(|run| run.len == 2));
        assert_eq!(runs.last().unwrap().dst_elem, (3 * 4 * 5 - 1) * 2);
    }

    #[test]
    fn empty_region_plans_no_runs() {
        assert!(Region::rect(0, 0, 0, 4).plan_runs(&[10, 10]).is_empty());
        assert!(Region::rect(0, 0, 4, 0).plan_runs(&[10, 10]).is_empty());
    }
}
