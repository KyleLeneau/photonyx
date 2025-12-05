from __future__ import annotations
import cappa
from astropy.io import fits
from astropy.io.fits import PrimaryHDU
from astropy.stats import sigma_clipped_stats
from rich.table import Table

from ..interface.app import PhotonyxApp
from ..interface.inspect import InspectCommand

IGNORED_KEYS = ['COMMENT', 'HISTORY']

async def invoke(app: PhotonyxApp, command: InspectCommand, output: cappa.Output):
    # Ensure file exists else error
    if not command.file.exists():
        output.error(f"Input file does not exist: '{command.file}'")
        return

    stats_table = Table(title="Sigma-clipped statistics")
    stats_table.add_column("")
    stats_table.add_column("Raw Value")
    stats_table.add_column("ADU Value")
    stats_table.add_column("% of ADU")

    header_table = Table()
    header_table.add_column("Key")
    header_table.add_column("Value")
    header_table.add_column("Comments")

    # Open the file for inspection
    with fits.open(command.file, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        hdr = primary_hdu.header

        # Assuming 'image_data' is already loaded as in the previous example
        data = primary_hdu.data
        mean, median, std = sigma_clipped_stats(data, sigma=3.0)
        data_min = data.min()
        data_max = data.max()

        # If you want to see the equivalent ADU values for reference
        full_well = hdr.get('MAXADU') or hdr.get('SATLEVEL') or 65535

        stats_table.add_row("full well ADU", None, str(full_well), None)
        stats_table.add_row("mean", f"{mean:.4f}", f"{mean * full_well:.0f}", f"{mean * 100:.2f}%")
        stats_table.add_row("median", f"{median:.4f}", f"{median * full_well:.0f}", f"{median * 100:.2f}%")
        stats_table.add_row("std dev", f"{std:.4f}", f"{std * full_well:.0f}", f"{std * 100:.2f}%")
        stats_table.add_row("min", f"{data_min:.4f}", f"{data_min * full_well:.0f}", f"{data_min * 100:.2f}%")
        stats_table.add_row("max", f"{data_max:.4f}", f"{data_max * full_well:.0f}", f"{data_max * 100:.2f}%")

        for k in hdr:
            if k in IGNORED_KEYS:
                continue
            cmt = str(hdr.comments[k])
            header_table.add_row(k, str(hdr.get(k)), f"\\{cmt}" if "[" in cmt else cmt)

    output.output(stats_table)
    output.output(header_table)