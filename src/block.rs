use sha2::{Digest, Sha256};

pub const BLOCK_SIZE: usize = 4 * 1024;

pub struct Block(pub Vec<u8>);

impl From<&[u8]> for Block {
    fn from(bytes: &[u8]) -> Self {
        Block(bytes.to_owned())
    }
}

impl Block {
    pub fn compute_checksum(&self) -> Vec<u8> {
        Sha256::digest(&self.0).to_vec()
    }
}
