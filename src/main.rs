use clap::Parser;

pub mod archive_cleaner;
pub mod file_handler;
pub mod formatter;

// #[derive(Args, Debug)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    module: String,
}

fn main() {
    let args = Args::parse();
    match args.module.as_str() {
        "formatter" => formatter::main(),
        _ => println!("Unknown module: {}", args.module),
        
    }
}
