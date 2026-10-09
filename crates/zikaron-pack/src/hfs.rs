//! An HFS+ volume built from a folder records, for every catalog entry, the creating user's owner and group
//! and its timestamps, which a non-root user cannot change through the file system. So they are rewritten in
//! the catalog directly: every owner and group other than root (0) becomes 99 (the "unknown" owner, shown on
//! mount as whoever is looking), and every date becomes one given moment, also set in both volume headers.
//! Nothing else changes (sizes, extents, contents).

/// Seconds between 1904-01-01 (the HFS epoch) and 1970-01-01.
const HFS_EPOCH: u64 = 2_082_844_800;
const UNKNOWN: u32 = 99;

/// What the rewrite found and changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// File and folder records seen in the catalog.
    pub records: usize,
    /// Records whose owner or group changed.
    pub owners_changed: usize,
    /// Records in the attributes tree (extended attributes); a clean volume has none.
    pub attribute_records: u32,
}

fn be32(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]])).ok_or_else(|| format!("short read at {o}"))
}
fn be16(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2).map(|x| u16::from_be_bytes([x[0], x[1]])).ok_or_else(|| format!("short read at {o}"))
}
fn put32(b: &mut [u8], o: usize, v: u32) -> Result<(), String> {
    b.get_mut(o..o + 4).ok_or_else(|| format!("short write at {o}"))?.copy_from_slice(&v.to_be_bytes());
    Ok(())
}

/// A B-tree file of the volume: its extents (start block, block count), up to the eight held in the volume
/// header.
struct Fork {
    extents: Vec<(u64, u64)>,
    size: u64,
}

impl Fork {
    fn at(vol: &[u8], o: usize) -> Result<Fork, String> {
        let size = u64::from(be32(vol, o)?) << 32 | u64::from(be32(vol, o + 4)?);
        let mut extents = Vec::new();
        let mut covered = 0;
        for i in 0..8 {
            let (start, count) = (be32(vol, o + 16 + i * 8)? as u64, be32(vol, o + 20 + i * 8)? as u64);
            if count == 0 {
                break;
            }
            extents.push((start, count));
            covered += count;
        }
        let block = be32(vol, 1024 + 40)? as u64;
        if covered * block < size {
            return Err("a B-tree file continues past the volume header's eight extents (not handled)".into());
        }
        Ok(Fork { extents, size })
    }
    /// The byte offset in the volume of logical byte `l` of this fork.
    fn byte(&self, vol: &[u8], l: u64) -> Result<usize, String> {
        let block = be32(vol, 1024 + 40)? as u64;
        let mut base = 0;
        for (start, count) in &self.extents {
            if l < base + count * block {
                return Ok((start * block + (l - base)) as usize);
            }
            base += count * block;
        }
        Err(format!("logical byte {l} is outside the fork"))
    }
}

/// Rewrite owners and dates in the HFS+ volume `vol` (the whole volume, from its first byte) to `epoch`
/// (seconds since 1970).
pub fn neutral_owners(vol: &mut [u8], epoch: u64) -> Result<Report, String> {
    let sig = be16(vol, 1024)?;
    if sig != 0x482B && sig != 0x4858 {
        return Err("no HFS+ volume header at byte 1024".into());
    }
    let when = u32::try_from(epoch + HFS_EPOCH).map_err(|_| "the moment is out of HFS range".to_string())?;
    let block = be32(vol, 1024 + 40)? as usize;
    let total = be32(vol, 1024 + 44)? as usize;
    // Both volume headers: create, modify and checked dates (the backup date stays zero).
    let alternate = (total * block).min(vol.len()).checked_sub(1024).ok_or("volume too small")?;
    for h in [1024, alternate] {
        if be16(vol, h)? != sig {
            if h == alternate {
                continue;
            }
            return Err("volume header lost".into());
        }
        for d in [16, 20, 28] {
            put32(vol, h + d, when)?;
        }
    }
    let catalog = Fork::at(vol, 1024 + 272)?;
    let head = catalog.byte(vol, 0)?;
    let node_size = be16(vol, head + 14 + 18)? as u64;
    let mut node = be32(vol, head + 14 + 10)?;
    let mut report = Report::default();
    let mut guard = 0u64;
    while node != 0 {
        guard += 1;
        if guard * node_size > catalog.size {
            return Err("the catalog's leaf chain loops".into());
        }
        let at = catalog.byte(vol, u64::from(node) * node_size)?;
        let ns = node_size as usize;
        if vol.get(at + 8).map(|k| *k as i8) != Some(-1) {
            return Err(format!("catalog node {node} in the leaf chain is not a leaf"));
        }
        let n = be16(vol, at + 10)? as usize;
        for r in 0..n {
            let rec = at + be16(vol, at + ns - 2 * (r + 1))? as usize;
            let key_len = be16(vol, rec)? as usize;
            let data = rec + 2 + key_len;
            let kind = be16(vol, data)?;
            if kind != 1 && kind != 2 {
                continue;
            }
            report.records += 1;
            for d in [12, 16, 20, 24] {
                put32(vol, data + d, when)?;
            }
            let (owner, group) = (be32(vol, data + 32)?, be32(vol, data + 36)?);
            let (o2, g2) = (if owner == 0 { 0 } else { UNKNOWN }, if group == 0 { 0 } else { UNKNOWN });
            if (o2, g2) != (owner, group) {
                report.owners_changed += 1;
                put32(vol, data + 32, o2)?;
                put32(vol, data + 36, g2)?;
            }
        }
        node = be32(vol, at)?;
    }
    let attributes = Fork::at(vol, 1024 + 352)?;
    if attributes.size > 0 {
        let h = attributes.byte(vol, 0)?;
        report.attribute_records = be32(vol, h + 14 + 6)?;
    }
    Ok(report)
}
