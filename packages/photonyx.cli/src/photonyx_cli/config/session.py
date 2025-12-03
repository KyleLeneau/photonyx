# Configuration about a single target session
# includes:
# - Mapping between RAW folder name and calibration master to use

# file placed at: {profile_dir}/LIGHT/{TARGET_NAME}/{DATE}/px_session.yaml

from __future__ import annotations

import pathlib
import typing as t
import datetime

from pydantic import BaseModel

SESSION_CONFIG_FILE_NAME = "px_session.yaml"


class CalibrationSettings(BaseModel):
    """User supplied calibration settings for an exposure plan"""

    dark_date: t.Optional[datetime.date] = None
    flat_date: t.Optional[datetime.date] = None
    bias_date: t.Optional[datetime.date] = None


class ExposureConfig(BaseModel):
    """Configuration for an exposure plan for the session"""

    raw_folder: pathlib.Path
    exposure: float
    filter_key: str
    temperature: float

    calibration: CalibrationSettings = CalibrationSettings()

    @property
    def pp_folder_name(self) -> str:
        return f"PP_{self.raw_folder.name.removeprefix('RAW_')}"

    @property
    def pp_folder(self) -> pathlib.Path:
        return self.raw_folder.parent / self.pp_folder_name


class SessionConfig(BaseModel):
    """Configuration for a single target session."""

    exposures: list[ExposureConfig] = []

    def resolve(self, folder: pathlib.Path):
        """Resolve that all the exposure configs exist"""
        for exp in self.exposures:
            raw_dir = folder / exp.raw_folder

            if not raw_dir.exists():
                raise FileNotFoundError(f"RAW session folder not found: '{raw_dir}'")
            exp.raw_folder = raw_dir
