// res_index_write: serialise a `ResourceIndex` back into the V1 on-disk layout.
//
// The reader half lives in `res_index.rs`; this is its inverse. It exists so the build can
// stop depending on restool for the index: once ids are assigned in-tree (`res_ids`) and the
// entries are known, the file is just a fixed layout with no compression and no checksum.
//
// V1 layout (measured, `Restool 6.1.0.003`):
//
//   0x000           128-byte version block: "Restool <version>" then zero padding
//   0x080           u32 total file size
//   0x084           u32 config count
//   0x088           config count × KEYS block:
//                     "KEYS", u32 id-block offset, u32 qualifier count,
//                     qualifier count × (u32 kind, u32 value)
//   …               config count × IDSS block, at the offsets the KEYS blocks name:
//                     "IDSS", u32 entry count, entry count × (u32 id, u32 record offset)
//   …               records, each at the offset its IDSS row names:
//                     u32 payload size, u32 type, u32 id,
//                     u16 value size + value bytes, u16 name size + name bytes (NUL included)
//
// `payload size` counts everything after itself, so it is `4 + 4 + 2 + value + 2 + name`.
// Blocks follow each other in that order with no padding, so the offsets are entirely derived
// from the sizes. Every integer is little-endian (the reader's `u32`/`u16` are LE too).

#![allow(dead_code)] // exercised by the round-trip tests; the build still gets its index from
                     // restool until `res_index_build` reproduces the config ordering.

use super::res_index::ResourceIndex;

/// Serialise one index into V1 bytes.
pub(crate) fn write(index: &ResourceIndex) -> Vec<u8> {
    const HEADER: usize = 0x80;

    let keys_size: usize = index
        .configs
        .iter()
        .map(|config| 12 + 8 * config.qualifiers.len())
        .sum();
    let ids_size: usize = index
        .configs
        .iter()
        .map(|config| 8 + 8 * config.entries.len())
        .sum();

    let keys_start = HEADER + 8;
    let ids_start = keys_start + keys_size;
    let records_start = ids_start + ids_size;

    // Every entry's record size, then the offset it lands at. Records follow the entry order.
    let record_size = |value: usize, name: usize| 4 + 8 + 2 + value + 2 + (name + 1);
    let mut offsets: Vec<Vec<usize>> = Vec::with_capacity(index.configs.len());
    let mut cursor = records_start;
    for config in &index.configs {
        let mut per_config = Vec::with_capacity(config.entries.len());
        for entry in &config.entries {
            per_config.push(cursor);
            cursor += record_size(entry.value.len(), entry.name.len());
        }
        offsets.push(per_config);
    }
    let total = cursor;

    let mut out = vec![0u8; total];
    // `version` already carries the whole block text (`Restool 6.1.0.003`).
    let version = index.version.clone();
    let bytes = version.as_bytes();
    out[..bytes.len().min(HEADER)].copy_from_slice(&bytes[..bytes.len().min(HEADER)]);
    write_u32(&mut out, HEADER, total as u32);
    write_u32(&mut out, HEADER + 4, index.configs.len() as u32);

    let mut at = keys_start;
    for (position, config) in index.configs.iter().enumerate() {
        out[at..at + 4].copy_from_slice(b"KEYS");
        write_u32(
            &mut out,
            at + 4,
            (ids_start + ids_size_of(&index.configs[..position])) as u32,
        );
        write_u32(&mut out, at + 8, config.qualifiers.len() as u32);
        let mut qualifier_at = at + 12;
        for qualifier in &config.qualifiers {
            write_u32(&mut out, qualifier_at, qualifier.kind);
            write_u32(&mut out, qualifier_at + 4, qualifier.value);
            qualifier_at += 8;
        }
        at += 12 + 8 * config.qualifiers.len();
    }

    let mut at = ids_start;
    for (position, config) in index.configs.iter().enumerate() {
        out[at..at + 4].copy_from_slice(b"IDSS");
        write_u32(&mut out, at + 4, config.entries.len() as u32);
        let mut row_at = at + 8;
        for (slot, entry) in config.entries.iter().enumerate() {
            write_u32(&mut out, row_at, entry.id);
            write_u32(&mut out, row_at + 4, offsets[position][slot] as u32);
            row_at += 8;
        }
        at += 8 + 8 * config.entries.len();
    }

    for (position, config) in index.configs.iter().enumerate() {
        for (slot, entry) in config.entries.iter().enumerate() {
            let mut at = offsets[position][slot];
            let payload = 8 + 2 + entry.value.len() + 2 + (entry.name.len() + 1);
            write_u32(&mut out, at, payload as u32);
            at += 4;
            write_u32(&mut out, at, entry.kind.code());
            at += 4;
            write_u32(&mut out, at, entry.id);
            at += 4;
            write_u16(&mut out, at, entry.value.len() as u16);
            at += 2;
            out[at..at + entry.value.len()].copy_from_slice(&entry.value);
            at += entry.value.len();
            write_u16(&mut out, at, (entry.name.len() + 1) as u16);
            at += 2;
            out[at..at + entry.name.len()].copy_from_slice(entry.name.as_bytes());
        }
    }

    out
}

fn ids_size_of(configs: &[super::res_index::ConfigIndex]) -> usize {
    configs
        .iter()
        .map(|config| 8 + 8 * config.entries.len())
        .sum()
}

fn write_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
