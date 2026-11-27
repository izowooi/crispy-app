// DeepZ (Windows) — Ctrl+C,C 로 선택 영역을 번역해 주는 트레이 앱
#![cfg_attr(windows, windows_subsystem = "windows")]

mod core;
#[cfg(windows)]
mod win;

fn main() {
    // 점검용: deepz --translate "text" → 번역을 표준 출력으로 내보내고 종료
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 && args[1] == "--translate" {
        use std::io::Write;
        let text = args[2..].join(" ");
        let result = core::translate(&text, &core::target_for(&text), &core::load_env(), &mut |delta| {
            print!("{delta}");
            std::io::stdout().flush().is_ok()
        });
        match result {
            Ok(model) => println!("\n[{model}]"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }

    #[cfg(windows)]
    win::run();
    #[cfg(not(windows))]
    eprintln!("이 프로그램은 Windows 용입니다. macOS 에서는 --translate 점검만 됩니다.");
}
