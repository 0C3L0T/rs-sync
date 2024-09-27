mod block;
mod file;
mod path;

use clap::Parser;
use file::send_file;
use path::FilePath;

/// rs-snyc, a local and remote file-copying tool
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Source file or directory
    #[arg(required = true)]
    source: String,

    /// Destination file or directory
    #[arg(required = true)]
    destination: String,

    /// Recursively copy entire directories
    #[arg(short, long)]
    recursive: bool,

    /// Archive mode
    #[arg(short, long)]
    archive: bool,

    /// Show progress during transfer
    #[arg(short, long)]
    progress: bool,

    /// Increase verbosity
    #[arg(long)]
    verbose: bool,

    /// Compress file data during the transfer
    #[arg(short, long)]
    compress: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let src = match FilePath::parse(&cli.source) {
        FilePath::Local(path) => path,
        FilePath::Remote {
            user: _,
            host: _,
            path: _,
        } => todo!(),
    };

    let dest = match FilePath::parse(&cli.destination) {
        FilePath::Local(path) => path,
        FilePath::Remote {
            user: _,
            host: _,
            path: _,
        } => todo!(),
    };

    send_file(src, dest)?;

    // for entry in WalkDir::new(src) {
    //     let entry = entry?;
    //     let src_path = entry.path();
    //     if src_path.is_file() {
    //     }
    // }

    Ok(())
}
