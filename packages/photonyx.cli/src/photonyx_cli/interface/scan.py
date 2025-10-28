from dataclasses import dataclass
import cappa


@cappa.command(name="scan", invoke="photonyx_cli.command.scan.invoke")
@dataclass
class ScanCommand:
    """Scan a Hardware Profile to index all the sets."""
