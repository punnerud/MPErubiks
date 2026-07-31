fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("gen-table") => gen_table(),
        Some("table-stats") => table_stats(),
        Some("bench-solve") => bench_solve(
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20),
        ),
        _ => eprintln!("usage: cargo run -p xtask -- <gen-table|table-stats|bench-solve [n]>"),
    }
}

fn gen_table() {
    let out = std::path::Path::new("assets/table.bin");
    if out.exists() {
        println!("{} already exists; delete it to regenerate", out.display());
        return;
    }
    println!("generating kewb move/pruning tables, this takes a while...");
    kewb::fs::write_table(out).expect("failed to generate/write table");
    let size = std::fs::metadata(out).unwrap().len();
    println!("wrote {} ({size} bytes)", out.display());
}

/// M7 experiment: can MPEE/matcodec-style domain decomposition beat plain
/// whole-file compression for the solver table asset? Reports the sizes so
/// the decision is data, not vibes. (The web server should serve the file
/// with its own transport compression either way; this informs whether a
/// custom pre-compressed format is worth maintaining.)
fn table_stats() {
    let path = std::path::Path::new("assets/table.bin");
    let raw = std::fs::read(path).expect("assets/table.bin missing — run gen-table");
    println!("raw table.bin:            {:>9} bytes", raw.len());

    // Baseline: generic compressors over the whole file (what a web server
    // or CDN gives for free).
    for (tool, args) in [("gzip", vec!["-9", "-c"]), ("xz", vec!["-9", "-c"]), ("zstd", vec!["-19", "-c"])] {
        match compress_with(tool, &args, &raw) {
            Some(n) => println!("{tool:<5} whole file:         {n:>9} bytes"),
            None => println!("{tool:<5} not installed — skipped"),
        }
    }

    // Domain decomposition: the table is one bincode stream of six move
    // tables (varint u16 coordinates) and four pruning tables (u8 depths
    // 0..13). Splitting lets the compressor model each distribution
    // separately — the matcodec idea applied at its simplest.
    let decoded = kewb::fs::decode_table(&raw).expect("decode");
    let mut sections: Vec<(String, Vec<u8>)> = Vec::new();
    {
        let m = &decoded.move_table;
        sections.push(("move.co".into(), u16s(&m.co)));
        sections.push(("move.eo".into(), u16s(&m.eo)));
        sections.push(("move.e_combo".into(), u16s(&m.e_combo)));
        sections.push(("move.cp".into(), u16s(&m.cp)));
        sections.push(("move.ep".into(), u16s(&m.ep)));
        sections.push(("move.e_ep".into(), u16s(&m.e_ep)));
        let p = &decoded.pruning_table;
        sections.push(("prune.co_e".into(), u8s(&p.co_e)));
        sections.push(("prune.eo_e".into(), u8s(&p.eo_e)));
        sections.push(("prune.cp_e".into(), u8s(&p.cp_e)));
        sections.push(("prune.ep_e".into(), u8s(&p.ep_e)));
    }
    let fixed_total: usize = sections.iter().map(|(_, b)| b.len()).sum();
    println!("fixed-width sections:     {fixed_total:>9} bytes (vs bincode varint)");
    let mut per_section_zstd = 0usize;
    let mut have_zstd = true;
    for (name, bytes) in &sections {
        match compress_with("zstd", &["-19", "-c"], bytes) {
            Some(n) => {
                println!("  zstd {name:<14} {:>9} -> {n:>9}", bytes.len());
                per_section_zstd += n;
            }
            None => {
                have_zstd = false;
                break;
            }
        }
    }
    if have_zstd {
        println!("zstd per-section total:   {per_section_zstd:>9} bytes");
    }
}

fn u16s(t: &[Vec<u16>]) -> Vec<u8> {
    let mut out = Vec::new();
    for row in t {
        for &v in row {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

fn u8s(t: &[Vec<u8>]) -> Vec<u8> {
    t.iter().flat_map(|r| r.iter().copied()).collect()
}

fn compress_with(tool: &str, args: &[&str], data: &[u8]) -> Option<usize> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(data).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status.success().then_some(out.stdout.len())
}

/// Measure solve latency at different move bounds — drives the app's
/// latency/quality strategy (a frozen UI is worse than a 22-move solve).
fn bench_solve(n: usize) {
    let bytes = std::fs::read("assets/table.bin").expect("run gen-table first");
    cube_solver::install_table(&bytes).expect("table");
    for max_len in [23u8, 21, 20] {
        let mut total = std::time::Duration::ZERO;
        let mut worst = std::time::Duration::ZERO;
        let mut lengths = Vec::new();
        for _ in 0..n {
            let state = cube_solver::random_state();
            let t0 = std::time::Instant::now();
            let alg = cube_solver::solve_bounded_public(&state, max_len).expect("solve");
            let dt = t0.elapsed();
            total += dt;
            worst = worst.max(dt);
            lengths.push(alg.len_htm());
        }
        let avg_len: f64 = lengths.iter().sum::<usize>() as f64 / n as f64;
        println!(
            "max {max_len}: avg {:>7.1?}  worst {:>7.1?}  avg len {avg_len:.1}  (n={n})",
            total / n as u32,
            worst
        );
    }
}
