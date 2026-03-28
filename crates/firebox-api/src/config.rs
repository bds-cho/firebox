use config::{Config, Environment, File};
use firebox_core::DaemonConfig;

pub fn load_config() -> Result<DaemonConfig, config::ConfigError> {
    let cfg = Config::builder()
        // Defaults
        .set_default("firecracker_bin", "/usr/bin/firecracker")?
        .set_default("listen_addr", "127.0.0.1:8080")?
        .set_default("socket_dir", "/run/firebox/sockets")?
        .set_default("log_level", "info")?
        // Optional config file
        .add_source(File::with_name("/etc/firebox/config").required(false))
        // Environment overrides: FIREBOX_LISTEN_ADDR, etc.
        .add_source(Environment::with_prefix("FIREBOX").separator("_"))
        .build()?;

    cfg.try_deserialize()
}
