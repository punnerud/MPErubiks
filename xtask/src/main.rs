fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("gen-table") => gen_table(),
        Some("table-stats") => table_stats(),
        Some("pack-table") => pack_table(),
        Some("bench-solve") => bench_solve(
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20),
        ),
        _ => eprintln!("usage: cargo run -p xtask -- <gen-table|pack-table|table-stats|bench-solve [n]>"),
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
/// table.bin -> assets/table.pack (the asset the app actually ships).
fn pack_table() {
    let raw = std::fs::read("assets/table.bin").expect("run gen-table first");
    let decoded = kewb::fs::decode_table(&raw).expect("decode");
    let t0 = std::time::Instant::now();
    let packed = cube_solver::encode_packed(&decoded);
    let back = cube_solver::decode_packed(&packed).expect("roundtrip decode");
    assert_eq!(decoded.pruning_table.ep_e, back.pruning_table.ep_e, "roundtrip");
    std::fs::write("assets/table.pack", &packed).expect("write table.pack");
    println!(
        "table.pack: {} bytes ({:.0}% of {}), packed+verified in {:.1?}",
        packed.len(),
        packed.len() as f64 / raw.len() as f64 * 100.0,
        raw.len(),
        t0.elapsed()
    );
}

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

    // Domain transforms (the matcodec idea proper): shape each section to
    // its value distribution BEFORE the entropy coder sees it.
    // - pruning depths are 0..13 -> two per byte (nibble packing)
    // - u16 move-table coordinates -> separate lo/hi byte planes, so the
    //   mostly-zero hi bytes and smooth lo bytes stop interleaving
    let mut transformed: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, bytes) in &sections {
        if name.starts_with("prune.") {
            let mut packed = Vec::with_capacity(bytes.len() / 2 + 1);
            for pair in bytes.chunks(2) {
                let lo = pair[0] & 0x0F;
                let hi = pair.get(1).copied().unwrap_or(0) & 0x0F;
                packed.push(lo | (hi << 4));
            }
            transformed.push((format!("{name}+nib"), packed));
        } else {
            let half = bytes.len() / 2;
            let mut planes = Vec::with_capacity(bytes.len());
            for i in 0..half {
                planes.push(bytes[i * 2]); // lo plane
            }
            for i in 0..half {
                planes.push(bytes[i * 2 + 1]); // hi plane
            }
            transformed.push((format!("{name}+pln"), planes));
        }
    }
    // What the app can realistically ship: miniz_oxide deflate, inflated
    // once at startup in wasm. Report size AND inflate time.
    let mut deflate_total = 0usize;
    let mut inflate_time = std::time::Duration::ZERO;
    for (name, bytes) in &transformed {
        let t0 = std::time::Instant::now();
        let packed = miniz_oxide::deflate::compress_to_vec(bytes, 10);
        let dt_pack = t0.elapsed();
        let t0 = std::time::Instant::now();
        let back = miniz_oxide::inflate::decompress_to_vec(&packed).expect("inflate");
        inflate_time += t0.elapsed();
        assert_eq!(&back, bytes, "roundtrip {name}");
        println!(
            "  deflate {name:<18} {:>9} -> {:>9}  (pack {dt_pack:.1?})",
            bytes.len(),
            packed.len()
        );
        deflate_total += packed.len();
        if have_zstd {
            if let Some(n) = compress_with("zstd", &["-19", "-c"], bytes) {
                println!("  zstd    {name:<18} {:>9} -> {n:>9}", bytes.len());
            }
        }
    }
    println!("deflate transformed total: {deflate_total:>8} bytes (inflate all: {inflate_time:.1?})");
    println!(
        "summary: raw {} -> transformed+deflate {} ({:.0}%)",
        raw.len(),
        deflate_total,
        deflate_total as f64 / raw.len() as f64 * 100.0
    );
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
    // Feed stdin from a thread: writing multi-MB input while the child's
    // stdout pipe fills up deadlocks both processes otherwise (this is
    // why every previous table-stats run hung forever).
    let mut stdin = child.stdin.take()?;
    let owned = data.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = stdin.write_all(&owned);
    });
    let out = child.wait_with_output().ok()?;
    let _ = feeder.join();
    out.status.success().then_some(out.stdout.len())
}

/// Measure solve latency at different move bounds — drives the app's
/// latency/quality strategy (a frozen UI is worse than a 22-move solve).
fn bench_solve(n: usize) {
    use kewb::{CubieCube, FaceCube, Solver};

    let t0 = std::time::Instant::now();
    let bytes = std::fs::read("assets/table.bin").expect("run gen-table first");
    let decoded = kewb::fs::decode_table(&bytes).expect("decode");
    println!("decode table.bin: {:.2?}", t0.elapsed());

    let t0 = std::time::Instant::now();
    let generated = kewb::DataTable::default();
    println!("generate in-memory table: {:.2?}", t0.elapsed());

    for i in 0..n {
        let state = kewb::generators::generate_random_state();
        for (name, table) in [("decoded ", &decoded), ("generated", &generated)] {
            for max_len in [23u8, 21] {
                let t0 = std::time::Instant::now();
                let sol = Solver::new(table, max_len, None).solve(state);
                match sol {
                    Some(sol) => println!(
                        "{name} max {max_len} state {i}: {:>9.2?}  len {}",
                        t0.elapsed(),
                        sol.get_all_moves().len()
                    ),
                    None => println!(
                        "{name} max {max_len} state {i}: {:>9.2?}  NO SOLUTION",
                        t0.elapsed()
                    ),
                }
            }
        }
        // Silence unused-import warnings for conversions kept for later.
        let _ = (FaceCube::try_from(&state), CubieCube::default());
    }
}
