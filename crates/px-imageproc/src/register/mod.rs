//! Star detection, PSF fitting, matching, and registration (ADR 007,
//! Phases 7-10).
//!
//! The registration pipeline runs in four stages, each a module here:
//! candidate detection ([`starfind`]), profile fitting and star rejection
//! ([`psf`]), correspondence and transform estimation, then warping. Stage
//! boundaries are where the cost changes by an order of magnitude, which is
//! also where the budget scheduler needs to be able to cut.

pub mod framing;
pub mod psf;
pub mod starfind;
pub mod starmatch;
pub mod transform;

pub use framing::{
    FrameQuality, Framing, OutputGeometry, frame_qualities, output_geometry, select_reference,
};
pub use psf::{
    FwhmStats, Profile, PsfStar, Rejected, amplitude_threshold_for, filter_stars_by_amplitude,
    fwhm_stats,
};
pub use starmatch::{MatchOptions, match_stars, pair_by_proximity, register_pair};
pub use transform::{
    Correspondence, Fit, Point, RansacOptions, Transform, TransformKind, estimate, ransac, refine,
};

pub use starfind::{
    Background, Candidate, StarfindOptions, StarfindReport, detect_candidates, find_stars,
    find_stars_reported,
};
