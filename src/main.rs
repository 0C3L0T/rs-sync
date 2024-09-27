use clap::Parser;

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

impl Cli {
    fn exec(self) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    cli.exec()?;

    Ok(())
}
