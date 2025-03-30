mod models;
mod config;
mod utils;
mod commands;
mod rclone;

fn main() -> anyhow::Result<()> {
    commands::run()
}
