// compile/abc_records: the record table of a modules.abc, read without the SDK's ark_disasm.
//
// A merged modules.abc is a `PANDA` container: a 60-byte header, a class index (`num_classes`
// u32 offsets at `class_idx_off`), then one class entity per file the merge consumed. Each entity
// opens with its name as a length-prefixed string, and `ark_disasm` prints exactly those names as
// `.record <name>` lines. Reading that index gives the record set of an abc without the SDK tool.
//
// What this cannot answer: it never looks at method code, so opcodes, symbol operands and control
// flow stay unverified. `.scratch/final-verify.sh` used ark_disasm for those axes (symbol multiset
// and mnemonic sequence); this reader is a strictly weaker, record-level check.
//
// Header layout (measured on SDK 26.0.0.23 output, PANDA version 12.0.6.0, 2026-11-08):
//   u8 magic[8] | u32 checksum | u8 version[4] | u32 file_size | u32 foreign_off |
//   u32 foreign_size | u32 num_classes | u32 class_idx_off | u32 num_lnps | u32 lnp_idx_off |
//   u32 num_literalarrays | u32 literalarray_idx_off | u32 num_index_regions | u32 index_section_off
//
// Record names are `uleb128((byte_len << 1) | ascii) + MUTF-8 + '\0'` and carry the class
// descriptor (`L<name>;`); the `L`/`;` are stripped so the name matches the runtime address.
//
// Owning file: the debug information records each compile unit as
// `<carrier>|<package>|<version>|<source path>`, and a record's debug data is written inside that
// record's own class region, so a single such string between this class offset and the next is the
// source file. An abc built without debug information (and the generated ResourceTable record)
// has no compile unit, and a region that names more than one cannot be attributed; those records
// fall back to the module-addressed path the record name itself encodes
// (`&<module>/<path>&[<version>]`), which is what es2abc derived the record from.

use std::path::Path;

/// The length of a PANDA file header (8 magic bytes + 13 u32 fields).
const HEADER_LEN: usize = 60;

/// A merged modules.abc opens with this magic.
const MAGIC: [u8; 8] = *b"PANDA\0\0\0";

/// One record of a merged `modules.abc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbcRecord {
    /// The name the runtime addresses the record by: `&entry/src/main/ets/pages/Index&`,
    /// `@ohos.app`, or a compiler annotation such as `_ESSlotNumberAnnotation`.
    pub(crate) name: String,
    /// Source file: the debug record's path when the abc carries an unambiguous one, otherwise the
    /// module-addressed path inside the record name. `None` for native stubs and annotations.
    pub(crate) file: Option<String>,
}

/// Read every record of one `modules.abc`, in class-index order (the order ark_disasm lists them).
pub(crate) fn read(path: &Path) -> Result<Vec<AbcRecord>, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse(&data).map_err(|e| format!("{}: {e}", path.display()))
}

/// Print one `name<TAB>file` line per record; a file that cannot be read exits non-zero.
pub(crate) fn run(path: &Path) {
    match read(path) {
        Ok(records) => {
            for record in records {
                println!("{}\t{}", record.name, record.file.as_deref().unwrap_or("-"));
            }
        }
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
        }
    }
}

fn parse(data: &[u8]) -> Result<Vec<AbcRecord>, String> {
    if data.len() < HEADER_LEN {
        return Err(format!(
            "short file: {} bytes, the PANDA header alone is {HEADER_LEN}",
            data.len()
        ));
    }
    if data[..8] != MAGIC {
        return Err("not a PANDA bytecode file (magic mismatch)".to_string());
    }
    let file_size = read_u32(data, 0x10)? as usize;
    if file_size > data.len() {
        return Err(format!(
            "header says {file_size} bytes, file has {} (truncated)",
            data.len()
        ));
    }
    let num_classes = read_u32(data, 0x1c)? as usize;
    let class_idx_off = read_u32(data, 0x20)? as usize;
    let index_bytes = num_classes
        .checked_mul(4)
        .ok_or("class count overflows the index size")?;
    let index_end = class_idx_off
        .checked_add(index_bytes)
        .ok_or("class index offset overflows")?;
    if index_end > data.len() {
        return Err(format!(
            "class index at {class_idx_off:#x} with {num_classes} entries runs past the file"
        ));
    }
    let mut offsets = Vec::with_capacity(num_classes);
    for i in 0..num_classes {
        let offset = read_u32(data, class_idx_off + 4 * i)? as usize;
        if offset >= data.len() {
            return Err(format!(
                "class #{i} offset {offset:#x} is past the end of the file"
            ));
        }
        offsets.push(offset);
    }
    let mut sorted = offsets.clone();
    sorted.sort_unstable();
    let mut records = Vec::with_capacity(num_classes);
    for &offset in &offsets {
        let name = record_name(&string_at(data, offset)?);
        let end = next_offset(&sorted, offset).min(data.len());
        // Only `&...&` names come from a source file. Native stubs (`@ohos.*`/`@system.*`) and the
        // compiler's annotation classes never have a compile unit, and the debug data of the
        // generated ResourceTable record can sit in a neighbouring annotation's region.
        let file = if name.starts_with('&') {
            source_file_in(data, offset, end).or_else(|| record_path(&name))
        } else {
            None
        };
        records.push(AbcRecord { name, file });
    }
    Ok(records)
}

/// The next class offset after `offset` in file order, or `usize::MAX` for the last class.
fn next_offset(sorted: &[usize], offset: usize) -> usize {
    match sorted.binary_search(&offset) {
        Ok(i) => sorted.get(i + 1).copied().unwrap_or(usize::MAX),
        Err(i) => sorted.get(i).copied().unwrap_or(usize::MAX),
    }
}

/// One length-prefixed MUTF-8 string item at `offset`.
fn string_at(data: &[u8], offset: usize) -> Result<String, String> {
    let (prefix, start) = varint(data, offset)?;
    let byte_len = (prefix >> 1) as usize;
    let end = start
        .checked_add(byte_len)
        .ok_or("string length overflows the file offset")?;
    if end >= data.len() {
        return Err(format!(
            "string at {offset:#x} runs past the end of the file"
        ));
    }
    if data[end] != 0 {
        return Err(format!("string at {offset:#x} is not NUL-terminated"));
    }
    Ok(String::from_utf8_lossy(&data[start..end]).into_owned())
}

/// ULEB128: the abc's variable-length integer encoding.
fn varint(data: &[u8], mut offset: usize) -> Result<(u32, usize), String> {
    let mut value = 0u32;
    let mut shift = 0u32;
    loop {
        let byte = *data.get(offset).ok_or("truncated uleb128")?;
        offset += 1;
        if shift > 28 || (shift == 28 && byte & 0x7f > 0x0f) {
            return Err("uleb128 value overflows a u32".to_string());
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, offset));
        }
        shift += 7;
    }
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("u32 at {offset:#x} runs past the end of the file"))?
        .try_into()
        .map_err(|_| format!("u32 at {offset:#x} is incomplete"))?;
    Ok(u32::from_le_bytes(bytes))
}

/// `L<name>;` (a class descriptor) → `<name>`; anything else is returned unchanged.
fn record_name(raw: &str) -> String {
    raw.strip_prefix('L')
        .and_then(|name| name.strip_suffix(';'))
        .unwrap_or(raw)
        .to_string()
}

/// The module-addressed source path a record name encodes: `&entry/pages/Index&` →
/// `entry/pages/Index`, `&lib.Index&1.0.0` → `lib.Index`. `None` for non-record classes.
fn record_path(name: &str) -> Option<String> {
    let inner = name.strip_prefix('&')?;
    let path = match inner.find('&') {
        Some(end) => &inner[..end],
        None => inner,
    };
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// The compile unit recorded in `[start, end)`, when the region names exactly one.
fn source_file_in(data: &[u8], start: usize, end: usize) -> Option<String> {
    let mut found: Option<String> = None;
    for at in start..end {
        // Debug strings are separate NUL-terminated items: a candidate starts after the previous
        // item's terminator (or at the class offset itself, whose name is not a source path).
        if at > start && data[at - 1] != 0 {
            continue;
        }
        let Some(path) = source_string_at(data, at, end) else {
            continue;
        };
        match &found {
            // Two compile units in one region: the layout cannot attribute either, so report none.
            Some(previous) if previous != &path => return None,
            _ => found = Some(path),
        }
    }
    found
}

/// `<carrier>|<package>|<version>|<source path>` at `at`, when the item is exactly that shape.
fn source_string_at(data: &[u8], at: usize, end: usize) -> Option<String> {
    let (prefix, start) = varint(data, at).ok()?;
    if prefix & 1 == 0 {
        return None; // non-ASCII string: not a compile-unit path
    }
    let byte_len = (prefix >> 1) as usize;
    if !(4..=300).contains(&byte_len) {
        return None;
    }
    let str_end = start.checked_add(byte_len)?;
    if str_end >= end || data.get(str_end) != Some(&0) {
        return None;
    }
    let text = std::str::from_utf8(&data[start..str_end]).ok()?;
    let mut fields = text.splitn(4, '|');
    let (_carrier, _package, _version) = (fields.next()?, fields.next()?, fields.next()?);
    let path = fields.next()?;
    if path.is_empty() || !has_source_extension(path) {
        return None;
    }
    Some(path.to_string())
}

fn has_source_extension(path: &str) -> bool {
    [".ts", ".ets", ".js"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_abc() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/foreach/golden/modules.abc")
    }

    #[test]
    fn lists_the_records_of_a_golden_abc() {
        let path = golden_abc();
        if !path.exists() {
            return; // fixtures are generated artefacts; a source-only checkout may not carry them
        }
        let records = read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert!(!records.is_empty(), "{} carries no records", path.display());
        let index = records
            .iter()
            .find(|record| record.name == "&entry/src/main/ets/pages/Index&")
            .unwrap_or_else(|| {
                panic!("the Index page record is missing: {records:?}");
            });
        assert_eq!(index.file.as_deref(), Some("src/main/ets/pages/Index.ts"));
    }

    #[test]
    fn rejects_a_file_without_the_panda_magic() {
        assert!(parse(b"not a panda file").is_err());
    }

    #[test]
    fn strips_the_class_descriptor_from_record_names() {
        assert_eq!(
            record_name("L&entry/src/main/ets/pages/Index&;"),
            "&entry/src/main/ets/pages/Index&"
        );
        assert_eq!(record_name("L@ohos.app;"), "@ohos.app");
        assert_eq!(record_name("plain"), "plain");
    }
}
