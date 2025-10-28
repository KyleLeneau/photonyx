from __future__ import annotations
import cappa
import importlib.metadata

from ..interface.app import PhotonyxApp
from ..interface.scan import ScanCommand


async def invoke(app: PhotonyxApp, command: ScanCommand, output: cappa.Output):
    print(app)
    print(command)
    print(app.global_config)
    output(importlib.metadata.version("photonyx_cli"))
