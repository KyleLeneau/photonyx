//! Star-list matching by triangle similarity (ADR 007, P8-T4).
//!
//! Two frames of the same field give two star lists in different coordinate
//! systems, with no shared identifiers: matching them is the problem of finding
//! which star is which before any transform is known. The published solution is
//! Groth, *A Pattern-Matching Algorithm for Two-Dimensional Coordinate Lists*,
//! AJ 91, 1986, as refined by Valdes, Campusano, Velasquez & Stetson, *FOCAS
//! Automatic Catalog Matching Algorithms*, PASP 107, 1995.
//!
//! The idea is to match *triangles* rather than stars. A triangle's shape is
//! invariant under translation, rotation and uniform scale, so a triangle of
//! three stars in one frame can be recognized in the other without knowing the
//! transform between them. Each matched triangle casts three votes — one per
//! vertex pair — and the star pairs that collect the most votes are the
//! correspondences. A single mismatched triangle casts three wrong votes, which
//! the hundreds of right ones outvote; that redundancy is why the method
//! tolerates spurious detections and missing stars.
//!
//! Shape is parameterized as Valdes et al. do: with the sides ordered
//! `a >= b >= c`, the pair `(b/a, c/a)` is a complete scale-free description of
//! the triangle, and the sign of the vertex cross product records its
//! handedness. Handedness is compared, not ignored: a reflected triangle has the
//! same side ratios, and on a field with no reflection — which is every pair of
//! frames from one camera — accepting mirror matches would double the wrong
//! votes for nothing.

use std::collections::HashMap;

use super::psf::PsfStar;
use super::transform::{Correspondence, Fit, Point, RansacOptions, Transform, TransformKind};
use crate::error::ImageProcError;

/// Brightest stars per list that enter the triangle construction.
///
/// The triangle count is cubic in this, so it is the one number that decides
/// what matching costs: 40 stars is 9,880 triangles per list, 80 would be
/// 82,160. Forty brightest stars is ample — the vote margin between the right
/// correspondences and the wrong ones is already hundreds to one — and the
/// faint end of a star list is where the spurious detections live anyway.
pub const DEFAULT_MATCH_STARS: usize = 40;

/// Tolerance on each shape coordinate for two triangles to be called the same.
///
/// A star measured to 0.05 px contributes a relative error of roughly
/// `0.05 / side` to each ratio, so a 50 px triangle carries ~1e-3. At 5e-3 the
/// comparison admits triangles down to a few tens of pixels on a side without
/// letting genuinely different shapes through — and a shape collision is not
/// fatal in any case, it is three votes against hundreds.
pub const DEFAULT_SHAPE_TOLERANCE: f64 = 5e-3;

/// Smallest ratio of shortest to longest side a triangle may have.
///
/// A needle-thin triangle has two nearly coincident vertices as far as the shape
/// coordinates can tell, so its vertex correspondence is ambiguous and its votes
/// are noise. This is the standard FOCAS guard against them.
const MIN_SIDE_RATIO: f64 = 0.1;

/// Votes a star pair needs before it is accepted as a correspondence, as a
/// fraction of the best-supported pair's.
///
/// The threshold has to be *relative*, which is worth spelling out because an
/// absolute floor looks sufficient and is not. Two 40-star lists give 9,880
/// triangles each, and shape space is small enough that unrelated triangles
/// collide within tolerance by chance tens of thousands of times per pair of
/// lists — spread over 1,600 possible star pairs, that is ~18 votes each from
/// pure coincidence. A correct pair, by contrast, is in every triangle it
/// belongs to: C(39, 2) = 741 of them. The two populations are three orders of
/// magnitude apart in support but only a few votes apart in absolute floor
/// terms, so the cut is taken at a fraction of the leader.
const VOTE_FRACTION: f64 = 0.15;

/// Absolute floor, for the sparse-field case where even the leader has few
/// votes.
const MIN_VOTES: u32 = 3;

/// How far the best-supported pair must stand above the middle of the vote
/// distribution for the distribution to be considered a match at all.
///
/// This is the test for *whether* two lists match, as opposed to which stars
/// correspond, and it is needed because a vote matrix always has a leader —
/// even for two unrelated patches of sky. Measured on 40-brightest lists of
/// 300-800 stars:
///
/// | Case | Leader | Median | Ratio |
/// |---|---|---|---|
/// | same field | 897 | 54 | 16.6 |
/// | unrelated fields | 137 | 50 | 2.7 |
/// | reflected field | 164 | 52 | 3.2 |
///
/// The two populations are an order of magnitude apart, so 6 sits in the gap
/// with room either side. Without this test the ranking falls through to its
/// tie-break and returns whichever noise pairs sorted first, which is how a
/// reflected field came back "matched" with a reflecting affine transform.
const SIGNAL_MARGIN: u32 = 6;

/// Vote level, as a multiple of the distribution's median, below which a pair is
/// indistinguishable from a coincidence.
const NOISE_MARGIN: u32 = 2;

/// How close a star must fall to a mapped position to be paired with it during
/// match expansion, in pixels.
///
/// Generous relative to the ~0.05 px accuracy of the seed transform, because
/// what it has to tolerate is not measurement error but the part of the field
/// the seed transform does not yet describe — a projective term fitted from 40
/// bright stars clustered toward the centre can be a pixel out at the edges.
const EXPAND_TOLERANCE: f64 = 2.0;

/// Matching parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchOptions {
    pub max_stars: usize,
    pub shape_tolerance: f64,
    /// Fewest correspondences a match must produce to be considered a match at
    /// all. Siril's registration default is 10 (ADR 007 D10).
    pub min_pairs: usize,
}

impl Default for MatchOptions {
    fn default() -> Self {
        Self {
            max_stars: DEFAULT_MATCH_STARS,
            shape_tolerance: DEFAULT_SHAPE_TOLERANCE,
            min_pairs: 10,
        }
    }
}

/// One triangle: its vertices, ordered so that vertex `i` is opposite the
/// `i`-th longest side, and its scale-free shape.
#[derive(Debug, Clone, Copy)]
struct Triangle {
    /// Star indices, ordered by the length of the side each is opposite to,
    /// longest first. This ordering is what makes a shape match yield a *vertex*
    /// correspondence and not just "these two triangles look alike".
    vertices: [usize; 3],
    /// `b/a` and `c/a` for sides `a >= b >= c`.
    shape: (f64, f64),
    /// Sign of the vertex cross product: +1 counter-clockwise, -1 clockwise.
    handedness: f64,
}

/// Builds every usable triangle of `points`, sorted by the first shape
/// coordinate so matching can binary-search it.
///
/// Two kinds of triangle are discarded. Needle-thin ones, per
/// [`MIN_SIDE_RATIO`]. And ones with two sides of nearly equal length, because
/// the whole scheme rests on *labelling* vertices by the side each is opposite:
/// if two sides agree to within the matching tolerance, their two vertices can
/// be labelled either way round, and such a triangle casts two wrong votes out
/// of three even on a correct shape match. It also votes across a mirror, since
/// swapping the ambiguous pair flips the handedness back — so discarding these
/// is what makes handedness comparison mean anything.
fn build_triangles(points: &[Point], tolerance: f64) -> Vec<Triangle> {
    let n = points.len();
    let mut triangles = Vec::with_capacity(n * (n - 1) * (n - 2) / 6);

    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                // Side lengths, each labelled by the vertex opposite it.
                let mut sides = [
                    (points[j].distance(&points[k]), i),
                    (points[i].distance(&points[k]), j),
                    (points[i].distance(&points[j]), k),
                ];
                sides.sort_by(|x, y| y.0.total_cmp(&x.0));

                let (a, b, c) = (sides[0].0, sides[1].0, sides[2].0);
                if a <= 0.0 || c / a < MIN_SIDE_RATIO {
                    continue;
                }
                // Ambiguous labelling: the side lengths must be separated by
                // more than the tolerance at which shapes are compared.
                if (a - b) / a <= tolerance || (b - c) / a <= tolerance {
                    continue;
                }

                let vertices = [sides[0].1, sides[1].1, sides[2].1];
                let (p0, p1, p2) = (
                    points[vertices[0]],
                    points[vertices[1]],
                    points[vertices[2]],
                );
                let cross = (p1.x - p0.x) * (p2.y - p0.y) - (p1.y - p0.y) * (p2.x - p0.x);
                triangles.push(Triangle {
                    vertices,
                    shape: (b / a, c / a),
                    handedness: if cross >= 0.0 { 1.0 } else { -1.0 },
                });
            }
        }
    }

    triangles.sort_by(|x, y| x.shape.0.total_cmp(&y.shape.0));
    triangles
}

/// Accumulates one vote per vertex pair for every pair of triangles with the
/// same shape and handedness.
///
/// The vote matrix is sparse in practice — of the 1,600 possible star pairs on
/// two 40-star lists, a few hundred ever receive a vote — so it is a map rather
/// than a dense `n × m` array. Siril's FOCAS ancestor allocates the dense
/// matrix; at 2,000 stars that would be four million counters to scan for a
/// maximum, and the sparse form is both smaller and faster to rank.
fn vote(
    reference: &[Triangle],
    target: &[Triangle],
    tolerance: f64,
) -> HashMap<(usize, usize), u32> {
    let mut votes: HashMap<(usize, usize), u32> = HashMap::new();

    for t in target {
        // Both lists are sorted by the first shape coordinate, so only a window
        // of the reference list can match this triangle.
        let low = reference.partition_point(|r| r.shape.0 < t.shape.0 - tolerance);
        for r in &reference[low..] {
            if r.shape.0 > t.shape.0 + tolerance {
                break;
            }
            if (r.shape.1 - t.shape.1).abs() > tolerance || r.handedness != t.handedness {
                continue;
            }
            for v in 0..3 {
                *votes.entry((r.vertices[v], t.vertices[v])).or_insert(0) += 1;
            }
        }
    }
    votes
}

/// Resolves the vote matrix into a one-to-one correspondence list, or an empty
/// one when the distribution carries no signal.
///
/// Pairs are taken in descending vote order, and a pair is accepted only if
/// neither of its stars has already been claimed. That greedy rule is what makes
/// the result one-to-one: a bright star in one frame can otherwise collect votes
/// from several stars in the other, and a transform fitted to a one-to-many
/// correspondence list is fitted to a contradiction.
fn top_vote_getters(votes: HashMap<(usize, usize), u32>) -> Vec<(usize, usize, u32)> {
    let mut ranked: Vec<((usize, usize), u32)> = votes.into_iter().collect();
    // Votes descending, then by index, so an equal-vote tie does not depend on
    // the hash map's iteration order — the whole pipeline has to be
    // reproducible.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let leader = ranked.first().map(|(_, count)| *count).unwrap_or(0);
    // `ranked` is sorted by vote count, so the middle element is the median.
    let median = ranked.get(ranked.len() / 2).map(|(_, c)| *c).unwrap_or(0);
    if leader < SIGNAL_MARGIN * median.max(1) {
        return Vec::new();
    }

    let cut = MIN_VOTES
        .max((VOTE_FRACTION * leader as f64).ceil() as u32)
        .max(NOISE_MARGIN * median);

    let mut used_reference = std::collections::HashSet::new();
    let mut used_target = std::collections::HashSet::new();
    let mut accepted = Vec::new();
    for ((r, t), count) in ranked {
        if count < cut {
            break;
        }
        if used_reference.contains(&r) || used_target.contains(&t) {
            continue;
        }
        used_reference.insert(r);
        used_target.insert(t);
        accepted.push((r, t, count));
    }
    accepted
}

/// Matches two star lists, returning the correspondences in descending vote
/// order.
///
/// `from` is the frame being registered and `to` the reference, matching the
/// direction of [`Correspondence`] and of the transform estimated from it.
pub fn match_stars(
    from: &[PsfStar],
    to: &[PsfStar],
    options: &MatchOptions,
) -> Result<Vec<Correspondence>, ImageProcError> {
    let target = brightest(from, options.max_stars);
    let reference = brightest(to, options.max_stars);
    if target.len() < 3 || reference.len() < 3 {
        return Err(ImageProcError::Registration(format!(
            "need at least 3 stars per frame to form a triangle, got {} and {}",
            target.len(),
            reference.len()
        )));
    }

    let votes = vote(
        &build_triangles(&reference, options.shape_tolerance),
        &build_triangles(&target, options.shape_tolerance),
        options.shape_tolerance,
    );
    let pairs = top_vote_getters(votes);
    if pairs.len() < options.min_pairs {
        return Err(ImageProcError::Registration(format!(
            "matched only {} star pairs, fewer than the {} required",
            pairs.len(),
            options.min_pairs
        )));
    }

    Ok(pairs
        .into_iter()
        .map(|(r, t, _)| Correspondence {
            from: target[t],
            to: reference[r],
        })
        .collect())
}

/// Matches two star lists and fits a transform to the result.
///
/// The whole single-pair registration: triangle matching for the
/// correspondences, then a robust fit with refinement. Kept here rather than in
/// the caller because the two halves share a failure mode — a match that found
/// too few pairs and a fit that found too few inliers are the same problem seen
/// twice — and a caller should get one error either way.
/// Matching runs on the brightest stars only, because the triangle count is
/// cubic; the transform that comes out is therefore fitted to a few dozen
/// correspondences and is no more precise than that allows. So it is used as a
/// *seed*: every star in both full lists is paired by proximity under it, and
/// the transform is refitted on the result. Following the initial match with a
/// proximity pass is FOCAS's own two-stage structure (Valdes et al. 1995, §3),
/// and it is what brings corner accuracy from a tenth of a pixel to a
/// hundredth — precision in a least-squares fit goes as the square root of the
/// correspondence count, and this turns 40 into hundreds.
pub fn register_pair(
    from: &[PsfStar],
    to: &[PsfStar],
    kind: TransformKind,
    match_options: &MatchOptions,
    ransac_options: &RansacOptions,
) -> Result<Fit, ImageProcError> {
    let seed_pairs = match_stars(from, to, match_options)?;
    let seed = super::transform::refine(kind, &seed_pairs, ransac_options)?;

    let expanded = pair_by_proximity(from, to, &seed.transform, EXPAND_TOLERANCE);
    if expanded.len() <= seed_pairs.len() {
        return Ok(seed);
    }
    // A refit on more correspondences is only an improvement if it actually
    // fits; a failure here leaves the seed, which is already a valid answer.
    match super::transform::refine(kind, &expanded, ransac_options) {
        Ok(fit) => Ok(fit),
        Err(_) => Ok(seed),
    }
}

/// Pairs every star in `from` with the nearest star in `to` to its mapped
/// position, within `tolerance` pixels, one-to-one.
///
/// Nearest-neighbour lookup is over a uniform grid of cell size `tolerance`, so
/// the pass is linear in the star counts rather than quadratic: the lists run to
/// two thousand stars each, and this is called once per frame of a hundred-frame
/// sequence.
pub fn pair_by_proximity(
    from: &[PsfStar],
    to: &[PsfStar],
    transform: &Transform,
    tolerance: f64,
) -> Vec<Correspondence> {
    let cell = |p: &Point| {
        (
            (p.x / tolerance).floor() as i64,
            (p.y / tolerance).floor() as i64,
        )
    };

    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, star) in to.iter().enumerate() {
        grid.entry(cell(&Point::new(star.x0, star.y0)))
            .or_default()
            .push(i);
    }

    // Candidate pairs with their distances, then resolved closest-first so that
    // two `from` stars competing for one `to` star are settled by distance
    // rather than by list order.
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (i, star) in from.iter().enumerate() {
        let mapped = transform.apply(Point::new(star.x0, star.y0));
        if !mapped.x.is_finite() || !mapped.y.is_finite() {
            continue;
        }
        let (cx, cy) = cell(&mapped);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &j in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let distance = mapped.distance(&Point::new(to[j].x0, to[j].y0));
                    if distance <= tolerance && best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, j));
                    }
                }
            }
        }
        if let Some((distance, j)) = best {
            candidates.push((distance, i, j));
        }
    }

    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1, a.2).cmp(&(b.1, b.2)))
    });
    let mut used_to = std::collections::HashSet::new();
    let mut pairs = Vec::new();
    for (_, i, j) in candidates {
        if used_to.insert(j) {
            pairs.push(Correspondence {
                from: Point::new(from[i].x0, from[i].y0),
                to: Point::new(to[j].x0, to[j].y0),
            });
        }
    }
    pairs
}

/// The positions of the `count` brightest stars, in descending amplitude order.
///
/// Amplitude rather than list order: `find_stars` returns stars brightest-first
/// already, but a caller that filtered or concatenated lists may not have, and
/// the matcher's cost depends on taking the genuinely brightest.
fn brightest(stars: &[PsfStar], count: usize) -> Vec<Point> {
    let mut ordered: Vec<&PsfStar> = stars.iter().collect();
    ordered.sort_by(|a, b| {
        b.amplitude
            .total_cmp(&a.amplitude)
            .then_with(|| (a.x0, a.y0).partial_cmp(&(b.x0, b.y0)).unwrap())
    });
    ordered
        .into_iter()
        .take(count)
        .map(|s| Point::new(s.x0, s.y0))
        .collect()
}
