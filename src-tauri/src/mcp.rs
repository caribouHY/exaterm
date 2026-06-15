mod backend;
mod service;
mod stdio;

pub use stdio::run_stdio_proxy;

#[cfg(test)]
mod tests;
