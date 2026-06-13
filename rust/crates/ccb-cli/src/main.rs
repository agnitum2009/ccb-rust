fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(ccb_cli::entry::run_cli(&args));
}
