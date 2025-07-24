from __future__ import annotations
import cappa
import importlib.metadata

from photonyx.main import PhotonyxApp
from photonyx.interface.version import VersionCommand


async def invoke(app: PhotonyxApp, command: VersionCommand, output: cappa.Output):
    print(app)
    print(command)
    output(importlib.metadata.version("photonyx"))
