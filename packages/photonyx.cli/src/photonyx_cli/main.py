from __future__ import annotations

import cappa
import asyncio
import structlog

from .interface.app import PhotonyxApp


log = structlog.get_logger()


def main() -> None:  # pragma: no cover
    try:
        # # Ensure app directory exists
        # setup_app_dir()

        # # Load or create global config
        # global_config = load_or_create_global_config()

        # # Setup logging
        # setup_logging(global_config.logging)
        # log.debug("Global configuration loaded", config=global_config)

        # app_state = cappa.State[AppState]({"global_config": global_config})
        asyncio.run(cappa.invoke_async(PhotonyxApp))
    except Exception as e:
        log.exception("Unhandled exception")
        raise cappa.Exit("There was an error while executing", code=-1) from e


if __name__ == "__main__":
    main()
