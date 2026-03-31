use px_cli::{Cli, LogFormat};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(cli: &Cli) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match cli.verbose {
            0 => "error",
            1 => "info,siril_sys=debug",
            2 => "debug",
            _ => "trace",
        };
        EnvFilter::new(level)
    });

    let json_log = matches!(cli.log_format, LogFormat::Json);

    let json_layer = json_log.then(|| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_current_span(true)
            .with_span_list(true)
    });

    let pretty_layer = (!json_log).then(|| {
        tracing_subscriber::fmt::layer()
            .pretty()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(pretty_layer)
        .init();
}
