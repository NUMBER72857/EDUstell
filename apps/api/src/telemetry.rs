use tracing_subscriber::{
    EnvFilter, Layer, filter::filter_fn, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{config::ObservabilityConfig, error::InternalError};

pub fn init(config: &ObservabilityConfig) -> Result<(), InternalError> {
    let env_filter = EnvFilter::try_new(&config.rust_log)
        .map_err(|err| InternalError::Config(format!("invalid RUST_LOG: {err}")))?;

    let app_layer = match config.log_format.as_str() {
        "json" => fmt::layer()
            .json()
            .with_filter(filter_fn(|metadata| metadata.target() != "audit"))
            .boxed(),
        "pretty" => fmt::layer()
            .pretty()
            .with_filter(filter_fn(|metadata| metadata.target() != "audit"))
            .boxed(),
        other => {
            return Err(InternalError::Config(format!("invalid LOG_FORMAT value: {other}")));
        }
    };

    let audit_layer = fmt::layer()
        .json()
        .with_filter(filter_fn(|metadata| metadata.target() == "audit"))
        .boxed();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(app_layer)
        .with(audit_layer)
        .try_init()
        .map_err(|err| InternalError::Startup(format!("failed to initialize tracing: {err}")))?;

    Ok(())
}
