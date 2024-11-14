#![deny(clippy::perf)]
#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
mod cli;
mod color;
mod generate;
mod schema;
mod util;

use std::process::exit;

use clap::Parser;
use cli::Cli;

fn main() -> ! {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    // I want to actually see the panic without having to go into /tmp if I'm in debug mode.
    // I should really make my own version of `human_panic` one day
    #[cfg(not(debug_assertions))]
    {
        use human_panic::setup_panic;
        setup_panic!();
    }
    pretty_env_logger::init();
    let cli = Cli::parse();
    cli.run();
    exit(0)
}
