//! `wield` — the Wield command-line entry point.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let code = wield_cli::run(std::env::args().collect()).await;
    std::process::exit(code);
}
