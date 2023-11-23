use clap::Parser;
use magoo::Magoo;

fn main() {
    let cli = Magoo::parse();
    cli.set_print_options();
    if let Err(e) = cli.run() {
        eprintln!("magoo: fatal:");
        eprintln!("  {e}");
    }
}
