//! One build entry point on either desktop OS. Only the host's native adapter
//! is compiled into this helper; shared engine code needs no source edits.
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::Command;
use std::process::ExitCode;

#[cfg(target_os = "macos")]
fn platform_command() -> Command {
    let mut command = Command::new("zsh");
    command.arg("scripts/build-app.sh");
    command
}

#[cfg(target_os = "windows")]
fn platform_command() -> Command {
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts/build-windows.ps1",
    ]);
    command
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Sua View native app builder\n\nUsage: cargo run --locked --bin sua-view-build [-- --dry-run]\n\nSelects the native macOS or Windows build without editing source files."
        );
        return Ok(());
    }
    if args.iter().any(|arg| arg != "--dry-run") {
        return Err("Unknown argument. Use --help.".into());
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let mut command = platform_command();
        command.current_dir(env!("CARGO_MANIFEST_DIR"));
        if args.iter().any(|arg| arg == "--dry-run") {
            println!("{command:?}");
            return Ok(());
        }
        let status = command
            .status()
            .map_err(|error| format!("Cannot start native build: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Native build failed: {status}"))
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("Native desktop packaging supports macOS and Windows only.".into())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Sua View: {error}");
            ExitCode::FAILURE
        }
    }
}
