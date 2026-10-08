use config::Config;
use tracing_subscriber::{fmt, EnvFilter};

/// Initialise the global tracing subscriber.
///
/// Priority (highest wins):
///   1. `RUST_LOG` environment variable (full EnvFilter syntax).
///   2. `config.toml [logs] level` applied to soundgnome crates only;
///      noisy third-party crates are capped at `warn` unless the level is
///      `trace`, in which case everything is opened up.
pub fn init_logger() {
    let soundgnome_level = &Config::get().logs.level;

    let filter = std::env::var("RUST_LOG").ok().unwrap_or_else(|| {
        let reqwest_logs = Config::get().logs.enable_reqwest_logging;

        // When the configured level is trace we open everything; otherwise
        // external crates stay at warn to reduce noise.
        if soundgnome_level == "trace" {
            return soundgnome_level.clone();
        }

        let reqwest_directive = if reqwest_logs {
            format!(",reqwest={},hyper={}", soundgnome_level, soundgnome_level)
        } else {
            String::new()
        };

        format!(
            "warn,rustypipe=error,soundgnome={level},server={level},domain={level},shared={level},\
             config={level},database={level},downloader={level},fetcher={level},\
             tagger={level},organizer={level},ai={level}{reqwest}",
            level = soundgnome_level,
            reqwest = reqwest_directive
        )
    });

    fmt()
        .with_env_filter(
            EnvFilter::try_new(&filter).unwrap_or_else(|_| EnvFilter::new("warn,soundgnome=info")),
        )
        .init();
}
