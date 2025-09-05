# Global App configuration
# includes:
# - Application settings
# - User preferences
# - Logging configuration
# - File paths and directories
# - Siril integration settings
# - System processing settings & limits

# file placed at: ~/.px/px_global.yaml

from __future__ import annotations

import pathlib

from pydantic import BaseModel, Field

from ..utils.logging import LogLevels


GLOBAL_CONFIG_DIR = pathlib.Path.home() / ".px"
GLOBAL_CONFIG_FILE_PATH = GLOBAL_CONFIG_DIR / "px_global.yaml"


class LoggingConfig(BaseModel):
    """Logging configuration for the Photonyx application."""

    verbosity: int = 0
    log_levels: dict[str, LogLevels] = Field(
        default_factory=lambda: {
            "photonyx_cli": LogLevels.INFO,
        },
        frozen=True,
    )
    _testing: bool = False


class GlobalAppConfig(BaseModel):
    """Global photonyx application configuration."""

    # Logging configuration
    logging: LoggingConfig = LoggingConfig()

    # User preferences
    profile_home_directory: pathlib.Path = pathlib.Path.home() / "Pictures" / "Astro"
