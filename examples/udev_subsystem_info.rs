use clap::Parser;
use tux_validation::utils::subsystem_info_udev;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Performs a provided subsystem scan with udev."
)]
struct Args {
    //Subsystem to perform scan on
    subsystem: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    subsystem_info_udev(args.subsystem)
}
