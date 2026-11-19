use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("healthcheck") {
        let probe = args.next().unwrap_or_else(|| "ready".to_owned());
        return match web::startup::healthcheck(&probe).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("healthcheck {probe}: {e}");
                ExitCode::FAILURE
            }
        };
    }

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
