//! Ratio-corrected demosaicing (ADR 007, Phase 6c).
//!
//! # Provenance
//!
//! RCD has no algorithm paper. The reference implementation is in librtprocess
//! and is GPL-3.0, so it cannot be read by anyone writing this file.
//!
//! This module is therefore implemented under a **clean-room separation**: the
//! project owner read the reference and wrote an implementation-independent
//! functional specification (`librtprocess-spec/spec_demosiac_rcd.md`,
//! 2026-09-18), describing behaviour, data flow and constants but no code,
//! no identifiers and no control flow. This file was written from that
//! document alone. The author of this file has never opened `rcd.cc` or any
//! other librtprocess source, and the specification contains no expression
//! from it.
//!
//! That is the established clean-room method, and the reason it matters here
//! is that photonyx is permissively licensed: a structure-preserving
//! translation of GPL source would be a derivative work, while an independent
//! implementation of the same *method* is not. Algorithms are not copyrightable;
//! expression is.
//!
//! Validation is black-box only — output compared against Siril's RCD, never
//! coefficients fitted to it. See `tests/debayer_parity.rs`.
//!
//! # The method
//!
//! Two ideas distinguish RCD from the linear and homogeneity-directed methods
//! in its neighbours [`super::mhc`] and [`super::ahd`].
//!
//! **Ratio correction.** Most methods estimate green at a red site as
//! `neighbour_green + (centre_red − neighbour_red)`, a *difference* model that
//! assumes chroma is locally constant. RCD instead scales the neighbour by the
//! ratio of local panchromatic levels, assuming *relative* chroma is locally
//! constant. That degrades more gracefully across illumination changes, in
//! shadows, and near clipping.
//!
//! **Soft two-axis discrimination.** For each axis pair — vertical/horizontal
//! for green, the two diagonals for chroma — a discrimination ratio in `[0, 1]`
//! is formed from directional high-pass energies, then refined by comparing the
//! pixel's own ratio against its four diagonal neighbours' and keeping whichever
//! is further from the neutral half. The two directional estimates are then
//! *interpolated* using that ratio: a soft decision, never a hard switch. This
//! is the same problem AHD solves with 3×3 homogeneity pooling, solved with two
//! comparisons instead — and being soft, it is what makes RCD quiet where AHD
//! is not.

use rayon::prelude::*;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;

/// Side of the square working tile.
const TILE: usize = 194;

/// Tile overlap, and the frame margin handed to the border fill. Must exceed
/// the largest kernel reach: four samples for the green gradients, plus the
/// high-pass reach.
const MARGIN: usize = 9;

/// Rows and columns of a tile actually written back.
const INTERIOR: usize = TILE - 2 * MARGIN;

/// Division guard on every gradient. Also sets how far a perfectly flat
/// direction is allowed to dominate its opposite.
const EPS_GRAD: f32 = 1e-5;

/// Floor on the discrimination energies, which are squared and so can underflow
/// much further than a gradient.
const EPS_ENERGY: f32 = 1e-10;

/// Directional high-pass whose energy drives the discrimination ratios.
///
/// The specification describes this as "a five-tap symmetric high-pass filter"
/// that "combines the samples at distances 1, 2 and 3 with alternating signs so
/// that it responds to structure at the CFA's own sampling rate while
/// cancelling smooth gradients". Those two clauses name different filters: a
/// five-tap symmetric filter reaches 2, not 3. This is the fourth difference,
/// which satisfies every stated property except the reach — it alternates sign,
/// annihilates any cubic (so all smooth gradients), and peaks exactly at the
/// sampling rate. The sixth difference `[1, -6, 15, -20, 15, -6, 1]` satisfies
/// the reach instead.
///
/// Swapping the two is a one-line change and `HP_REACH` follows automatically.
/// It was **not** chosen by fitting against Siril's output; see the provenance
/// note above.
const HIGHPASS: [f32; 5] = [1.0, -4.0, 6.0, -4.0, 1.0];

/// How far [`HIGHPASS`] reaches either side of centre.
const HP_REACH: usize = HIGHPASS.len() / 2;

/// How far in from the tile edge each pass can produce valid output.
///
/// The passes form a dependency chain and each one reads beyond the region the
/// next one needs, so they do *not* all run over the writeback interior — a
/// pass that stopped there would leave the following pass reading samples it
/// never wrote. Working backwards from the interior at `MARGIN`:
///
/// - chroma at green sites reads chroma at ±3 and green at ±2 → `MARGIN`
/// - chroma at chroma sites reads raw at ±3 diagonally, green at ±2 → `MARGIN - 3`
/// - green reads raw at ±4, levels at ±2, and the axis map at ±1 → `MARGIN - 5`
/// - levels read raw at ±1, the maps reach `HP_REACH + 1`
///
/// The raw reach of 4 under a green inset of 4 is what fixes `MARGIN` at 9: one
/// less and the green pass would run off the tile.
const INSET_MAPS: usize = HP_REACH + 1;
const INSET_LEVEL: usize = 1;
const INSET_GREEN: usize = MARGIN - 5;
const INSET_CHROMA: usize = MARGIN - 3;

/// Demosaics a single-channel CFA frame into planar RGB at full resolution.
///
/// Returns `width * height * 3` samples: all of red, then green, then blue —
/// the layout `Frame` uses.
pub fn demosaic(
    cfa: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
) -> Result<Vec<f32>, ImageProcError> {
    if pattern == BayerPattern::None {
        return Err(ImageProcError::Processing(
            "demosaicing needs a Bayer pattern".to_string(),
        ));
    }
    if cfa.len() != width * height {
        return Err(ImageProcError::Processing(format!(
            "CFA buffer has {} samples, expected {}",
            cfa.len(),
            width * height
        )));
    }
    let min_side = 2 * MARGIN + 1;
    if width < min_side || height < min_side {
        return Err(ImageProcError::Processing(format!(
            "frame is {width}x{height}; demosaicing needs at least {min_side}x{min_side}"
        )));
    }

    // The 9-pixel border has no room for the kernels; bilinear fills it, as it
    // does for the other full-resolution methods here. The specification's
    // shared edge fill is a 3x3 colour average, which is what bilinear already
    // is.
    let mut out = super::bilinear::demosaic(cfa, width, height, pattern)?;

    let plane = width * height;
    let interior = MARGIN * width..(height - MARGIN) * width;
    let (red, rest) = out.split_at_mut(plane);
    let (green, blue) = rest.split_at_mut(plane);

    // Bands of `INTERIOR` rows, so each band's read region is exactly one tile
    // tall and the output rows of different bands never overlap. Column tiling
    // happens inside.
    red[interior.clone()]
        .par_chunks_mut(INTERIOR * width)
        .zip(green[interior.clone()].par_chunks_mut(INTERIOR * width))
        .zip(blue[interior].par_chunks_mut(INTERIOR * width))
        .enumerate()
        .for_each(|(band, ((r_rows, g_rows), b_rows))| {
            let y0 = MARGIN + band * INTERIOR;
            let y1 = y0 + r_rows.len() / width;
            let mut tile = Tile::default();
            let mut x0 = MARGIN;
            while x0 < width - MARGIN {
                let x1 = (x0 + INTERIOR).min(width - MARGIN);
                tile.run(
                    cfa, width, height, pattern, y0, y1, x0, x1, r_rows, g_rows, b_rows,
                );
                x0 = x1;
            }
        });

    Ok(out)
}

/// The tile's read region in frame coordinates, and its size.
#[derive(Clone, Copy)]
struct Region {
    /// Frame row and column of the tile's top-left sample.
    gy0: usize,
    gx0: usize,
    /// Tile height and width, in samples.
    th: usize,
    tw: usize,
}

impl Region {
    /// Colour of the CFA site at tile-local `(lx, ly)`.
    #[inline]
    fn colour_at(&self, pattern: BayerPattern, lx: usize, ly: usize) -> usize {
        pattern.colour_at(self.gx0 + lx, self.gy0 + ly)
    }
}

/// Per-tile scratch, reused across the tiles of one band so the allocation
/// happens once per thread rather than once per tile.
#[derive(Default)]
struct Tile {
    /// The clamped mosaic. Kept distinct from `green` because the green pass
    /// reads raw samples two and four steps out, which it is simultaneously
    /// overwriting.
    raw: Vec<f32>,
    green: Vec<f32>,
    red: Vec<f32>,
    blue: Vec<f32>,
    /// Vertical/horizontal discrimination, full resolution.
    vh: Vec<f32>,
    /// Diagonal discrimination, meaningful only at non-green sites.
    diag: Vec<f32>,
    /// Panchromatic low-pass level, meaningful only at non-green sites.
    level: Vec<f32>,
}

impl Tile {
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        cfa: &[f32],
        width: usize,
        height: usize,
        pattern: BayerPattern,
        y0: usize,
        y1: usize,
        x0: usize,
        x1: usize,
        r_rows: &mut [f32],
        g_rows: &mut [f32],
        b_rows: &mut [f32],
    ) {
        // Read region: the output rectangle grown by MARGIN. Output starts at
        // MARGIN and ends MARGIN short of the frame, so this is always inside.
        let (gy0, gx0) = (y0 - MARGIN, x0 - MARGIN);
        let (th, tw) = ((y1 + MARGIN) - gy0, (x1 + MARGIN) - gx0);
        debug_assert!(gy0 + th <= height && gx0 + tw <= width);

        let region = Region { gy0, gx0, th, tw };
        self.load(cfa, width, pattern, region);
        self.discriminate_axes(region);
        self.levels(pattern, region);
        self.green_at_chroma_sites(pattern, region);
        self.discriminate_diagonals(pattern, region);
        self.chroma_at_chroma_sites(pattern, region);
        self.chroma_at_green_sites(pattern, region);

        // Writeback, clamped at zero from below and not from above: the input
        // clamp already bounded the highlights.
        for y in y0..y1 {
            let src = (y - gy0) * tw - gx0;
            let dst = (y - y0) * width;
            for x in x0..x1 {
                r_rows[dst + x] = self.red[src + x].max(0.0);
                g_rows[dst + x] = self.green[src + x].max(0.0);
                b_rows[dst + x] = self.blue[src + x].max(0.0);
            }
        }
    }

    /// §3.1 — load the tile, clamped to `[0, 1]`.
    ///
    /// The frame convention here is already `[0, 1]`, so the specification's
    /// divide-by-full-scale is the identity and only the clamp remains. That
    /// clamp is RCD's highlight handling: a clipped highlight becomes exactly
    /// one and the ratio correction degenerates to unit scale, which is what
    /// stops an unbounded ratio from producing coloured fringing. It also
    /// clamps genuine negatives — calibrated astro frames contain a few, from
    /// hot pixels in the master dark — to zero. Siril's RCD does the same, so
    /// the reference frame carries it too.
    ///
    /// Each sample is written into every plane its row can carry, so later
    /// passes read a plane without first testing the phase. Green therefore
    /// starts out holding the whole mosaic.
    fn load(&mut self, cfa: &[f32], width: usize, pattern: BayerPattern, region: Region) {
        let Region { gy0, gx0, th, tw } = region;
        let n = th * tw;
        self.raw.clear();
        self.raw.resize(n, 0.0);
        self.red.clear();
        self.red.resize(n, 0.0);
        self.blue.clear();
        self.blue.resize(n, 0.0);
        self.vh.clear();
        self.vh.resize(n, 0.5);
        self.diag.clear();
        self.diag.resize(n, 0.5);
        self.level.clear();
        self.level.resize(n, 0.0);

        for ly in 0..th {
            let gy = gy0 + ly;
            // A row carries red if either of its two phases is red, and so on.
            let (c0, c1) = (pattern.colour_at(0, gy), pattern.colour_at(1, gy));
            let has_red = c0 == 0 || c1 == 0;
            let has_blue = c0 == 2 || c1 == 2;
            for lx in 0..tw {
                let v = cfa[gy * width + gx0 + lx].clamp(0.0, 1.0);
                let i = ly * tw + lx;
                self.raw[i] = v;
                if has_red {
                    self.red[i] = v;
                }
                if has_blue {
                    self.blue[i] = v;
                }
            }
        }
        self.green.clear();
        self.green.extend_from_slice(&self.raw);
    }

    /// §3.2 — vertical/horizontal discrimination, at every interior position.
    ///
    /// The ratio is near zero where the mosaic is smooth vertically (so
    /// vertical interpolation is the right one) and near one in the opposite
    /// case.
    fn discriminate_axes(&mut self, region: Region) {
        let Region { th, tw, .. } = region;
        let edge = INSET_MAPS;
        for ly in edge..th - edge {
            for lx in edge..tw - edge {
                let i = ly * tw + lx;
                // Energy is summed over three positions *perpendicular* to the
                // axis being measured, which stabilises the ratio without
                // blurring along the direction it is deciding about.
                let mut ev = 0.0;
                let mut eh = 0.0;
                for t in -1isize..=1 {
                    ev += sq(highpass(&self.raw, (i as isize + t) as usize, tw));
                    eh += sq(highpass(
                        &self.raw,
                        (i as isize + t * tw as isize) as usize,
                        1,
                    ));
                }
                let (ev, eh) = (ev.max(EPS_ENERGY), eh.max(EPS_ENERGY));
                self.vh[i] = ev / (ev + eh);
            }
        }
    }

    /// §3.3 — panchromatic low-pass level at non-green sites.
    ///
    /// Deliberately mixed-colour: it is the denominator of the ratio correction
    /// and must behave identically in all three channels, so it must not be
    /// per-channel. Unnormalised, because it only ever appears as a ratio.
    fn levels(&mut self, pattern: BayerPattern, region: Region) {
        let Region { th, tw, .. } = region;
        for ly in INSET_LEVEL..th - INSET_LEVEL {
            for lx in INSET_LEVEL..tw - INSET_LEVEL {
                if region.colour_at(pattern, lx, ly) == 1 {
                    continue;
                }
                let i = ly * tw + lx;
                let r = &self.raw;
                self.level[i] = r[i]
                    + 0.5 * (r[i - tw] + r[i + tw] + r[i - 1] + r[i + 1])
                    + 0.25 * (r[i - tw - 1] + r[i - tw + 1] + r[i + tw - 1] + r[i + tw + 1]);
            }
        }
    }

    /// §3.4 — green at red and blue sites.
    fn green_at_chroma_sites(&mut self, pattern: BayerPattern, region: Region) {
        let Region { th, tw, .. } = region;
        let s = tw as isize;
        for ly in INSET_GREEN..th - INSET_GREEN {
            for lx in INSET_GREEN..tw - INSET_GREEN {
                if region.colour_at(pattern, lx, ly) == 1 {
                    continue;
                }
                let i = ly * tw + lx;
                let r = &self.raw;
                let c = r[i];

                // Each direction's gradient reaches four samples: the two
                // greens straddling the centre (shared by an opposing pair),
                // the same-colour sample two steps out, and two further
                // differences at three and four steps.
                let straddle_v = (r[i - tw] - r[i + tw]).abs();
                let straddle_h = (r[i - 1] - r[i + 1]).abs();
                let grad = |straddle: f32, step: isize| {
                    let at = |k: isize| r[(i as isize + k * step) as usize];
                    EPS_GRAD
                        + straddle
                        + (c - at(2)).abs()
                        + (at(1) - at(3)).abs()
                        + (at(2) - at(4)).abs()
                };
                let (gn, gs) = (grad(straddle_v, -s), grad(straddle_v, s));
                let (gw, ge) = (grad(straddle_h, -1), grad(straddle_h, 1));

                // Ratio correction: the green one step out, scaled by the
                // panchromatic level at the centre against the level at the
                // same-colour site two steps out. Equal levels give unit scale.
                let lvl = self.level[i];
                let est = |step: isize| {
                    let at = |k: isize| (i as isize + k * step) as usize;
                    let far = self.level[at(2)];
                    r[at(1)] * (2.0 * lvl / (lvl + far).max(EPS_GRAD))
                };
                let (en, es) = (est(-s), est(s));
                let (ew, ee) = (est(-1), est(1));

                // Weight each direction by its *opposing* gradient, so the
                // smoother side dominates.
                let vertical = (en * gs + es * gn) / (gn + gs);
                let horizontal = (ew * ge + ee * gw) / (gw + ge);

                let ratio = refine(self.vh[i], diagonal_mean(&self.vh, i, tw));
                self.green[i] = vertical + ratio * (horizontal - vertical);
            }
        }
    }

    /// §3.5 — diagonal discrimination on the non-green lattice.
    fn discriminate_diagonals(&mut self, pattern: BayerPattern, region: Region) {
        let Region { th, tw, .. } = region;
        let edge = INSET_MAPS;
        let (nwse, nesw) = (tw + 1, tw - 1);
        for ly in edge..th - edge {
            for lx in edge..tw - edge {
                if region.colour_at(pattern, lx, ly) == 1 {
                    continue;
                }
                let i = ly * tw + lx;
                let mut ea = 0.0;
                let mut eb = 0.0;
                for t in -1isize..=1 {
                    ea += sq(highpass(
                        &self.raw,
                        (i as isize + t * nesw as isize) as usize,
                        nwse,
                    ));
                    eb += sq(highpass(
                        &self.raw,
                        (i as isize + t * nwse as isize) as usize,
                        nesw,
                    ));
                }
                let (ea, eb) = (ea.max(EPS_ENERGY), eb.max(EPS_ENERGY));
                self.diag[i] = ea / (ea + eb);
            }
        }
    }

    /// §3.6 — red at blue sites and blue at red sites.
    ///
    /// A difference model, not a ratio one: ratio correction is used for green
    /// only.
    fn chroma_at_chroma_sites(&mut self, pattern: BayerPattern, region: Region) {
        let Region { th, tw, .. } = region;
        let (nwse, nesw) = ((tw + 1) as isize, (tw - 1) as isize);
        for ly in INSET_CHROMA..th - INSET_CHROMA {
            for lx in INSET_CHROMA..tw - INSET_CHROMA {
                let measured = region.colour_at(pattern, lx, ly);
                if measured == 1 {
                    continue;
                }
                let i = ly * tw + lx;
                let (raw, g) = (&self.raw, &self.green);

                // Opposing corners of one diagonal share their straddle term.
                let straddle = |d: isize| {
                    (raw[(i as isize - d) as usize] - raw[(i as isize + d) as usize]).abs()
                };
                let (st_a, st_b) = (straddle(nwse), straddle(nesw));
                let grad = |st: f32, step: isize| {
                    let at = |k: isize| (i as isize + k * step) as usize;
                    EPS_GRAD + st + (raw[at(1)] - raw[at(3)]).abs() + (g[i] - g[at(2)]).abs()
                };
                let (g_nw, g_se) = (grad(st_a, -nwse), grad(st_a, nwse));
                let (g_ne, g_sw) = (grad(st_b, -nesw), grad(st_b, nesw));

                // Colour difference carried by each diagonal neighbour.
                let cd = |k: isize| {
                    let j = (i as isize + k) as usize;
                    raw[j] - g[j]
                };
                let (nw, se) = (cd(-nwse), cd(nwse));
                let (ne, sw) = (cd(-nesw), cd(nesw));

                let diag_a = (nw * g_se + se * g_nw) / (g_nw + g_se);
                let diag_b = (ne * g_sw + sw * g_ne) / (g_ne + g_sw);

                let ratio = refine(self.diag[i], diagonal_mean(&self.diag, i, tw));
                let value = g[i] + diag_a + ratio * (diag_b - diag_a);
                if measured == 0 {
                    self.blue[i] = value;
                } else {
                    self.red[i] = value;
                }
            }
        }
    }

    /// §3.7 — red and blue at green sites.
    ///
    /// Runs last because it reads chroma at its cardinal neighbours, which are
    /// non-green sites and so are only complete once §3.6 has filled them.
    fn chroma_at_green_sites(&mut self, pattern: BayerPattern, region: Region) {
        let Region { th, tw, .. } = region;
        let s = tw as isize;
        for ly in MARGIN..th - MARGIN {
            for lx in MARGIN..tw - MARGIN {
                if region.colour_at(pattern, lx, ly) != 1 {
                    continue;
                }
                let i = ly * tw + lx;
                let ratio = refine(self.vh[i], diagonal_mean(&self.vh, i, tw));
                let g = &self.green;

                for channel in [0usize, 2] {
                    let p: &[f32] = if channel == 0 { &self.red } else { &self.blue };

                    let straddle_v = (p[i - tw] - p[i + tw]).abs();
                    let straddle_h = (p[i - 1] - p[i + 1]).abs();
                    let grad = |st: f32, step: isize| {
                        let at = |k: isize| (i as isize + k * step) as usize;
                        EPS_GRAD + (g[i] - g[at(2)]).abs() + st + (p[at(1)] - p[at(3)]).abs()
                    };
                    let (gn, gs) = (grad(straddle_v, -s), grad(straddle_v, s));
                    let (gw, ge) = (grad(straddle_h, -1), grad(straddle_h, 1));

                    let cd = |k: isize| {
                        let j = (i as isize + k) as usize;
                        p[j] - g[j]
                    };
                    let vertical = (cd(-s) * gs + cd(s) * gn) / (gn + gs);
                    let horizontal = (cd(-1) * ge + cd(1) * gw) / (gw + ge);

                    let value = g[i] + vertical + ratio * (horizontal - vertical);
                    if channel == 0 {
                        self.red[i] = value;
                    } else {
                        self.blue[i] = value;
                    }
                }
            }
        }
    }
}

/// [`HIGHPASS`] applied at `i` with the given step, which is `1` for
/// horizontal, the row stride for vertical, and stride ± 1 for the diagonals.
#[inline]
fn highpass(data: &[f32], i: usize, step: usize) -> f32 {
    let mut sum = 0.0;
    for (k, w) in HIGHPASS.iter().enumerate() {
        let offset = (k as isize - HP_REACH as isize) * step as isize;
        sum += w * data[(i as isize + offset) as usize];
    }
    sum
}

#[inline]
fn sq(v: f32) -> f32 {
    v * v
}

/// Mean of a map over the four diagonal neighbours.
#[inline]
fn diagonal_mean(map: &[f32], i: usize, tw: usize) -> f32 {
    0.25 * (map[i - tw - 1] + map[i - tw + 1] + map[i + tw - 1] + map[i + tw + 1])
}

/// The refinement rule: keep whichever of the pixel's own discrimination ratio
/// and its neighbourhood's is *more decisive*, meaning further from the neutral
/// half.
///
/// Two comparisons, and they are what stops the direction decision oscillating
/// on noise — the same job AHD needs a 3×3 homogeneity pool to do.
#[inline]
fn refine(centre: f32, neighbourhood: f32) -> f32 {
    if (centre - 0.5).abs() > (neighbourhood - 0.5).abs() {
        centre
    } else {
        neighbourhood
    }
}
