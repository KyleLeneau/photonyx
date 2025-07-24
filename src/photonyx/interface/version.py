from dataclasses import dataclass
import cappa


@cappa.command(name="version", invoke="photonyx.command.version.invoke")
@dataclass
class VersionCommand:
    """Show the version of Photonyx."""
