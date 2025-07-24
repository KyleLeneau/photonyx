from dataclasses import dataclass
import cappa
from .version import VersionCommand


@dataclass
class PhotonyxApp:
    """Photonyx application."""

    commands: cappa.Subcommands[VersionCommand]
