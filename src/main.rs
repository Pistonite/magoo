use std::process::exit;

use clap::Parser;
use magoo::Magoo;
use magoo::git::GitError;

fn main() {
    let cli = Magoo::parse();
    cli.set_print_options();
    if let Err(e) = cli.run() {
        if let GitError::NeedFix(false) = e {
            exit(1)
        }
        println!("magoo: fatal:");
        println!("  {e}");
        exit(2)
    }
}
