use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
};

use crate::block::Block;

pub struct ChunkedFile(pub Vec<Block>);

impl ChunkedFile {
    pub fn new() -> Self {
        ChunkedFile(vec![])
    }
}

/// Represents how the file must be synced
#[derive(Debug)]
pub enum CopyKind {
    /// the file must be copied from scratch
    Full,

    /// the file must be incrementally copied
    Incremental,

    /// the file already exists
    None,
}

pub fn should_copy(src: &Path, dest: &Path) -> anyhow::Result<CopyKind> {
    if !dest.exists() {
        return Ok(CopyKind::Full);
    }

    let src_metadata = fs::metadata(src)?;
    let dest_metadata = fs::metadata(dest)?;

    if src_metadata.len() != dest_metadata.len()
        || src_metadata.modified()? != dest_metadata.modified()?
    {
        return Ok(CopyKind::Full);
    }

    let chunked_src = retrieve_chunked_file(src)?;
    let chunked_dest = retrieve_chunked_file(dest)?;

    for (src_block, dest_block) in chunked_src.0.iter().zip(chunked_dest.0.iter()) {
        let src_checksum = src_block.compute_checksum();
        let dest_checksum = dest_block.compute_checksum();

        if src_checksum != dest_checksum {
            return Ok(CopyKind::Incremental);
        }
    }

    Ok(CopyKind::None)
}

pub fn retrieve_chunked_file(path: &Path) -> anyhow::Result<ChunkedFile> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![0; 10]; // vec![0; BLOCK_SIZE];
    let mut chunked_file = ChunkedFile::new();

    while let Ok(bytes_read) = reader.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }

        chunked_file.0.push(buffer[..bytes_read].into());
    }

    Ok(chunked_file)
}

pub fn send_file(src: &Path, dest: &Path) -> anyhow::Result<()> {
    if !matches!(should_copy(src, dest)?, CopyKind::None) {
        println!("Copying file: {} -> {}", src.display(), dest.display());
        copy_file(src, dest)?;
    }

    Ok(())
}

fn copy_file(src: &Path, dest: &Path) -> anyhow::Result<()> {
    todo!()
}
