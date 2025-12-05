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

import pathlib
import datetime
import typing as t
import pydash as py_

from .session import ExposureConfig
from pydantic import BaseModel


PROFILE_CONFIG_FILE_NAME = "px_profile.yaml"


class FilterInfo(BaseModel):
    """Information about a single filter."""

    name: str
    key: str
    description: str | None = None


class CalibrationMasterFlat(BaseModel):
    """Information about a flat calibration master file."""

    date: datetime.date
    filter_key: str
    temperature: float
    file: pathlib.Path


class CalibrationMasterBias(BaseModel):
    """Information about a bias calibration master file."""

    date: datetime.date
    file: pathlib.Path


class CalibrationMasterDark(BaseModel):
    """Information about a dark calibration master file."""

    date: datetime.date
    exposure: float
    temperature: float
    file: pathlib.Path


class CalibrationMastersConfig(BaseModel):
    """References to calibration master files."""

    bias: list[CalibrationMasterBias] = []
    dark: list[CalibrationMasterDark] = []
    flat: list[CalibrationMasterFlat] = []

    def _resolve(self, profile_home: pathlib.Path):
        """Resolve that all the master files exist"""
        for master in self.flat + self.bias + self.dark:
            file_path = profile_home / master.file

            if not file_path.exists():
                raise FileNotFoundError(f"Calibration master file not found: '{file_path}'")

            # Update the file_path to be absolute in memory
            master.file = file_path


class ResolvedCalibrationMasters(BaseModel):
    bias: t.Optional[CalibrationMasterBias] = None
    dark: t.Optional[CalibrationMasterDark] = None
    flat: t.Optional[CalibrationMasterFlat] = None


class ProfileConfig(BaseModel):
    """Configuration for a single telescope profile."""

    # Full path to the profile home directory
    profile_home: pathlib.Path

    # Telescope specifications
    telescope_name: str
    aperture: float
    focal_length: float

    # Filter inventory
    filters: list[FilterInfo] = []

    # Store for calibration masters to reference
    calibration_masters: CalibrationMastersConfig = CalibrationMastersConfig()

    @property
    def home(self) -> pathlib.Path:
        return self.profile_home.expanduser().resolve()

    @property
    def bias(self) -> pathlib.Path:
        return self.home / "BIAS"

    @property
    def dark(self) -> pathlib.Path:
        return self.home / "DARK"

    @property
    def flat(self) -> pathlib.Path:
        return self.home / "FLAT"

    @property
    def light(self) -> pathlib.Path:
        return self.home / "LIGHT"

    @property
    def projects(self) -> pathlib.Path:
        return self.home / "PROJECTS"

    def resolve(self):
        """Resolve the profile configuration."""
        self.profile_home = self.profile_home.expanduser()
        self.calibration_masters._resolve(self.home)

    def resolve_calibration_masters(self, exposure: ExposureConfig) -> ResolvedCalibrationMasters:
        resolved = ResolvedCalibrationMasters()

        # Find the master bias if requested
        if exposure.calibration.bias_date is not None:
            resolved.bias = py_.find(
                self.calibration_masters.bias, lambda bias: bias.date == exposure.calibration.bias_date
            )

        # Find the master dark if requested
        if exposure.calibration.dark_date is not None:
            resolved.dark = py_.find(
                self.calibration_masters.dark,
                lambda dark: dark.date == exposure.calibration.dark_date
                and dark.exposure == exposure.exposure
                and dark.temperature == exposure.temperature,
            )

        # Find the master flat if requested
        if exposure.calibration.flat_date is not None:
            resolved.flat = py_.find(
                self.calibration_masters.flat,
                lambda flat: flat.date == exposure.calibration.flat_date and flat.filter_key == exposure.filter_key,
            )

        return resolved
