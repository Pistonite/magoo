use std::process::exit;

use clap::Parser;
use magoo::Magoo;

fn main() {
    let cli = Magoo::parse();
    cli.set_print_options();
    if let Err(e) = cli.run() {
        println!("magoo: fatal:");
        println!("  {e}");
        exit(1)
    }
}
