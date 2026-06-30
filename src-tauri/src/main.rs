use std::process::ExitCode;

fn main() -> ExitCode {
    match modelhub_windows_lib::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run ModelHub Windows: {error}");
            ExitCode::FAILURE
        }
    }
}
