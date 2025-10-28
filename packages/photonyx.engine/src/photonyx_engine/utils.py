import pathlib

from astropy.io import fits
from astropy.io.fits import PrimaryHDU
from async_siril.command_types import fits_extension


def all_color_raw_frames(raw_folder: pathlib.Path, extension: fits_extension) -> bool:
    raw_files = list(raw_folder.glob(f"*.{extension.value}"))
    result = all(is_color_frame(raw_file) for raw_file in raw_files)
    return result


def is_color_frame(file: pathlib.Path) -> bool:
    with fits.open(file, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        has_bayer = "BAYERPAT" in primary_hdu.header and primary_hdu.header["BAYERPAT"] != ""
        three_dim = "NAXIS" in primary_hdu.header and primary_hdu.header["NAXIS"] == 3
        return has_bayer or three_dim
