#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use syncrs::delta::compute_deltas;
use syncrs::sync::BlockDescriptor;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    block_size: usize,
    buffer: Vec<u8>,
    descriptors: HashMap<u16, Vec<BlockDescriptor>>,
}

fuzz_target!(|data: FuzzInput| {
    // fuzzed code goes here
    if data.block_size == 0 || data.buffer.len() > 1_000_000 || data.descriptors.len() > 10_000 {
        return;
    }
    let _ = compute_deltas(data.block_size, data.buffer, data.descriptors);
});
