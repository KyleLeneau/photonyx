from __future__ import annotations

import asyncio
import glob
import pathlib
from typing import AsyncGenerator


async def watch_files(
    folder_path: str | pathlib.Path,
    interval_seconds: float = 1.0,
    extension: str = ".fit",
) -> AsyncGenerator[pathlib.Path, None]:
    """
    Async generator that yields {extension} files from a folder with configurable delay.

    Args:
        folder_path: Path to the folder to watch for {extension} files
        interval_seconds: Delay in seconds between yielding files
        extension: File extension to watch for

    Yields:
        Path objects for each {extension} file found
    """
    folder = pathlib.Path(folder_path)
    if not folder.exists():
        raise ValueError(f"Folder does not exist: {folder_path}")

    if not folder.is_dir():
        raise ValueError(f"Path is not a directory: {folder_path}")

    pattern = str(folder / f"*{extension}")
    files = glob.glob(pattern)
    for file_path in sorted(files):
        yield pathlib.Path(file_path)
        await asyncio.sleep(interval_seconds)


async def _test_file_watcher():
    """Test the file watcher with a sample directory."""
    test_path = "~/Pictures/Astro/Radian_75_71mc_pro/LIGHT/2025-06-25_NGC_7000_NA_Nebula/RAW_Ultra"

    print(f"Watching for *.fit files in: {test_path}")
    print("Press Ctrl+C to stop\n")

    try:
        count = 0
        async for fit_file in watch_files(test_path, interval_seconds=0.5):
            count += 1
            print(f"Found file {count}: {fit_file}")

    except KeyboardInterrupt:
        print("\nStopped by user")
    except Exception as e:
        print(f"Error: {e}")


if __name__ == "__main__":
    asyncio.run(_test_file_watcher())
