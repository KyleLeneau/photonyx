//! Reference selection and output geometry (ADR 007, P9-T1 / P9-T3).
//!
//! Two-pass registration measures every frame before registering any of them,
//! which is what makes it possible to *choose* the reference rather than take
//! the first frame. The choice matters: every other frame is warped into the
//! reference's pixel grid, so a reference with poor seeing or a thin star field
//! costs resolution and correspondences on all of them.

use super::psf::{PsfStar, amplitude_threshold_for, filter_stars_by_amplitude, fwhm_stats};
use super::transform::{Point, Transform};
use crate::error::ImageProcError;
use crate::runtime::ImageDims;

/// Weight on the star-count term of the reference score, relative to the
/// seeing term.
///
/// The score is `2·FWHM·(cap − stars)/cap + FWHM`: a frame with a full star
/// field is scored at its FWHM, and one with no stars at three times it. So
/// losing the whole star field is worth the same as seeing three times worse,
/// and the two terms trade off smoothly in between. That ratio is the
/// substantive choice here, and it is Siril's.
const COUNT_WEIGHT: f64 = 2.0;

/// Fraction of the best frame's star count a frame must reach to be eligible as
/// the reference.
///
/// A frame with a quarter of the stars may still score well on seeing — a short
/// gap in the clouds gives sharp stars and few of them — and choosing it would
/// leave every other frame matching against a field that is mostly missing.
const MIN_COUNT_FRACTION: f64 = 0.5;

/// What the reference selection ranks frames on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameQuality {
    /// Index into the sequence.
    pub index: usize,
    pub stars: usize,
    /// Major-axis FWHM, pixels — the frame's seeing.
    pub fwhm: f64,
    pub roundness: f64,
}

impl FrameQuality {
    /// Summarizes one frame's star list, or `None` when it has no stars.
    pub fn from_stars(index: usize, stars: &[PsfStar]) -> Option<Self> {
        let stats = fwhm_stats(stars)?;
        Some(Self {
            index,
            stars: stars.len(),
            fwhm: stats.mean,
            roundness: stats.mean_roundness,
        })
    }

    /// Siril's reference score. Lower is better.
    pub fn score(&self, best_count: usize) -> f64 {
        if best_count == 0 {
            return f64::INFINITY;
        }
        let missing = best_count.saturating_sub(self.stars) as f64 / best_count as f64;
        COUNT_WEIGHT * self.fwhm * missing + self.fwhm
    }
}

/// Summarizes every frame's star list, making the counts comparable when they
/// have been capped.
///
/// The cap is the problem this solves. With `max_stars` set to 2,000, every
/// frame of a rich field reports exactly 2,000 stars and the count term of the
/// score carries no information at all — a frame that could have yielded 8,000
/// ties with one that barely reached the cap. So when frames are capped, all of
/// them are re-counted above one common amplitude: the brightest of their
/// individual cut-offs. A frame that went deeper keeps its full 2,000 above that
/// line, a shallower one keeps fewer, and the counts mean something again.
///
/// Each entry carries its own sequence `index`, and a frame with no stars left
/// after the re-count is simply absent — it is not an eligible reference, which
/// is the only question this function answers. It says nothing about whether
/// that frame can be *registered*, which depends on its own star list.
pub fn frame_qualities(star_lists: &[&[PsfStar]], cap: usize) -> Vec<FrameQuality> {
    let capped = star_lists.iter().filter(|s| s.len() >= cap).count();
    // Only worth doing when the cap actually bit on more than one frame; with
    // one capped frame the ordering is unaffected.
    if cap == 0 || capped < 2 {
        return star_lists
            .iter()
            .enumerate()
            .filter_map(|(i, stars)| FrameQuality::from_stars(i, stars))
            .collect();
    }

    let common_threshold = star_lists
        .iter()
        .filter(|stars| stars.len() >= cap)
        .map(|stars| cap_cutoff(stars, cap))
        .fold(0.0f64, f64::max);

    star_lists
        .iter()
        .enumerate()
        .filter_map(|(i, stars)| {
            let kept = filter_stars_by_amplitude(stars, common_threshold);
            FrameQuality::from_stars(i, &kept)
        })
        .collect()
}

/// The amplitude of the `cap`-th brightest star: the level at which a capped
/// list was truncated.
///
/// [`amplitude_threshold_for`] reports no cut for a list no longer than the cap,
/// which is right for its own purpose and wrong here — a list of exactly `cap`
/// stars was truncated *at* its faintest one.
fn cap_cutoff(stars: &[PsfStar], cap: usize) -> f64 {
    if stars.len() > cap {
        amplitude_threshold_for(stars, cap)
    } else {
        stars.iter().map(|s| s.amplitude).fold(f64::MAX, f64::min)
    }
}

/// Picks the frame every other frame should be registered to.
///
/// Returns `None` when no frame has any stars.
pub fn select_reference(qualities: &[FrameQuality]) -> Option<usize> {
    let best_count = qualities.iter().map(|q| q.stars).max()?;
    let floor = (MIN_COUNT_FRACTION * best_count as f64).ceil() as usize;

    qualities
        .iter()
        .filter(|q| q.stars >= floor)
        .min_by(|a, b| {
            a.score(best_count)
                .total_cmp(&b.score(best_count))
                // Ties go to the earlier frame, so the choice does not depend on
                // iteration order.
                .then_with(|| a.index.cmp(&b.index))
        })
        .map(|q| q.index)
}

/// How the output image's geometry is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Framing {
    /// The reference frame's own geometry. Pixels a frame does not cover are
    /// left empty.
    #[default]
    Current,
    /// The largest rectangle every frame covers, so the output has no partially
    /// exposed border at all.
    Minimum,
    /// The union of every frame's footprint. Out of scope (ADR 007 Scope).
    Maximum,
    /// Centred on the sequence's centre of gravity. Out of scope.
    CenterOfGravity,
}

/// The output image's shape, and where it sits in the reference frame's
/// coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputGeometry {
    pub dims: ImageDims,
    /// Offset of the output's origin within the reference frame, in pixels.
    pub x: f64,
    pub y: f64,
}

impl OutputGeometry {
    /// The transform that takes reference-frame coordinates into output
    /// coordinates. Composed onto every frame's transform before warping.
    pub fn shift(&self) -> Transform {
        Transform::shift(-self.x, -self.y)
    }
}

/// Chooses the output geometry for a registered sequence.
///
/// `transforms` holds one entry per frame, mapping that frame onto the
/// reference; `None` marks a frame that will not be included.
pub fn output_geometry(
    framing: Framing,
    dims: ImageDims,
    transforms: &[Option<Transform>],
) -> Result<OutputGeometry, ImageProcError> {
    match framing {
        Framing::Current => Ok(OutputGeometry {
            dims,
            x: 0.0,
            y: 0.0,
        }),
        Framing::Minimum => minimum_roi(dims, transforms),
        Framing::Maximum | Framing::CenterOfGravity => Err(ImageProcError::Unsupported(format!(
            "{framing:?} framing is out of scope for the native engine; \
             use Current or Minimum"
        ))),
    }
}

/// The largest axis-aligned rectangle contained in every frame's footprint.
///
/// A rotated frame's footprint in reference coordinates is a quadrilateral, and
/// the largest axis-aligned rectangle inside one is bounded by its corners: the
/// left edge cannot pass either of the two leftmost corners, and so on for the
/// other three sides. Taking the second-smallest and second-largest corner
/// coordinate on each axis gives exactly that bound, and intersecting those
/// rectangles over the sequence gives a region every frame covers.
///
/// This is conservative rather than optimal — for a frame rotated by 45° the
/// true inscribed rectangle is larger — but it is never *wrong*, which is the
/// property that matters: a region claimed to be fully covered and not is a
/// border of partially stacked pixels.
fn minimum_roi(
    dims: ImageDims,
    transforms: &[Option<Transform>],
) -> Result<OutputGeometry, ImageProcError> {
    let (w, h) = ((dims.width - 1) as f64, (dims.height - 1) as f64);
    let mut left = 0.0f64;
    let mut top = 0.0f64;
    let mut right = w;
    let mut bottom = h;

    for transform in transforms.iter().flatten() {
        let corners: Vec<Point> = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .into_iter()
            .map(|(x, y)| transform.apply(Point::new(x, y)))
            .collect();
        if corners.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(ImageProcError::Registration(
                "a transform maps a frame corner to infinity; cannot compute minimum framing"
                    .into(),
            ));
        }

        let mut xs: Vec<f64> = corners.iter().map(|p| p.x).collect();
        let mut ys: Vec<f64> = corners.iter().map(|p| p.y).collect();
        xs.sort_by(f64::total_cmp);
        ys.sort_by(f64::total_cmp);

        left = left.max(xs[1]);
        right = right.min(xs[2]);
        top = top.max(ys[1]);
        bottom = bottom.min(ys[2]);
    }

    // Inclusive bounds rounded inward, so every output pixel is one every frame
    // genuinely covers.
    let x0 = left.ceil();
    let y0 = top.ceil();
    let width = (right.floor() - x0 + 1.0).max(0.0) as usize;
    let height = (bottom.floor() - y0 + 1.0).max(0.0) as usize;
    if width == 0 || height == 0 {
        return Err(ImageProcError::Registration(
            "the frames have no common area; minimum framing would be empty".into(),
        ));
    }

    Ok(OutputGeometry {
        dims: ImageDims::new(width, height, dims.channels),
        x: x0,
        y: y0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Matrix3;

    fn star(amplitude: f64, fwhm: f64) -> PsfStar {
        PsfStar {
            x0: 0.0,
            y0: 0.0,
            sx: fwhm / 2.3548,
            sy: fwhm / 2.3548,
            fwhm_x: fwhm,
            fwhm_y: fwhm,
            angle: 0.0,
            amplitude,
            background: 0.01,
            beta: None,
            roundness: 1.0,
            rmse: 1e-4,
            peak: amplitude,
            layer: 0,
        }
    }

    /// `count` stars of the given FWHM, amplitudes descending from `brightest`.
    fn field(count: usize, fwhm: f64, brightest: f64) -> Vec<PsfStar> {
        (0..count)
            .map(|i| star(brightest - 0.0001 * i as f64, fwhm))
            .collect()
    }

    #[test]
    fn the_sharpest_full_field_wins() {
        let qualities = vec![
            FrameQuality {
                index: 0,
                stars: 1000,
                fwhm: 4.0,
                roundness: 0.9,
            },
            FrameQuality {
                index: 1,
                stars: 1000,
                fwhm: 3.0,
                roundness: 0.9,
            },
            FrameQuality {
                index: 2,
                stars: 1000,
                fwhm: 3.5,
                roundness: 0.9,
            },
        ];
        assert_eq!(select_reference(&qualities), Some(1));
    }

    #[test]
    fn a_sharp_frame_with_few_stars_does_not_win() {
        // Half the star count, a third better seeing: the count term more than
        // cancels the seeing advantage.
        let qualities = vec![
            FrameQuality {
                index: 0,
                stars: 1000,
                fwhm: 3.0,
                roundness: 0.9,
            },
            FrameQuality {
                index: 1,
                stars: 500,
                fwhm: 2.0,
                roundness: 0.9,
            },
        ];
        assert_eq!(select_reference(&qualities), Some(0));
    }

    #[test]
    fn a_frame_below_half_the_best_count_is_not_eligible_at_all() {
        // Even with perfect seeing: a quarter of the field is not a reference.
        let qualities = vec![
            FrameQuality {
                index: 0,
                stars: 1000,
                fwhm: 4.0,
                roundness: 0.9,
            },
            FrameQuality {
                index: 1,
                stars: 249,
                fwhm: 0.5,
                roundness: 0.9,
            },
        ];
        assert_eq!(select_reference(&qualities), Some(0));
    }

    #[test]
    fn an_empty_sequence_has_no_reference() {
        assert_eq!(select_reference(&[]), None);
    }

    #[test]
    fn capped_star_counts_are_made_comparable_again() {
        // Three frames, all capped at 10. The second went deepest (its tenth
        // star is the faintest), the third barely reached the cap.
        let cap = 10;
        let deep: Vec<PsfStar> = field(10, 3.0, 0.9); // amplitudes 0.9 .. 0.8991
        // Every star fainter than the deep frame's faintest.
        let shallow: Vec<PsfStar> = (0..10).map(|i| star(0.5 - 0.02 * i as f64, 3.0)).collect();
        let lists: Vec<&[PsfStar]> = vec![&deep, &shallow];

        let qualities = frame_qualities(&lists, cap);
        // The common threshold is the brighter of the two cut-offs — the deep
        // frame's tenth star — so the shallow frame loses every star below it
        // and drops out of the ranking entirely.
        assert_eq!(qualities.len(), 1, "{qualities:?}");
        assert_eq!(qualities[0].index, 0);
        assert_eq!(qualities[0].stars, 10);
        assert_eq!(select_reference(&qualities), Some(0));
    }

    #[test]
    fn uncapped_counts_are_left_alone() {
        let a = field(40, 3.0, 0.9);
        let b = field(30, 3.0, 0.4);
        let lists: Vec<&[PsfStar]> = vec![&a, &b];
        let qualities = frame_qualities(&lists, 2000);
        assert_eq!(qualities[0].stars, 40);
        assert_eq!(qualities[1].stars, 30);
    }

    #[test]
    fn current_framing_is_the_reference_geometry() {
        let dims = ImageDims::new(4000, 3000, 1);
        let geometry = output_geometry(
            Framing::Current,
            dims,
            &[Some(Transform::shift(50.0, -20.0))],
        )
        .expect("current framing");
        assert_eq!(geometry.dims, dims);
        assert_eq!((geometry.x, geometry.y), (0.0, 0.0));
    }

    #[test]
    fn minimum_framing_trims_to_the_common_area() {
        let dims = ImageDims::new(1000, 800, 1);
        // One frame shifted right and down by 10, one left and up by 4: the
        // common area loses 10 px on the left/top and 4 px on the right/bottom.
        let transforms = vec![
            Some(Transform::identity()),
            Some(Transform::shift(10.0, 10.0)),
            Some(Transform::shift(-4.0, -4.0)),
        ];
        let geometry = output_geometry(Framing::Minimum, dims, &transforms).expect("minimum");
        assert_eq!((geometry.x, geometry.y), (10.0, 10.0));
        assert_eq!(geometry.dims, ImageDims::new(986, 786, 1));

        // The shift it reports puts that region's origin at the output origin.
        let mapped = geometry.shift().apply(Point::new(10.0, 10.0));
        assert_eq!((mapped.x, mapped.y), (0.0, 0.0));
    }

    #[test]
    fn minimum_framing_is_conservative_under_rotation() {
        let dims = ImageDims::new(1000, 800, 1);
        // A 2-degree rotation about the frame centre: the ROI must exclude the
        // corners the rotation swept out of the frame.
        let centre = Transform::shift(500.0, 400.0);
        let rotation = centre
            .compose(
                &Transform::similarity(1.0, 2f64.to_radians(), 0.0, 0.0)
                    .compose(&Transform::shift(-500.0, -400.0))
                    .expect("composable"),
            )
            .expect("composable");

        let geometry = output_geometry(Framing::Minimum, dims, &[Some(rotation)]).expect("minimum");
        assert!(geometry.dims.width < dims.width, "{:?}", geometry.dims);
        assert!(geometry.dims.height < dims.height, "{:?}", geometry.dims);

        // Every corner of the ROI must fall inside the rotated frame's
        // footprint, which is the property the conservative bound buys.
        let inverse = rotation.invert().expect("invertible");
        let (x0, y0) = (geometry.x, geometry.y);
        let (x1, y1) = (
            x0 + (geometry.dims.width - 1) as f64,
            y0 + (geometry.dims.height - 1) as f64,
        );
        for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            let source = inverse.apply(Point::new(x, y));
            assert!(
                source.x >= -0.5
                    && source.y >= -0.5
                    && source.x <= dims.width as f64 - 0.5
                    && source.y <= dims.height as f64 - 0.5,
                "ROI corner ({x}, {y}) samples outside the frame at {source:?}"
            );
        }
    }

    #[test]
    fn framing_modes_out_of_scope_are_typed_errors() {
        let dims = ImageDims::new(100, 100, 1);
        for framing in [Framing::Maximum, Framing::CenterOfGravity] {
            let error = output_geometry(framing, dims, &[Some(Transform::identity())]);
            assert!(
                matches!(error, Err(ImageProcError::Unsupported(_))),
                "{framing:?}"
            );
        }
    }

    #[test]
    fn frames_with_no_common_area_are_an_error() {
        let dims = ImageDims::new(100, 100, 1);
        let transforms = vec![
            Some(Transform::identity()),
            Some(Transform::shift(500.0, 0.0)),
        ];
        assert!(output_geometry(Framing::Minimum, dims, &transforms).is_err());
    }

    #[test]
    fn a_projective_transform_is_handled_by_minimum_framing() {
        let dims = ImageDims::new(1000, 800, 1);
        let projective =
            Transform::from_matrix(Matrix3::new(1.0, 0.0, 5.0, 0.0, 1.0, 3.0, 1e-5, 2e-6, 1.0))
                .expect("finite");
        let geometry = output_geometry(Framing::Minimum, dims, &[Some(projective)]).expect("roi");
        assert!(geometry.dims.width > 0 && geometry.dims.height > 0);
    }
}
