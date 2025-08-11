use tracing::{debug, error, info, trace, warn};

/// Write a log message coming from a plugin.
pub fn write(level: &str, message: &str) {
    match level.to_ascii_uppercase().as_str() {
        "ERROR" => error!("{}", message),
        "WARN" => warn!("{}", message),
        "INFO" => info!("{}", message),
        "DEBUG" => debug!("{}", message),
        "TRACE" => trace!("{}", message),
        _ => info!("{}", message),
    }
}
