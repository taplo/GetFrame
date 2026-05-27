use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

pub fn init(config: &crate::config::LoggingConfig) {
    let fmt_layer = if config.json {
        fmt::layer().json().with_target(true).boxed()
    } else {
        fmt::layer().compact().boxed()
    };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
