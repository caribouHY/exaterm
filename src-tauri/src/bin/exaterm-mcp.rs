#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();

    if let Err(error) = exaterm_lib::run_stdio_proxy().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
