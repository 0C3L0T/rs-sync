use std::fmt;

use sha2::{Digest, Sha256};

pub const KB: usize = 1024;
pub const MB: usize = KB * KB;
pub const GB: usize = MB * KB;
pub const TB: usize = GB * KB;

pub struct Block(pub Vec<u8>);

impl From<&[u8]> for Block {
    fn from(bytes: &[u8]) -> Self {
        Block(bytes.to_owned())
    }
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl Block {
    const ROLL_MOD: u32 = 2u32.pow(16);
    const MIN_BLOCK_SIZE: usize = 4 * KB;
    const MAX_BLOCK_SIZE: usize = 4 * MB;

    pub fn compute_strong_checksum(data: &[u8]) -> Vec<u8> {
        Sha256::digest(data).to_vec()
    }

    // More details about the rolling checksum algorithm:
    // https://rsync.samba.org/tech_report/node3.html
    pub fn compute_weak_checksum(data: &[u8]) -> u32 {
        let mut a = 0u32;
        let mut b = 0u32;

        for (i, &byte) in data.iter().enumerate() {
            a = (a + byte as u32).rem_euclid(Self::ROLL_MOD);
            b = (b + (data.len() as u32 - i as u32) * byte as u32).rem_euclid(Self::ROLL_MOD);
        }

        (b << 16) | a
    }

    pub fn update_weak_checksum(
        old_checksum: u32,
        old_byte: u8,
        new_byte: u8,
        block_size: usize,
    ) -> u32 {
        let a = old_checksum & 0xFFFF;
        let b = (old_checksum >> 16) & 0xFFFF;

        // using this modular arithmetic property:
        // (x - y) % M = ((x % M) - (y % M) + M) % M
        // ensure that subtraction never results in a negative value (thus, avoiding overflow panic)
        // - ORIGINAL FORMULA
        // let new_a = (a + new_byte as u32 - old_byte as u32).rem_euclid(Self::MOD);
        // - NON-NEGATIVE FORMULA
        let new_a = ((a + new_byte as u32).rem_euclid(Self::ROLL_MOD) + Self::ROLL_MOD
            - (old_byte as u32).rem_euclid(Self::ROLL_MOD))
        .rem_euclid(Self::ROLL_MOD);

        // - ORIGINAL FORMULA
        // let new_b = (b + new_a - (block_size as u32) * old_byte as u32).rem_euclid(Self::MOD);
        // - NON-NEGATIVE FORMULA
        let new_b = ((b + new_a).rem_euclid(Self::ROLL_MOD) + Self::ROLL_MOD
            - (block_size as u32 * old_byte as u32).rem_euclid(Self::ROLL_MOD))
        .rem_euclid(Self::ROLL_MOD);

        (new_b << 16) | new_a
    }

    pub fn cut_weak_checksum(checksum: u32) -> u16 {
        (checksum & 0xFFFF) as u16
    }

    pub fn calculate_block_size(src_size: u64, dest_size: u64) -> usize {
        let src_block_size = Self::calculate_individual_block_size(src_size);
        let dest_block_size = Self::calculate_individual_block_size(dest_size);

        src_block_size.min(dest_block_size)
    }

    fn calculate_individual_block_size(file_size: u64) -> usize {
        // let log_factor = ((file_size as f64) / (Self::MIN_BLOCK_SIZE as f64))
        //     .log2()
        //     .floor() as usize;
        // let block_size = Self::MIN_BLOCK_SIZE * (1 + log_factor);

        let block_size = (file_size as f64).sqrt().round() as usize;

        block_size.clamp(Self::MIN_BLOCK_SIZE, Self::MAX_BLOCK_SIZE)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_update_weak_checksum() {
        let data = b"abcd";
        let old_checksum = Block::compute_weak_checksum(data);
        let updated_checksum = Block::update_weak_checksum(old_checksum, b'a', b'e', data.len());

        let expected_updated_checksum = Block::compute_weak_checksum(b"bcde");

        assert_eq!(updated_checksum, expected_updated_checksum);
    }

    #[test]
    fn test_cut_weak_checksum() {
        let checksum = 0xDEADBEEF;
        let expected_cut_checksum = 0xBEEF;
        assert_eq!(Block::cut_weak_checksum(checksum), expected_cut_checksum);
    }
}
