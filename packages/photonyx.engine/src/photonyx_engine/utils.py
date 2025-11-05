import pathlib
import typing as t

from astropy.io import fits
from astropy.io.fits import PrimaryHDU
from datetime import datetime


def header_value(hdu: PrimaryHDU, key: str, default: t.Any) -> t.Any:
    return hdu.header[key] if key in hdu.header else default


def all_fits_files(raw_folder: pathlib.Path) -> t.List[pathlib.Path]:
    extensions = ["fit", "fits"]
    files: t.List[pathlib.Path] = []
    for ext in extensions:
        files.extend(raw_folder.glob(f"*.{ext}"))
    return files


def first_file(raw_folder: pathlib.Path) -> t.Optional[pathlib.Path]:
    _sorted = sorted(all_fits_files(raw_folder), key=lambda p: p.name)
    return _sorted[0] if len(_sorted) > 0 else None


def all_color_raw_frames(raw_folder: pathlib.Path) -> bool:
    raw_files = all_fits_files(raw_folder)
    result = all(is_color_frame(raw_file) for raw_file in raw_files)
    return result


def is_color_frame(file: pathlib.Path) -> bool:
    with fits.open(file, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        has_bayer = "BAYERPAT" in primary_hdu.header and primary_hdu.header["BAYERPAT"] != ""
        three_dim = "NAXIS" in primary_hdu.header and primary_hdu.header["NAXIS"] == 3
        return has_bayer or three_dim


def first_observation_date(raw_folder: pathlib.Path) -> datetime:
    first = first_file(raw_folder)
    if first is None:
        return datetime.now()

    with fits.open(first, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        date_loc = header_value(primary_hdu, "DATE-OBS", "")
        if len(date_loc) > 0:
            return datetime.fromisoformat(date_loc)
        else:
            return datetime.now()


def first_observation_date_exp_temp(raw_folder: pathlib.Path) -> t.Tuple[datetime, float, float]:
    first = first_file(raw_folder)
    if first is None:
        return datetime.now(), 0, 0

    with fits.open(first, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        date_loc = header_value(primary_hdu, "DATE-OBS", "")
        date = datetime.fromisoformat(date_loc) if len(date_loc) > 0 else datetime.now()
        exp = header_value(primary_hdu, "EXPOSURE", 0)
        temp = header_value(primary_hdu, "SET-TEMP", 0)
        return date, exp, temp


def first_observation_date_filter(raw_folder: pathlib.Path) -> t.Tuple[datetime, str]:
    first = first_file(raw_folder)
    if first is None:
        return datetime.now(), ""

    with fits.open(first, ignore_missing_simple=True) as hdu_list:
        primary_hdu = next(h for h in hdu_list if isinstance(h, PrimaryHDU))
        date_loc = header_value(primary_hdu, "DATE-OBS", "")
        date = datetime.fromisoformat(date_loc) if len(date_loc) > 0 else datetime.now()
        _filter = header_value(primary_hdu, "FILTER", "")
        return date, _filter
