# Configuration about a project composition from sessions
# includes:
# - Project details for stacking
# - Project type for framing (mosiac)
# - inclusion of PP_ folders for stacking
# -

# file placed at: {profile_dir}/PROJECTS/{PROJECT_NAME}/px_project.yaml

from __future__ import annotations

import pathlib

from pydantic import BaseModel

PROJECT_CONFIG_FILE_NAME = "px_project.yaml"


class SessionReferenceConfig(BaseModel):
    """Configuration for a session to include in the project"""

    session_folder: pathlib.Path
    filter_key: str


class ProjectConfig(BaseModel):
    """Configuration for a project or 1 or more target sessions."""

    sessions: list[SessionReferenceConfig] = []

    def validate(self, folder: pathlib.Path):
        """Validate that all the session configs exist"""
        for ses in self.sessions:
            raw_dir = folder / ses.session_folder

            if not raw_dir.exists():
                raise FileNotFoundError(f"session folder not found: '{raw_dir}'")
            ses.session_folder = raw_dir
