# px-imageproc

Pixel-space image processing for FITS data, built on top of [`px-fits`](../px-fits).

`px-fits` reads and writes FITS bytes and has no opinion on what the pixels mean
photographically. This crate owns that interpretation: debayering, display
normalization, autostretch, and — planned — calibration, stacking, and composition.

Scaffolded as part of ADR 006 Phase 9. See
[`docs/adr/006-native-fits-implementation.md`](../../docs/adr/006-native-fits-implementation.md)
for the phased plan and rationale for the crate split.
