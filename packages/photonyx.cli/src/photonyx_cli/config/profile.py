# Configuration about a single telescope profile
# includes:
# - Telescope & hardware specifications
# - Camera settings to apply (crop, etc)
# - Image Calibration settings to apply
# - Location information (fixed or mobile)
# - Calibration master references
# - Filter information

# file placed at: {profile_dir}/px_profile.yaml

from __future__ import annotations

from pydantic import BaseModel


class ProfileConfig(BaseModel):
    """Configuration for a single telescope profile."""

    # Telescope specifications
    telescope_name: str
    aperture: float
    focal_length: float
