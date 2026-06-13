#[tokio::main]
async fn main() {
    std::process::exit(exaterm_lib::run_terminal_cli().await);
}
