"""Based off https://github.com/hynek/structlog/issues/395#issuecomment-2510234481"""

from __future__ import annotations

import contextlib
import datetime
import enum
import functools
import logging.config
import sys
import typing as t

import attrs as a
import orjson
import structlog.processors
import structlog.tracebacks
import structlog.types

log = structlog.stdlib.get_logger()


class LogLevels(enum.Enum):
    CRITICAL = "CRITICAL"
    DEBUG = "DEBUG"
    ERROR = "ERROR"
    INFO = "INFO"
    WARNING = "WARNING"


class LoggedClass:
    """Mixin class that provides a static property with a
    logger that used the class module and name"""

    __logger: t.Optional[t.Any] = None

    @classmethod
    def get_logger(cls) -> t.Any:
        if cls.__logger is None:
            cls.__logger = structlog.get_logger(f"{cls.__module__}.{cls.__name__}")
        return cls.__logger

    @property
    def logger(self) -> t.Any:
        return self.get_logger()


@a.define(kw_only=True)
class VerboseFilter:
    verbosity: int = 0

    def __call__(
        self,
        logger: structlog.typing.WrappedLogger,
        method_name: str,
        event_dict: structlog.typing.EventDict,
    ) -> structlog.typing.ProcessorReturnValue:
        if "verbosity" in event_dict and event_dict.pop("verbosity") > self.verbosity:
            raise structlog.DropEvent

        return event_dict


def render_orjson(
    dict_tracebacks: structlog.processors.ExceptionRenderer,
    /,
    logger: structlog.types.WrappedLogger,
    name: str,
    event_dict: structlog.types.EventDict,
) -> str:
    """
    Render log records as JSON with ``orjson`` and use dict tracebacks.

    This function must be wrapped with :func:`functools.partial()` and the
    *dict_tracebacks* function has to be passed to it::

        "processor": functools.partial(render_orjson, dict_tracebacks),

    We cannot use :class:`structlog.processors.JSONRenderer` because it returns bytes
    and the :class:`structlog.stdlib.LoggerFactory` needs strings. There is also the
    :class:`structlog.BytesLoggerFactory` but we cannot combine them.

    We also do the dict traceback formatting here, which allows us to simplify the
    stdlib log handler config::

        "()": structlog.stdlib.ProcessorFormatter,
        "processor": functools.partial(render_orjson, dict_tracebacks),

    vs::

        "()": structlog.stdlib.ProcessorFormatter,
        "processors": [
            structlog.stdlib.ProcessorFormatter.remove_processors_meta,
            structlog.processors.dict_tracebacks,
            render_orjson,
        ],
    """

    event_dict = dict_tracebacks(logger, name, event_dict)
    if event_dict.get("exception"):
        e = event_dict["exception"][0]
        event_dict["exc_type"] = e.get("exc_type", "?")
        event_dict["exc_value"] = e.get("exc_value", "?")

    try:
        # Create a new event dict with improved key sorting
        # because even JSON logs will also be viewed by humans. ;-)
        sorted_dict: structlog.types.EventDict = {
            "timestamp": event_dict["timestamp"],
            "level": event_dict["level"],
            "event": event_dict["event"],
            **event_dict,
        }
    except KeyError:  # pragma: no cover
        # pytest-structlog removes the "timestamp" key.
        # In that case, just use the original event dict.
        sorted_dict = event_dict
    return orjson.dumps(sorted_dict, default=structlog.processors._json_fallback_handler).decode()


def add_extra_fields(
    logger: logging.Logger, method_name: str, event_dict: structlog.types.EventDict
) -> structlog.types.EventDict:
    """
    A structlog processor that adds extra info fields to the event dict.

    This stems largely from structlog but this way, it is more customizable and maybe
    a bit faster, too.
    """
    # ISO UTC timestamp
    # structlog.processors.TimeStamper(fmt="iso", utc=True),
    event_dict["timestamp"] = (
        datetime.datetime.now(tz=datetime.UTC).replace(tzinfo=None).isoformat(timespec="milliseconds") + "Z"
    )

    # Logger name
    # structlog.stdlib.add_logger_name,
    record = event_dict.get("_record")
    if record is None:
        event_dict["logger"] = logger.name
    else:  # pragma: no cover
        event_dict["logger"] = record.name

    # Log level in UPPER CASE
    # structlog.stdlib.add_log_level,
    if method_name == "warn":  # pragma: no cover
        # The stdlib has an alias
        method_name = "warning"
    elif method_name == "exception":
        # exception("") method is the same as error("", exc_info=True)
        method_name = "error"
    event_dict["level"] = method_name.upper()

    return event_dict


def setup_logging(
    *,
    force_json: bool | None = None,
    logger_levels: t.Mapping[str, LogLevels] | None = None,
    verbosity: int = 0,
    testing: bool = False,
    root_logger_level: LogLevels = LogLevels.INFO,
) -> None:
    # Setup structlog
    #
    # The performance guide (https://www.structlog.org/en/stable/performance.html)
    # suggest to not pump structlog log entries through stdlib and use
    # a BytesLoggerFactory instead.
    #
    # However, that would not work for us, because structlog would than only use single
    # logger instance in all libs and apps.  But we want to be able to configure
    # different log levels for our app and the various libs.  This only works if we
    # use the stdlib logger factory that can create different loggers with different
    # names.
    additional_ignores = [__name__, contextlib.__name__]

    pre_chain: list[structlog.types.Processor] = [
        structlog.stdlib.filter_by_level,
        VerboseFilter(verbosity=verbosity),
        structlog.contextvars.merge_contextvars,
    ]

    shared_processors: list[structlog.types.Processor] = [
        add_extra_fields,
        structlog.stdlib.ExtraAdder(),
    ]

    post_chain: list[structlog.types.Processor] = [
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.CallsiteParameterAdder(
            parameters=(
                structlog.processors.CallsiteParameter.FUNC_NAME,
                # structlog.processors.CallsiteParameter.LINENO,
                # structlog.processors.CallsiteParameter.PATHNAME,
            ),
            additional_ignores=additional_ignores,
        ),
        structlog.processors.StackInfoRenderer(additional_ignores=additional_ignores),
        structlog.stdlib.ProcessorFormatter.wrap_for_formatter,
    ]

    structlog.configure(
        cache_logger_on_first_use=not testing,
        # The filtering_bound_logger is fast, but it does not allow us to change the
        # log level at a later stage since the filtering is hard code in the logger
        # instances themselves.
        # In addition, we'd have to compute the minimum log level for all loggers and
        # use this, which might drastically reduce the positive performance impact.
        # wrapper_class=structlog.make_filtering_bound_logger(logging.INFO),
        wrapper_class=structlog.stdlib.BoundLogger,
        processors=pre_chain + shared_processors + post_chain,
        context_class=dict,
        logger_factory=structlog.stdlib.LoggerFactory(),
    )

    show_locals = verbosity >= 4
    dict_tracebacks = structlog.processors.ExceptionRenderer(
        structlog.tracebacks.ExceptionDictTransformer(show_locals=show_locals)
    )
    rich_tracebacks = structlog.dev.RichTracebackFormatter(show_locals=show_locals, width=-1)

    if logger_levels is None:
        logger_levels = dict()

    # Setup stdlib
    cfg = {
        "version": 1,
        "disable_existing_loggers": False,
        "formatters": {
            "json": {
                "()": structlog.stdlib.ProcessorFormatter,
                "processor": functools.partial(render_orjson, dict_tracebacks),
                "foreign_pre_chain": shared_processors,
            },
            "human": {
                "()": structlog.stdlib.ProcessorFormatter,
                "processor": structlog.dev.ConsoleRenderer(colors=True, exception_formatter=rich_tracebacks),
                "foreign_pre_chain": shared_processors,
            },
        },
        "handlers": {
            "stream": {
                "formatter": "json" if (force_json is None and not sys.stdout.isatty()) or force_json else "human",
                "class": "logging.StreamHandler",
                "stream": "ext://sys.stdout",
            },
        },
        "loggers": {name: dict(level=level.value, propagate=True) for name, level in logger_levels.items()},
    }

    logging.config.dictConfig(cfg)

    # Configure root logger manually to make it play well with pytest logging capabilities
    root_logger = logging.getLogger()
    stream_handler = logging.getHandlerByName("stream")
    assert stream_handler is not None
    root_logger.addHandler(stream_handler)
    root_logger.setLevel(root_logger_level.value)

    log.debug(
        "configured logging",
        verbosity=4,
        cfg=cfg,
        root_logger_level=root_logger_level,
        testing=testing,
    )


@contextlib.contextmanager
def logged_error(
    logger: structlog.stdlib.BoundLogger,
    event: str,
    *,
    exceptions: None | t.Collection[type[Exception]] = None,
    **kwargs: t.Any,
) -> t.Generator[None, None, None]:
    try:
        yield
    except Exception as e:
        if exceptions is not None and type(e) not in exceptions:
            raise
        logger.error(event, **kwargs)
