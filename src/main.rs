use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    // `time::UtcOffset::current_local_offset()` returns an error in multi-
    // threaded contexts, so resolve the offset eagerly while we're still
    // single-threaded.
    synchrogit::clock::init_local_offset();

    let cli = synchrogit::cli::Cli::parse();
    synchrogit::log_setup::init();

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("synchrogit: failed to start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(synchrogit::cli::dispatch(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("synchrogit: {e:#}");
            ExitCode::FAILURE
        }
    }
}
