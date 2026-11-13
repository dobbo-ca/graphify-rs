use std::process::ExitCode;

// Defaults from graphify-go cmd/graphify/main.go:154-156; release builds will set them.
const VERSION: &str = "dev";
const COMMIT: &str = "none";
const DATE: &str = "unknown";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(cmd) = args.get(1) else {
        eprintln!("usage: graphify <command> [args]");
        return ExitCode::from(2);
    };
    match cmd.as_str() {
        "version" | "--version" | "-v" => {
            println!("graphify {VERSION} (commit {COMMIT}, built {DATE})");
            ExitCode::SUCCESS
        }
        // ponytail: Debug quoting matches Go %q for ASCII only; gostd::strconv::quote replaces it.
        other => {
            eprintln!("graphify: unknown command {other:?}");
            ExitCode::from(1)
        }
    }
}
