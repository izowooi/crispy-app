use std::process::ExitCode;
use std::time::Instant;
use sua_view_core::Book;

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.is_empty() || arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Sua View engine {}\n\nUsage:\n  sua-view-engine inspect PATH\n  sua-view-engine benchmark PATH [READS]\n\nInspects a local book or measures index and raw-page IO. Image decoding and\nwindow presentation are measured separately by the native app.",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let command = &arguments[0];
    let path = arguments
        .get(1)
        .ok_or("A book path is required. Use --help.")?;
    let started = Instant::now();
    let mut book = Book::open(path)?;
    let index_ms = started.elapsed().as_secs_f64() * 1000.;
    match command.as_str() {
        "inspect" => {
            println!(
                "source={} pages={} initial_page={} index_ms={index_ms:.3}",
                book.source_kind(),
                book.len(),
                book.initial_page() + 1
            );
            for index in 0..book.len() {
                println!("{}\t{}", index + 1, book.page_name(index).unwrap_or(""));
            }
        }
        "benchmark" => {
            let reads: usize = arguments
                .get(2)
                .map(|v| v.parse().map_err(|_| "READS must be a positive integer"))
                .transpose()?
                .unwrap_or(100);
            if reads == 0 || reads > 100_000 {
                return Err("READS must be between 1 and 100000".into());
            }
            let started = Instant::now();
            let first = book.read_page(book.initial_page())?;
            let first_ms = started.elapsed().as_secs_f64() * 1000.;
            let mut times = Vec::with_capacity(reads);
            let mut bytes = 0;
            for index in 0..reads {
                let start = Instant::now();
                bytes += book.read_page(index % book.len())?.len();
                times.push(start.elapsed().as_secs_f64() * 1000.);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "source={} pages={} index_ms={index_ms:.3} first_page_ms={first_ms:.3} first_bytes={} reads={reads} read_p50_ms={:.3} read_p95_ms={:.3} read_max_ms={:.3} total_bytes={bytes}",
                book.source_kind(),
                book.len(),
                first.len(),
                times[reads / 2],
                times[(reads * 95 / 100).min(reads - 1)],
                times[reads - 1]
            );
        }
        _ => return Err(format!("Unknown command {command:?}. Use --help.")),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("Sua View: {message}");
            ExitCode::FAILURE
        }
    }
}
