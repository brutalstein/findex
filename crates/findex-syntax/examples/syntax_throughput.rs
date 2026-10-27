//! Offline diagnostic: isolates syntax work from file I/O and SQLite.
use std::borrow::Cow;
use std::path::Path;
use std::time::Instant;

fn analyze_range(
    iterations: usize,
    first: usize,
    workers: usize,
    directory: Option<&str>,
) -> Result<(), String> {
    let source = "pub fn indexed_probe(value: usize) -> usize { value + 1 }\n";
    for number in (first..iterations).step_by(workers) {
        let text = match directory {
            Some(root) => Cow::Owned(
                std::fs::read_to_string(Path::new(root).join(format!("pkg/{number:05}.rs")))
                    .map_err(|error| error.to_string())?,
            ),
            None => Cow::Borrowed(source),
        };
        let analysis = findex_syntax::analyze(Path::new("probe.rs"), &text)?
            .ok_or("Rust grammar unavailable")?;
        assert_eq!(analysis.symbols.len(), 1);
        let expected = if directory.is_some() {
            format!("marker_original_{number:05}")
        } else {
            "indexed_probe".to_owned()
        };
        assert_eq!(analysis.symbols[0].name, expected);
        assert_eq!(analysis.symbols[0].start_line, 1);
        std::hint::black_box(analysis);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iterations = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(10_000);
    let directory = std::env::args().nth(2);
    let workers = std::env::args()
        .nth(3)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if !(1..=16).contains(&workers) {
        return Err("workers must be between 1 and 16".into());
    }
    let started = Instant::now();
    std::thread::scope(|scope| -> Result<(), String> {
        let handles: Vec<_> = (0..workers)
            .map(|first| {
                let directory = directory.as_deref();
                scope.spawn(move || analyze_range(iterations, first, workers, directory))
            })
            .collect();
        for handle in handles {
            handle
                .join()
                .map_err(|_| "syntax benchmark worker panicked")??;
        }
        Ok(())
    })?;
    println!(
        "{{\"iterations\":{iterations},\"workers\":{workers},\"seconds\":{},\"with_file_io\":{}}}",
        started.elapsed().as_secs_f64(),
        directory.is_some()
    );
    Ok(())
}
