use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match web::startup::start().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Logging may not be initialised yet (e.g. a bad RUST_LOG), so the
            // reason also goes to stderr where the container runtime keeps it.
            tracing::error!(error = %e, "startup failed");
            eprintln!("agri-connect: startup failed: {e}");
            ExitCode::FAILURE
        }
    }
}
