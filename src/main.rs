mod models;
mod config;
mod utils;
mod commands;

fn main() -> anyhow::Result<()> {
    commands::run()
}
