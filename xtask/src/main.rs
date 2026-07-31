fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("gen-table") => gen_table(),
        _ => eprintln!("usage: cargo run -p xtask -- gen-table"),
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
