mod backend;
pub(crate) mod control;
mod service;
mod stdio;

pub use stdio::run_stdio_proxy;

#[cfg(test)]
mod tests;
