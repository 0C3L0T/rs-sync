use std::{
    collections::HashMap,
    fs::Metadata,
    io,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use arbitrary::Arbitrary;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::block::Block;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub len: u64,
    pub permissions: u32,
    pub modified: Option<u64>,
    pub is_dir: bool,
    // NOTE: These two fields aren't archived by rsync either
    // pub accessed: Option<u64>,
    // pub created: Option<u64>,
}

impl FileMetadata {
    pub fn new(path: PathBuf, metadata: &Metadata) -> Self {
        let time_since_unix = |time: io::Result<SystemTime>| {
            time.ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs()))
        };

        Self {
            path,
            len: metadata.len(),
            permissions: metadata.permissions().mode(),
            modified: time_since_unix(metadata.modified()),
            is_dir: metadata.is_dir(),
        }
    }

    pub fn needs_update(&self, dest: &Self) -> bool {
        self.len != dest.len
            || self.permissions != dest.permissions
            || self.modified != dest.modified
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Delta {
    /// literal new data sent from the source
    Literal(Vec<u8>),
    /// reference index used to retrieve existing data in the destination
    Reference(usize),
}

#[derive(Arbitrary, Debug, Serialize, Deserialize)]
pub struct BlockDescriptor {
    pub weak: u32,
    pub strong: Vec<u8>,
    pub index: usize,
}

impl BlockDescriptor {
    pub fn new(weak: u32, strong: Vec<u8>, index: usize) -> Self {
        Self {
            weak,
            strong,
            index,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.weak == 0 && self.strong == vec![0] && self.index == 0
    }
}

pub async fn slice_dest(
    reader: &mut (impl AsyncReadExt + Unpin),
    block_size: usize,
) -> (Vec<Block>, HashMap<u16, Vec<BlockDescriptor>>) {
    let mut buffer = vec![0; block_size];
    let mut blocks: Vec<Block> = vec![];
    let mut map: HashMap<u16, Vec<BlockDescriptor>> = HashMap::new();

    let mut i = 0;
    while let Ok(bytes_read) = reader.read(&mut buffer).await {
        if bytes_read == 0 {
            break;
        }

        let block: Block = buffer[..bytes_read].into();
        let dest_weak = Block::compute_weak_checksum(&block.0);
        let dest_strong = Block::compute_strong_checksum(&block.0);

        blocks.push(block);

        map.entry(Block::cut_weak_checksum(dest_weak))
            .or_default()
            .push(BlockDescriptor::new(dest_weak, dest_strong, i));

        i += 1;
    }

    (blocks, map)
}

pub fn compute_deltas(
    block_size: usize,
    buffer: Vec<u8>,
    descriptors: HashMap<u16, Vec<BlockDescriptor>>,
) -> Vec<Delta> {
    let mut previous_checksum = None;
    let mut accumulated_bytes = vec![];
    let mut deltas = vec![];

    let mut i: usize = 0;
    while i + block_size <= buffer.len() {
        let window = &buffer[i..(i + block_size)];
        let src_weak = if let Some(prev) = previous_checksum {
            let old_byte = buffer[i - 1];
            let new_byte = buffer[i + block_size - 1];
            Block::update_weak_checksum(prev, old_byte, new_byte, block_size)
        } else {
            Block::compute_weak_checksum(window)
        };
        previous_checksum = Some(src_weak);

        let half_weak = Block::cut_weak_checksum(src_weak);
        let mut found = false;

        if let Some(dest_data) = descriptors.get(&half_weak) {
            for BlockDescriptor {
                weak: dest_weak,
                strong: dest_strong,
                index: dest_index,
            } in dest_data
            {
                if *dest_weak == src_weak {
                    let src_strong = Block::compute_strong_checksum(window);

                    if *dest_strong == src_strong {
                        if !accumulated_bytes.is_empty() {
                            deltas.push(Delta::Literal(std::mem::take(&mut accumulated_bytes)));
                        }

                        deltas.push(Delta::Reference(*dest_index));

                        found = true;
                        i += block_size;
                        previous_checksum = None;
                        break;
                    }
                }
            }
        }

        if !found {
            accumulated_bytes.push(buffer[i]);
            i += 1;
        }
    }

    // handle any trailing unmatched data
    if i < buffer.len() {
        buffer[i..].iter().for_each(|b| {
            accumulated_bytes.push(*b);
        });
    }
    if !accumulated_bytes.is_empty() {
        deltas.push(Delta::Literal(accumulated_bytes));
    }

    deltas
}
