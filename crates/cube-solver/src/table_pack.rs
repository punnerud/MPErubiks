//! MPEE/matcodec-inspired packed solver table (M7).
//!
//! kewb's `table.bin` is a 6.8 MB bincode stream. Shaping each section to
//! its value distribution BEFORE the entropy coder sees it — nibble-packed
//! pruning depths (0..13); u16 move tables as per-column DELTAS along the
//! coordinate order (route compression: the positional/factorial number
//! systems make neighboring rows nearly equal, measured 11x on cp) in
//! lo/hi byte planes — then deflating per section lands at ~0.9 MB (13%),
//! a third of transport-gzip on the raw file, and unpacks in ~15 ms into
//! the exact same in-memory `DataTable` (solve times untouched by
//! design). Deflate because `miniz_oxide` inflates it dependency-free on
//! wasm.

use kewb::{move_table::MoveTable, pruning_table::PruningTable, DataTable};

const MAGIC: &[u8; 6] = b"KWPK2\0";
/// Sections in file order: six u16 move tables, four u8 pruning tables.
const U16_SECTIONS: usize = 6;
const SECTIONS: usize = 10;

fn u16_fields(t: &DataTable) -> [&Vec<Vec<u16>>; U16_SECTIONS] {
    let m = &t.move_table;
    [&m.co, &m.eo, &m.e_combo, &m.cp, &m.ep, &m.e_ep]
}

fn u8_fields(t: &DataTable) -> [&Vec<Vec<u8>>; SECTIONS - U16_SECTIONS] {
    let p = &t.pruning_table;
    [&p.co_e, &p.eo_e, &p.cp_e, &p.ep_e]
}

/// Pack a decoded table (build-time tool path; the app only decodes).
pub fn encode_packed(t: &DataTable) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let mut push_section = |rows: u32, row_len: u32, transformed: &[u8]| {
        let packed = miniz_oxide::deflate::compress_to_vec(transformed, 10);
        out.extend_from_slice(&rows.to_le_bytes());
        out.extend_from_slice(&row_len.to_le_bytes());
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&packed);
    };
    for table in u16_fields(t) {
        let row_len = table.first().map_or(0, Vec::len);
        assert!(table.iter().all(|r| r.len() == row_len), "uniform rows");
        // ROUTE COMPRESSION (KWPK2): kewb's coordinate enumeration is a
        // near-optimal "route" through the rows (neighboring indices in
        // the factorial/positional number systems differ in few digits),
        // so we store per-column DELTAS along it — 11x smaller than the
        // positions themselves — then lo/hi byte planes.
        let n = table.len() * row_len;
        let mut prev = vec![0u16; row_len];
        let mut planes = vec![0u8; n * 2];
        let mut i = 0;
        for row in table {
            for (j, &v) in row.iter().enumerate() {
                let d = v.wrapping_sub(prev[j]);
                let [lo, hi] = d.to_le_bytes();
                planes[i] = lo;
                planes[n + i] = hi;
                prev[j] = v;
                i += 1;
            }
        }
        push_section(table.len() as u32, row_len as u32, &planes);
    }
    for table in u8_fields(t) {
        let row_len = table.first().map_or(0, Vec::len);
        assert!(table.iter().all(|r| r.len() == row_len), "uniform rows");
        // Depths are 0..13: two per byte.
        let flat: Vec<u8> = table.iter().flatten().copied().collect();
        let mut packed = Vec::with_capacity(flat.len() / 2 + 1);
        for pair in flat.chunks(2) {
            let lo = pair[0] & 0x0F;
            let hi = pair.get(1).copied().unwrap_or(0) & 0x0F;
            packed.push(lo | (hi << 4));
        }
        push_section(table.len() as u32, row_len as u32, &packed);
    }
    out
}

pub fn decode_packed(bytes: &[u8]) -> Result<DataTable, String> {
    let mut pos = MAGIC.len();
    if bytes.len() < pos || &bytes[..pos] != MAGIC {
        return Err("not a packed solver table (bad magic)".into());
    }
    let mut header = |bytes: &[u8], pos: &mut usize| -> Result<(usize, usize, usize), String> {
        let take = |p: &mut usize| -> Result<u32, String> {
            let v = bytes
                .get(*p..*p + 4)
                .ok_or("truncated header")?
                .try_into()
                .map(u32::from_le_bytes)
                .map_err(|_| "truncated header")?;
            *p += 4;
            Ok(v)
        };
        Ok((
            take(pos)? as usize,
            take(pos)? as usize,
            take(pos)? as usize,
        ))
    };
    let mut u16_tables: Vec<Vec<Vec<u16>>> = Vec::with_capacity(U16_SECTIONS);
    let mut u8_tables: Vec<Vec<Vec<u8>>> = Vec::with_capacity(SECTIONS - U16_SECTIONS);
    for section in 0..SECTIONS {
        let (rows, row_len, packed_len) = header(bytes, &mut pos)?;
        let payload = bytes
            .get(pos..pos + packed_len)
            .ok_or("truncated section payload")?;
        pos += packed_len;
        let raw = miniz_oxide::inflate::decompress_to_vec(payload)
            .map_err(|e| format!("inflate: {e:?}"))?;
        let n = rows * row_len;
        if section < U16_SECTIONS {
            if raw.len() != n * 2 {
                return Err("section size mismatch (u16)".into());
            }
            // Undo the route compression: cumulative per-column sums.
            let mut prev = vec![0u16; row_len];
            let mut table = Vec::with_capacity(rows);
            let mut i = 0;
            for _ in 0..rows {
                let mut row = Vec::with_capacity(row_len);
                for p in prev.iter_mut().take(row_len) {
                    let d = u16::from_le_bytes([raw[i], raw[n + i]]);
                    *p = p.wrapping_add(d);
                    row.push(*p);
                    i += 1;
                }
                table.push(row);
            }
            u16_tables.push(table);
        } else {
            if raw.len() != n.div_ceil(2) {
                return Err("section size mismatch (u8)".into());
            }
            let mut flat = Vec::with_capacity(n);
            for i in 0..n {
                let b = raw[i / 2];
                flat.push(if i % 2 == 0 { b & 0x0F } else { b >> 4 });
            }
            let mut it = flat.chunks(row_len.max(1));
            u8_tables.push((0..rows).map(|_| it.next().unwrap().to_vec()).collect());
        }
    }
    let mut m16 = u16_tables.into_iter();
    let mut m8 = u8_tables.into_iter();
    let mut next16 = || m16.next().expect("section count checked");
    let mut next8 = || m8.next().expect("section count checked");
    Ok(DataTable {
        move_table: MoveTable {
            co: next16(),
            eo: next16(),
            e_combo: next16(),
            cp: next16(),
            ep: next16(),
            e_ep: next16(),
        },
        pruning_table: PruningTable {
            co_e: next8(),
            eo_e: next8(),
            cp_e: next8(),
            ep_e: next8(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_matches_the_bincode_table_exactly() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/table.bin");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("assets/table.bin missing - skipping (run xtask gen-table)");
            return;
        };
        let reference = kewb::fs::decode_table(&bytes).expect("decode");
        let packed = encode_packed(&reference);
        let back = decode_packed(&packed).expect("decode_packed");
        assert_eq!(reference.move_table.co, back.move_table.co);
        assert_eq!(reference.move_table.eo, back.move_table.eo);
        assert_eq!(reference.move_table.e_combo, back.move_table.e_combo);
        assert_eq!(reference.move_table.cp, back.move_table.cp);
        assert_eq!(reference.move_table.ep, back.move_table.ep);
        assert_eq!(reference.move_table.e_ep, back.move_table.e_ep);
        assert_eq!(reference.pruning_table.co_e, back.pruning_table.co_e);
        assert_eq!(reference.pruning_table.eo_e, back.pruning_table.eo_e);
        assert_eq!(reference.pruning_table.cp_e, back.pruning_table.cp_e);
        assert_eq!(reference.pruning_table.ep_e, back.pruning_table.ep_e);
        assert!(packed.len() * 3 < bytes.len(), "packed should be ~28%");
    }
}
