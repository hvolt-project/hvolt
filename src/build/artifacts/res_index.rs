// res_index: read the `resources.index` restool writes.
//
// restool writes two variants, and both the container and the records differ
// between them; the reader dispatches on the 0x80-byte version block. Everything
// below was measured 2026-11-08 with `xxd`; offsets are absolute.
//
// ── V1: `Restool 6.1.0.003` (single-string probe, 641 bytes) ──
//
//   0x00..0x80  ASCII version, NUL padded ("Restool 6.1.0.003")
//   0x80        u32 total size, equal to the file length
//   0x84        u32 config count
//   per config  "KEYS", u32 ids offset, u32 qualifier count,
//               qualifier count × (u32 kind, u32 packed value)
//   at ids off  "IDSS", u32 id count, id count × (u32 id, u32 record offset)
//   per record  u32 payload length, u32 type code, u32 id,
//               u16 value length + value bytes, u16 name length + name bytes
//
// ── V2: `RestoolV2 6.1.0.003` (951 B native-hello build, 163,557 B HMS
//       previewer, 1,369,226 B OpenHarmony previewer) ──
//
//   0x00..0x80  ASCII version, NUL padded ("RestoolV2 6.1.0.003")
//   0x80        u32 total size, equal to the file length
//   0x84        u32 config count
//   0x88        u32 offset of the DATA block
//   per config  "KEYS", u32 config index, u32 qualifier count,
//               qualifier count × (u32 kind, u32 packed value)
//   "IDSS"      u32 block size (tag and size included), u32 type group count,
//               u32 id count,
//               per type group: u32 type code, u32 group size (the 12-byte
//               header included), u32 id count,
//               per id: u32 id, u32 record offset, u32 name length + name bytes
//   at DATA off "DATA", u32 block size (tag and size included), u32 record
//               count,
//               per record: u32 id, u32 record size (12 + 8 × value count),
//               u32 value count,
//               value count × (u32 config index, u32 value offset)
//   after DATA  a string pool: every value offset points at a u16 length and
//               that many payload bytes; in every sample the pool holds one
//               string per value (no sharing) and runs to the end of the file
//
// So V2 differs in both layers. The container: the header gained the DATA
// offset, a KEYS block now carries its config's index instead of an id table
// offset, the id table became one global block grouped by resource type, and
// values moved into a trailing string pool. The record: V2's IDSS row keeps the
// id, the record offset and the name (the type now lives in the group header),
// and the DATA record only maps config index → pool offset; a config's entry
// list is the fan-out of those pairs.
//
// A qualifier value packs up to four ASCII characters right-aligned in a u32
// (`en` is 0x0000656e), so it has to be shifted out big-endian. V2 stores it the
// same way.
//
// A value payload is what V1 writes after its u16 value length: the text itself
// for text types (V2 does not append the NUL that V1 appends), and a
// NUL-separated `key\0value\0…` table for theme and plural. The measured V2
// indexes carry type codes 9, 14, 16, 17, 18, 19, 20, 22 and 23 only; 8, 10, 11
// and 12 (integer, str/int array, boolean) never showed up, so their V2 payload
// shape is unverified — this reader stores whatever the pool holds, verbatim.
//
// This is the reader half only: it validates an existing index and gives the
// id allocator / qualifier index / writer something to compare against.

#![allow(dead_code)] // the writer that consumes this reader is not wired in yet

use std::collections::{BTreeSet, HashMap};

/// The fixed version block at the start of every index.
const HEADER_SIZE: usize = 0x80;

/// One resource type as restool encodes it.
///
/// V1 keeps the code in the record; V2 keeps it in the `IDSS` type group header.
/// The V2 previewer indexes also use codes 22 (`ohos_corner_mark_pattern`, values
/// look like a `key\0$float:<id>\0…` table) and 23 (`earphone_16662_right`,
/// values look like `0xF1252` glyph ids and media paths); their resource names
/// are not established, so they stay [`Self::Other`] with the code preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceType {
    Integer,
    String,
    StrArray,
    IntArray,
    Boolean,
    Color,
    Theme,
    Plural,
    Float,
    Media,
    Profile,
    /// A code this reader has not seen; preserved so a caller can still report it.
    Other(u32),
}

impl ResourceType {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            8 => Self::Integer,
            9 => Self::String,
            10 => Self::StrArray,
            11 => Self::IntArray,
            12 => Self::Boolean,
            14 => Self::Color,
            16 => Self::Theme,
            17 => Self::Plural,
            18 => Self::Float,
            19 => Self::Media,
            20 => Self::Profile,
            other => Self::Other(other),
        }
    }

    /// The numeric code, for comparing against a table restool prints.
    pub(crate) fn code(self) -> u32 {
        match self {
            Self::Integer => 8,
            Self::String => 9,
            Self::StrArray => 10,
            Self::IntArray => 11,
            Self::Boolean => 12,
            Self::Color => 14,
            Self::Theme => 16,
            Self::Plural => 17,
            Self::Float => 18,
            Self::Media => 19,
            Self::Profile => 20,
            Self::Other(code) => code,
        }
    }

    /// Whether the value is a single NUL-terminated string. Theme, plural and the
    /// array types carry a nested length-prefixed table instead.
    pub(crate) fn is_text(self) -> bool {
        matches!(
            self,
            Self::Integer
                | Self::String
                | Self::Boolean
                | Self::Color
                | Self::Float
                | Self::Media
                | Self::Profile
        )
    }
}

/// One qualifier component of a config (`en`, `US`, color mode, …).
///
/// Observed kinds: 0 = language, 1 = region (together `en_US`), 6 = color mode
/// (`dark` is value 0). Other kinds are preserved raw until a build exercises them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Qualifier {
    pub kind: u32,
    pub value: u32,
}

impl Qualifier {
    /// The packed characters, e.g. `en` for 0x0000656e.
    pub(crate) fn text(self) -> String {
        unpack_chars(self.value)
    }
}

/// One id → record entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexEntry {
    pub id: u32,
    pub kind: ResourceType,
    /// The resource name (`app_name`), without the trailing NUL.
    pub name: String,
    /// The encoded value, NUL included: a string/path for text types, a nested
    /// length-prefixed table for theme/plural/arrays.
    pub value: Vec<u8>,
}

impl IndexEntry {
    /// The value of a text-typed entry, without the trailing NUL.
    pub(crate) fn text_value(&self) -> Option<&str> {
        if !self.kind.is_text() {
            return None;
        }
        let bytes = self.value.strip_suffix(&[0]).unwrap_or(&self.value);
        std::str::from_utf8(bytes).ok()
    }
}

/// The entries of one qualifier configuration; the base config carries none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigIndex {
    pub qualifiers: Vec<Qualifier>,
    pub entries: Vec<IndexEntry>,
}

/// A parsed `resources.index`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceIndex {
    pub version: String,
    pub configs: Vec<ConfigIndex>,
}

impl ResourceIndex {
    /// Every id slot in the file, qualifier configs included. A localized entry
    /// repeats the base id, so this exceeds [`Self::unique_id_count`] when the
    /// module has qualifier-specific resources.
    pub(crate) fn entry_count(&self) -> usize {
        self.configs.iter().map(|config| config.entries.len()).sum()
    }

    /// Distinct resource ids.
    pub(crate) fn unique_id_count(&self) -> usize {
        self.configs
            .iter()
            .flat_map(|config| config.entries.iter().map(|entry| entry.id))
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// The entry for `id` in the config with no qualifiers.
    pub(crate) fn base_entry(&self, id: u32) -> Option<&IndexEntry> {
        self.configs
            .iter()
            .find(|config| config.qualifiers.is_empty())
            .and_then(|config| config.entries.iter().find(|entry| entry.id == id))
    }
}

/// Parse a `resources.index` produced by restool, dispatching on the version
/// block: `RestoolV2 ` is the type-grouped layout, `Restool ` the older one.
pub(crate) fn parse(data: &[u8]) -> Result<ResourceIndex, String> {
    let version = read_version(data)?;
    if version.starts_with("RestoolV2 ") {
        return parse_v2(data, version);
    }
    if version.starts_with("Restool ") {
        return parse_v1(data, version);
    }
    Err(format!(
        "not a restool resources.index: version block is `{version}`"
    ))
}

/// Parse the V1 (`Restool `) layout: one id table per config, everything the
/// reader needs inside each record.
fn parse_v1(data: &[u8], version: String) -> Result<ResourceIndex, String> {
    let mut cursor = Cursor::new(data, HEADER_SIZE);
    let total_size = cursor.u32()? as usize;
    if total_size != data.len() {
        return Err(format!(
            "resources.index declares {total_size} bytes but is {} bytes",
            data.len()
        ));
    }
    let config_count = count(cursor.u32()?, data.len(), "config")?;
    let mut headers = Vec::with_capacity(config_count);
    for _ in 0..config_count {
        cursor.expect_tag(b"KEYS")?;
        let ids_offset = cursor.u32()? as usize;
        let qualifier_count = count(cursor.u32()?, data.len(), "qualifier")?;
        let mut qualifiers = Vec::with_capacity(qualifier_count);
        for _ in 0..qualifier_count {
            qualifiers.push(Qualifier {
                kind: cursor.u32()?,
                value: cursor.u32()?,
            });
        }
        headers.push((ids_offset, qualifiers));
    }
    let mut configs = Vec::with_capacity(config_count);
    for (ids_offset, qualifiers) in headers {
        configs.push(ConfigIndex {
            qualifiers,
            entries: read_id_block(data, ids_offset)?,
        });
    }
    Ok(ResourceIndex { version, configs })
}

/// Parse the V2 (`RestoolV2 `) layout.
///
/// Every structural error is reported with the version, so a caller that only
/// knows "some resources.index failed" still learns which variant it claimed.
fn parse_v2(data: &[u8], version: String) -> Result<ResourceIndex, String> {
    match parse_v2_body(data) {
        Ok(configs) => Ok(ResourceIndex { version, configs }),
        Err(error) => Err(format!("resources.index `{version}`: {error}")),
    }
}

fn parse_v2_body(data: &[u8]) -> Result<Vec<ConfigIndex>, String> {
    let mut cursor = Cursor::new(data, HEADER_SIZE);
    let total_size = cursor.u32()? as usize;
    if total_size != data.len() {
        return Err(format!(
            "declares {total_size} bytes but is {} bytes",
            data.len()
        ));
    }
    let config_count = count(cursor.u32()?, data.len(), "config")?;
    let data_offset = cursor.u32()? as usize;
    // The three tables follow each other in fixed order: KEYS blocks, IDSS,
    // DATA. Only DATA's offset is stored, IDSS is found by reading the KEYS
    // blocks to their end.
    let mut configs = read_v2_configs(data, &mut cursor, config_count)?;
    let rows = read_v2_id_groups(data, &mut cursor, data_offset)?;
    let (records, pool_start) = read_v2_records(data, data_offset)?;
    fill_v2_entries(&mut configs, rows, records, data, pool_start)?;
    Ok(configs)
}

/// The `KEYS` blocks of the V2 layout, one per config.
///
/// Each block stores the config index its `DATA` values refer to, and the blocks
/// are *not* written in index order: the OpenHarmony previewer index starts at
/// index 75 and leaves the qualifier-free base config at index 4. They are
/// therefore placed by the index they carry.
fn read_v2_configs(
    data: &[u8],
    cursor: &mut Cursor<'_>,
    config_count: usize,
) -> Result<Vec<ConfigIndex>, String> {
    let mut slots: Vec<Option<ConfigIndex>> = vec![None; config_count];
    for _ in 0..config_count {
        let at = cursor.pos;
        cursor.expect_tag(b"KEYS")?;
        let index = cursor.u32()? as usize;
        let qualifier_count = count(cursor.u32()?, data.len(), "qualifier")?;
        let mut qualifiers = Vec::with_capacity(qualifier_count);
        for _ in 0..qualifier_count {
            qualifiers.push(Qualifier {
                kind: cursor.u32()?,
                value: cursor.u32()?,
            });
        }
        let slot = slots.get_mut(index).ok_or_else(|| {
            format!("KEYS block at {at:#x} carries config index {index}, past the {config_count} declared")
        })?;
        if slot.is_some() {
            return Err(format!(
                "two KEYS blocks claim config index {index} (second at {at:#x})"
            ));
        }
        *slot = Some(ConfigIndex {
            qualifiers,
            entries: Vec::new(),
        });
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(index, config)| {
            config.ok_or_else(|| format!("no KEYS block carries config index {index}"))
        })
        .collect()
}

/// One `IDSS` row: the id, its name (inline in V2, unlike V1), the type its
/// group declares, and where the id's `DATA` record sits.
struct V2Row {
    kind: ResourceType,
    id: u32,
    record_offset: usize,
    name: String,
}

/// The V2 `IDSS` block: one sized, type-grouped table over every id.
fn read_v2_id_groups(
    data: &[u8],
    cursor: &mut Cursor<'_>,
    data_offset: usize,
) -> Result<Vec<V2Row>, String> {
    let block_offset = cursor.pos;
    cursor.expect_tag(b"IDSS")?;
    let block_size = cursor.u32()? as usize;
    let block_end = block_end(block_offset, block_size, data.len(), "IDSS")?;
    if block_end != data_offset {
        return Err(format!(
            "IDSS block ends at {block_end:#x} but the header puts DATA at {data_offset:#x}"
        ));
    }
    let group_count = count(cursor.u32()?, data.len(), "type group")?;
    let id_count = count(cursor.u32()?, data.len(), "id")?;
    let mut rows = Vec::with_capacity(id_count);
    for _ in 0..group_count {
        let group_offset = cursor.pos;
        let kind = ResourceType::from_code(cursor.u32()?);
        let group_size = cursor.u32()? as usize;
        let row_count = count(cursor.u32()?, data.len(), "id")?;
        for _ in 0..row_count {
            let id = cursor.u32()?;
            let record_offset = cursor.u32()? as usize;
            let name = cursor.string32()?;
            rows.push(V2Row {
                kind,
                id,
                record_offset,
                name,
            });
        }
        let spanned = cursor.pos - group_offset;
        if spanned != group_size {
            return Err(format!(
                "type group {} at {group_offset:#x} declares {group_size} bytes but spans {spanned}",
                kind.code()
            ));
        }
    }
    if cursor.pos != block_end {
        return Err(format!(
            "IDSS block declares {block_size} bytes but its contents span {}",
            cursor.pos - block_offset
        ));
    }
    if rows.len() != id_count {
        return Err(format!(
            "IDSS declares {id_count} ids but its groups carry {}",
            rows.len()
        ));
    }
    Ok(rows)
}

/// One V2 `DATA` record: where it sits, and for every config that overrides the
/// id, the string pool offset of that config's value.
struct V2Record {
    offset: usize,
    /// `(config index, string pool offset)`.
    values: Vec<(usize, usize)>,
}

/// The V2 `DATA` block, keyed by id; also returns where the string pool starts.
fn read_v2_records(
    data: &[u8],
    data_offset: usize,
) -> Result<(HashMap<u32, V2Record>, usize), String> {
    let mut cursor = Cursor::new(data, data_offset);
    cursor.expect_tag(b"DATA")?;
    let block_size = cursor.u32()? as usize;
    let block_end = block_end(data_offset, block_size, data.len(), "DATA")?;
    let record_count = count(cursor.u32()?, data.len(), "record")?;
    let mut records = HashMap::with_capacity(record_count);
    for _ in 0..record_count {
        let offset = cursor.pos;
        let id = cursor.u32()?;
        let record_size = cursor.u32()? as usize;
        let value_count = count(cursor.u32()?, data.len(), "value")?;
        let expected = 12 + 8 * value_count;
        if record_size != expected {
            return Err(format!(
                "record at {offset:#x} declares {record_size} bytes for {value_count} values, expected {expected}"
            ));
        }
        let mut values = Vec::with_capacity(value_count);
        for _ in 0..value_count {
            let config = cursor.u32()? as usize;
            let value_offset = cursor.u32()? as usize;
            values.push((config, value_offset));
        }
        if records.insert(id, V2Record { offset, values }).is_some() {
            return Err(format!("two DATA records carry id {id:#010x}"));
        }
    }
    if cursor.pos != block_end {
        return Err(format!(
            "DATA block declares {block_size} bytes but its contents span {}",
            cursor.pos - data_offset
        ));
    }
    Ok((records, block_end))
}

/// Join the `IDSS` rows to their `DATA` records and fan each value out into the
/// config it belongs to.
///
/// V2 has no per-config entry list on disk, so a config's entries come out in
/// `IDSS` order (groups by type code, ids within a group as restool wrote them),
/// which is also how a V1 `IDSS` block orders them.
fn fill_v2_entries(
    configs: &mut [ConfigIndex],
    rows: Vec<V2Row>,
    mut records: HashMap<u32, V2Record>,
    data: &[u8],
    pool_start: usize,
) -> Result<(), String> {
    let config_count = configs.len();
    for row in rows {
        let record = records
            .remove(&row.id)
            .ok_or_else(|| format!("IDSS row for id {:#010x} has no DATA record", row.id))?;
        if record.offset != row.record_offset {
            return Err(format!(
                "IDSS row for id {:#010x} points at {:#x} but its DATA record is at {:#x}",
                row.id, row.record_offset, record.offset
            ));
        }
        for (index, value_offset) in record.values {
            let value = read_pool_value(data, value_offset, pool_start)?;
            let config = configs.get_mut(index).ok_or_else(|| {
                format!(
                    "id {:#010x} carries a value for config index {index} of {config_count}",
                    row.id
                )
            })?;
            config.entries.push(IndexEntry {
                id: row.id,
                kind: row.kind,
                name: row.name.clone(),
                value,
            });
        }
    }
    if let Some((id, _)) = records.into_iter().next() {
        return Err(format!("DATA record for id {id:#010x} has no IDSS row"));
    }
    Ok(())
}

/// A V2 value: a u16 length at `offset`, then that many payload bytes.
fn read_pool_value(data: &[u8], offset: usize, pool_start: usize) -> Result<Vec<u8>, String> {
    if offset < pool_start {
        return Err(format!(
            "value offset {offset:#x} is inside the DATA block (the string pool starts at {pool_start:#x})"
        ));
    }
    let mut cursor = Cursor::new(data, offset);
    let len = cursor.u16()? as usize;
    Ok(cursor.take(len)?.to_vec())
}

/// The NUL-terminated version string in the fixed header block.
fn read_version(data: &[u8]) -> Result<String, String> {
    if data.len() < HEADER_SIZE {
        return Err(format!(
            "resources.index is {} bytes, shorter than the {HEADER_SIZE}-byte header",
            data.len()
        ));
    }
    let end = data[..HEADER_SIZE]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(HEADER_SIZE);
    std::str::from_utf8(&data[..end])
        .map(str::to_owned)
        .map_err(|error| format!("version block is not UTF-8: {error}"))
}

/// An `IDSS` block: one id and one absolute record offset per entry.
fn read_id_block(data: &[u8], offset: usize) -> Result<Vec<IndexEntry>, String> {
    let mut cursor = Cursor::new(data, offset);
    cursor.expect_tag(b"IDSS")?;
    let entry_count = count(cursor.u32()?, data.len(), "id")?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let id = cursor.u32()?;
        let record_offset = cursor.u32()? as usize;
        entries.push(read_record(data, record_offset, id)?);
    }
    Ok(entries)
}

/// One record: payload length, type, id, then the value and name strings.
fn read_record(data: &[u8], offset: usize, id: u32) -> Result<IndexEntry, String> {
    let mut cursor = Cursor::new(data, offset);
    let payload_len = cursor.u32()? as usize;
    let end = offset
        .checked_add(4)
        .and_then(|start| start.checked_add(payload_len))
        .ok_or_else(|| format!("record at {offset:#x}: payload length overflows"))?;
    if end > data.len() {
        return Err(format!(
            "record at {offset:#x} ends at {end:#x}, past the {} byte file",
            data.len()
        ));
    }
    let kind = ResourceType::from_code(cursor.u32()?);
    let record_id = cursor.u32()?;
    if record_id != id {
        return Err(format!(
            "record at {offset:#x} carries id {record_id:#010x}, its index entry says {id:#010x}"
        ));
    }
    let value_len = cursor.u16()? as usize;
    let value = cursor.take(value_len)?.to_vec();
    let name = cursor.string()?;
    if cursor.pos != end {
        return Err(format!(
            "record at {offset:#x} declares {payload_len} payload bytes but consumed {}",
            cursor.pos - offset - 4
        ));
    }
    Ok(IndexEntry {
        id,
        kind,
        name,
        value,
    })
}

/// Unpack the right-aligned ASCII characters of a qualifier value.
fn unpack_chars(value: u32) -> String {
    let bytes = [
        ((value >> 24) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ];
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(4);
    let end = bytes[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|at| start + at)
        .unwrap_or(4);
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

/// Reject a count that cannot fit in the file before allocating for it.
fn count(value: u32, len: usize, what: &str) -> Result<usize, String> {
    let value = value as usize;
    if value > len {
        return Err(format!("{what} count {value} exceeds the {len} byte file"));
    }
    Ok(value)
}

/// The end of a V2 sized block (its size field counts the 8-byte tag and size).
fn block_end(offset: usize, size: usize, len: usize, what: &str) -> Result<usize, String> {
    if size < 8 {
        return Err(format!(
            "{what} block at {offset:#x} declares only {size} bytes"
        ));
    }
    let end = offset
        .checked_add(size)
        .ok_or_else(|| format!("{what} block at {offset:#x} overflows"))?;
    if end > len {
        return Err(format!(
            "{what} block at {offset:#x} ends at {end:#x}, past the {len} byte file"
        ));
    }
    Ok(end)
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| format!("offset {} + {len} overflows", self.pos))?;
        if end > self.data.len() {
            return Err(format!(
                "truncated at offset {:#x}: need {len} bytes, {} left",
                self.pos,
                self.data.len() - self.pos
            ));
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn expect_tag(&mut self, expected: &[u8; 4]) -> Result<(), String> {
        let at = self.pos;
        let found = self.take(4)?;
        if found != expected {
            return Err(format!(
                "expected tag `{}` at {at:#x}, found `{}`",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(found)
            ));
        }
        Ok(())
    }

    /// A u16 length followed by that many bytes, NUL stripped.
    fn string(&mut self) -> Result<String, String> {
        let len = self.u16()? as usize;
        self.length_prefixed_string(len)
    }

    /// A u32 length followed by that many bytes, NUL stripped. V2 names use this
    /// width; V1 records use the u16 [`Self::string`].
    fn string32(&mut self) -> Result<String, String> {
        let len = self.u32()? as usize;
        self.length_prefixed_string(len)
    }

    fn length_prefixed_string(&mut self, len: usize) -> Result<String, String> {
        let at = self.pos;
        let bytes = self.take(len)?;
        let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| format!("string at {at:#x} is not UTF-8: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// The resources.index restool 6.1.0.003 wrote for the minimal probe
    /// (base: four strings, one color, one media, the generated main_pages
    /// profile; one `en_US` string), recorded 2026-11-08.
    const SAMPLE_HEX: &str = "\
        526573746f6f6c20362e312e302e30303300000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000081020000020000004b455953b0000000\
        000000004b455953f000000002000000000000006e650000010000005355000049445353\
        07000000000000010001000001000001200100000200000161010000030000018f010000\
        04000001b901000005000001e80100000600000118020000494453530100000000000001\
        5e0200001c000000090000000000000107007265736d696e0009006170705f6e616d6500\
        3d000000130000000100000127007372632f7265736f75726365732f626173652f6d6564\
        69612f737461727449636f6e2e706e67000a00737461727449636f6e002a000000090000\
        00020000010c006465736372697074696f6e001200456e7472794162696c6974795f6465\
        73630026000000090000000300000107007265736d696e001300456e7472794162696c69\
        74795f6c6162656c002b000000090000000400000113006d6f64756c6520646573637269\
        7074696f6e000c006d6f64756c655f64657363002c0000000e0000000500000108002346\
        464646464600180073746172745f77696e646f775f6261636b67726f756e640042000000\
        14000000060000012b007372632f7265736f75726365732f626173652f70726f66696c65\
        2f6d61696e5f70616765732e6a736f6e000b006d61696e5f7061676573001f0000000900\
        0000000000010a007265736d696e20656e0009006170705f6e616d6500\
        ";

    #[test]
    fn dump_raw_value_bytes_named_by_the_environment() {
        let (Ok(path), Ok(name)) = (
            std::env::var("HVOLT_INDEX_DUMP"),
            std::env::var("HVOLT_INDEX_ENTRY"),
        ) else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read the index");
        let index = super::parse(&bytes).expect("parse the index");
        for config in &index.configs {
            for entry in &config.entries {
                if entry.name == name {
                    println!("{} {:08x} {:?}", entry.name, entry.id, entry.value);
                }
            }
        }
    }

    #[test]
    fn dump_entries_named_by_the_environment() {
        let Ok(path) = std::env::var("HVOLT_INDEX_DUMP") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read the index");
        let index = super::parse(&bytes).expect("parse the index");
        for (position, config) in index.configs.iter().enumerate() {
            println!("config {position} qualifiers={:?}", config.qualifiers);
            for entry in &config.entries {
                println!(
                    "  {:08x} {:?} {} = {:?}",
                    entry.id,
                    entry.kind,
                    entry.name,
                    String::from_utf8_lossy(&entry.value)
                );
            }
        }
    }

    #[test]
    fn write_reproduces_any_index_named_by_the_environment() {
        let Ok(path) = std::env::var("HVOLT_INDEX_ROUNDTRIP") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read the index");
        let index = super::parse(&bytes).expect("parse the index");
        let again = crate::build::artifacts::res_index_write::write(&index);
        assert_eq!(again.len(), bytes.len(), "size of {path}");
        assert_eq!(again, bytes, "round trip of {path}");
    }

    #[test]
    fn write_reproduces_the_v1_sample_byte_for_byte() {
        let bytes = sample();
        let index = super::parse(&bytes).expect("parse the sample");
        let again = crate::build::artifacts::res_index_write::write(&index);
        assert_eq!(again.len(), bytes.len(), "size");
        assert_eq!(again, bytes, "round trip");
    }

    fn sample() -> Vec<u8> {
        assert!(SAMPLE_HEX.len() % 2 == 0);
        (0..SAMPLE_HEX.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&SAMPLE_HEX[at..at + 2], 16).expect("hex byte"))
            .collect()
    }

    #[test]
    fn parses_a_real_restool_index() {
        let bytes = sample();
        assert_eq!(bytes.len(), 641);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "7d020ee55caa29693eb69457e42102892731ff25d698ec5d7fd9daf1d52f35ae"
        );
        let index = parse(&bytes).expect("parse");
        assert_eq!(index.version, "Restool 6.1.0.003");
        assert_eq!(index.configs.len(), 2);
        assert_eq!(index.entry_count(), 8);
        assert_eq!(index.unique_id_count(), 7);

        let base = &index.configs[0];
        assert!(base.qualifiers.is_empty());
        assert_eq!(base.entries.len(), 7);
        let app_name = index.base_entry(0x0100_0000).expect("app_name");
        assert_eq!(app_name.name, "app_name");
        assert_eq!(app_name.kind, ResourceType::String);
        assert_eq!(app_name.text_value(), Some("resmin"));
        let icon = index.base_entry(0x0100_0001).expect("startIcon");
        assert_eq!(icon.kind, ResourceType::Media);
        assert_eq!(
            icon.text_value(),
            Some("src/resources/base/media/startIcon.png")
        );
        let pages = index.base_entry(0x0100_0006).expect("main_pages");
        assert_eq!(pages.kind, ResourceType::Profile);
        assert_eq!(
            pages.text_value(),
            Some("src/resources/base/profile/main_pages.json")
        );

        let localized = &index.configs[1];
        assert_eq!(
            localized
                .qualifiers
                .iter()
                .map(|qualifier| qualifier.text())
                .collect::<Vec<_>>(),
            ["en", "US"]
        );
        assert_eq!(localized.entries.len(), 1);
        assert_eq!(localized.entries[0].id, 0x0100_0000);
        assert_eq!(localized.entries[0].text_value(), Some("resmin en"));

        println!(
            "version={} configs={} entries={} unique_ids={} bytes={}",
            index.version,
            index.configs.len(),
            index.entry_count(),
            index.unique_id_count(),
            bytes.len()
        );
        for entry in &base.entries {
            println!(
                "  {:#010x} type={:<2} name={}",
                entry.id,
                entry.kind.code(),
                entry.name
            );
        }
    }

    /// A second product from an existing build, kept as a fixture: it carries a
    /// float and only the base config.
    #[test]
    fn parses_a_fixture_index_with_a_float() {
        let bytes = include_bytes!("../../../fixtures/v2-child-args/golden/resources.index");
        let index = parse(bytes).expect("parse");
        assert_eq!(index.configs.len(), 1);
        assert_eq!(index.entry_count(), 11);
        let float = index.base_entry(0x0100_0008).expect("example_float");
        assert_eq!(float.kind, ResourceType::Float);
        assert_eq!(float.text_value(), Some("1.0"));
    }

    #[test]
    fn rejects_a_truncated_index() {
        let mut bytes = sample();
        bytes.truncate(200);
        let error = parse(&bytes).expect_err("truncated index must fail");
        assert!(error.contains("declares 641 bytes"), "{error}");
    }

    /// The V2 index restool wrote for the minimal `examples/native-hello` build:
    /// one base config, ten ids in four type groups, values in the trailing
    /// string pool. Recorded 2026-11-08, 951 bytes.
    const SAMPLE_V2: &[u8] =
        include_bytes!("../../../fixtures/v2-index/native-hello/resources.index");

    #[test]
    fn parses_a_real_restool_v2_index() {
        let bytes = SAMPLE_V2;
        assert_eq!(bytes.len(), 951);
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "9a04f8ade2b49cd5768faefb9aebb17270eb2173e3b22d4d3a2bd2dc8d660001"
        );
        let index = parse(bytes).expect("parse");
        assert_eq!(index.version, "RestoolV2 6.1.0.003");
        assert_eq!(index.configs.len(), 1);
        assert_eq!(index.entry_count(), 10);
        assert_eq!(index.unique_id_count(), 10);

        let base = &index.configs[0];
        assert!(base.qualifiers.is_empty());
        // V2 writes the ids grouped by type code instead of by id: profile (20),
        // color (14), media (19) ×4, string (9) ×4.
        assert_eq!(
            base.entries
                .iter()
                .map(|entry| (entry.id, entry.kind.code(), entry.name.as_str()))
                .collect::<Vec<_>>(),
            [
                (0x0100_0009, 20, "main_pages"),
                (0x0100_0008, 14, "start_window_background"),
                (0x0100_0004, 19, "startIcon"),
                (0x0100_0003, 19, "layered_image"),
                (0x0100_0002, 19, "foreground"),
                (0x0100_0001, 19, "background"),
                (0x0100_0007, 9, "module_desc"),
                (0x0100_0006, 9, "EntryAbility_label"),
                (0x0100_0005, 9, "EntryAbility_desc"),
                (0x0100_0000, 9, "app_name"),
            ]
        );

        let app_name = index.base_entry(0x0100_0000).expect("app_name");
        assert_eq!(app_name.kind, ResourceType::String);
        assert_eq!(app_name.text_value(), Some("native-hello"));
        // V2 stores a text value without the NUL byte V1 appends.
        assert_eq!(app_name.value, b"native-hello");
        let icon = index.base_entry(0x0100_0004).expect("startIcon");
        assert_eq!(icon.kind, ResourceType::Media);
        assert_eq!(
            icon.text_value(),
            Some("src/resources/base/media/startIcon.png")
        );
        let pages = index.base_entry(0x0100_0009).expect("main_pages");
        assert_eq!(pages.kind, ResourceType::Profile);
        assert_eq!(
            pages.text_value(),
            Some("src/resources/base/profile/main_pages.json")
        );
        let color = index
            .base_entry(0x0100_0008)
            .expect("start_window_background");
        assert_eq!(color.kind, ResourceType::Color);
        assert_eq!(color.text_value(), Some("#FFFFFF"));

        println!(
            "v2 version={} configs={} entries={} unique_ids={} bytes={}",
            index.version,
            index.configs.len(),
            index.entry_count(),
            index.unique_id_count(),
            bytes.len()
        );
        for entry in &base.entries {
            println!(
                "  {:#010x} type={:<2} name={}",
                entry.id,
                entry.kind.code(),
                entry.name
            );
        }
    }

    /// The OpenHarmony previewer index DevEco Studio ships: 76 configs, 7,636 ids
    /// and 25,997 per-config values. The SDK is not part of the repository and its
    /// path moved between releases, so try both known locations and skip when
    /// neither exists.
    #[test]
    fn parses_the_dev_eco_v2_index_when_present() {
        const CANDIDATES: [&str; 2] = [
            "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/previewer/common/bin/resources.index",
            "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/previewer/common/resources/resources.index",
        ];
        let found = CANDIDATES
            .iter()
            .find_map(|path| std::fs::read(path).ok().map(|bytes| (*path, bytes)));
        let Some((path, bytes)) = found else {
            println!("skipped: no DevEco previewer resources.index under /Applications");
            return;
        };
        let index = parse(&bytes).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(index.version, "RestoolV2 6.1.0.003");
        let base = index
            .configs
            .iter()
            .find(|config| config.qualifiers.is_empty())
            .expect("a config with no qualifiers");
        println!(
            "{path}: bytes={} configs={} entries={} unique_ids={} base_entries={}",
            bytes.len(),
            index.configs.len(),
            index.entry_count(),
            index.unique_id_count(),
            base.entries.len()
        );
        for entry in base.entries.iter().take(5) {
            println!(
                "  {:#010x} type={:<2} name={}",
                entry.id,
                entry.kind.code(),
                entry.name
            );
        }
        // The build measured on 2026-11-08; a newer SDK may legitimately differ.
        if bytes.len() == 1_369_226 {
            assert_eq!(index.configs.len(), 76);
            assert_eq!(index.unique_id_count(), 7_636);
            assert_eq!(index.entry_count(), 25_997);
            assert_eq!(base.entries.len(), 7_629);
            // The base config is not config 0 in V2 (it is index 4 here), so the
            // reader has to place configs by the index the KEYS block carries.
            assert!(index.configs[4].qualifiers.is_empty());
        }
    }

    /// A V1 body behind a V2 version block must be rejected, not misread: the
    /// two containers disagree from the third header word on.
    #[test]
    fn rejects_a_v1_body_under_a_v2_version_block() {
        let mut bytes = sample();
        bytes[..10].copy_from_slice(b"RestoolV2 ");
        let error = parse(&bytes).expect_err("a V1 body is not valid V2");
        assert!(error.contains("RestoolV2"), "{error}");
    }

    #[test]
    fn rejects_a_truncated_v2_index() {
        let mut bytes = SAMPLE_V2.to_vec();
        bytes.truncate(600);
        let error = parse(&bytes).expect_err("a truncated V2 index must fail");
        assert!(error.contains("declares 951 bytes"), "{error}");
    }

    #[test]
    fn unpacks_qualifier_characters_big_endian() {
        assert_eq!(unpack_chars(0x0000_656e), "en");
        assert_eq!(unpack_chars(0x0000_7a68), "zh");
        assert_eq!(unpack_chars(0x0000_5553), "US");
        assert_eq!(unpack_chars(0), "");
    }
}
