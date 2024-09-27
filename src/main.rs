mod path;

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use clap::Parser;
use path::FilePath;
use walkdir::WalkDir;

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

const BLOCK_SIZE: usize = 4 * 1024;

fn process_file(path: &Path) -> anyhow::Result<()> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![0; 10]; // vec![0; BLOCK_SIZE];
    let mut blocks = Vec::new();

    while let Ok(bytes_read) = reader.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }

        blocks.push(buffer[..bytes_read].to_owned());
    }

    println!("{:#?}", blocks.len());

    // Here you can apply checksum logic or incremental copy
    // compute_checksum(buffer.as_slice());

    Ok(())
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

    let _dest = match FilePath::parse(&cli.destination) {
        FilePath::Local(path) => path,
        FilePath::Remote {
            user: _,
            host: _,
            path: _,
        } => todo!(),
    };

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            println!("hey {:?}", path.display());
            process_file(path)?;
        }
    }

    Ok(())
}
