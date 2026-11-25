use std::io::{Read, Write};

#[derive(Clone, Copy)]
pub struct Shape {
    pub magic: &'static str,
    pub first: Option<&'static str>,
    pub name_ok: fn(&str) -> bool,
}

pub const LINE_MAX: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bad {
    Magic(String),
    Truncated(String),
    Path(String),
    Order(String),
    Oversize(u64),
    Io(String),
}

impl Bad {
    pub fn code(&self) -> &'static str {
        match self {
            Bad::Magic(_) => "E_FILE_MAGIC",
            Bad::Truncated(_) => "E_FILE_TRUNCATED",
            Bad::Path(_) => "E_FILE_PATH",
            Bad::Order(_) => "E_FILE_ORDER",
            Bad::Oversize(_) => "E_FILE_OVERSIZE",
            Bad::Io(_) => "E_IO",
        }
    }

    pub fn subject(&self) -> String {
        match self {
            Bad::Magic(s) | Bad::Truncated(s) | Bad::Path(s) | Bad::Order(s) | Bad::Io(s) => s.clone(),
            Bad::Oversize(n) => n.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Caps {
    pub total: Option<u64>,
    pub items: Option<usize>,
}

fn order_key<'a>(shape: &Shape, name: &'a str) -> (u8, &'a [u8]) {
    (if shape.first == Some(name) { 0 } else { 1 }, name.as_bytes())
}

pub struct Writer<W: Write> {
    w: W,
    shape: Shape,
    last: Option<String>,
}

impl<W: Write> Writer<W> {
    pub fn start(mut w: W, shape: Shape) -> Result<Writer<W>, Bad> {
        w.write_all(shape.magic.as_bytes()).map_err(|e| Bad::Io(e.to_string()))?;
        Ok(Writer { w, shape, last: None })
    }

    pub fn put(&mut self, name: &str, len: u64, body: &mut dyn Read) -> Result<(), Bad> {
        match &self.last {
            None if self.shape.first.is_some_and(|f| f != name) => return Err(Bad::Path(name.to_string())),
            Some(last) if order_key(&self.shape, last) >= order_key(&self.shape, name) => return Err(Bad::Order(name.to_string())),
            _ => {}
        }
        if !(self.shape.name_ok)(name) {
            return Err(Bad::Path(name.to_string()));
        }
        let io = |e: std::io::Error| Bad::Io(e.to_string());
        self.w.write_all(format!("{name}\n{len}\n").as_bytes()).map_err(io)?;
        let copied = std::io::copy(&mut body.take(len), &mut self.w).map_err(io)?;
        if copied != len {
            return Err(Bad::Truncated(name.to_string()));
        }
        self.w.write_all(b"\n").map_err(io)?;
        self.last = Some(name.to_string());
        Ok(())
    }

    pub fn finish(self) -> W {
        self.w
    }
}

pub fn lay(shape: Shape, pairs: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut items: Vec<&(String, Vec<u8>)> = pairs.iter().collect();
    items.sort_by(|a, b| order_key(&shape, &a.0).cmp(&order_key(&shape, &b.0)));
    let mut out = shape.magic.as_bytes().to_vec();
    for (name, bytes) in items {
        out.extend_from_slice(format!("{name}\n{}\n", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
        out.push(b'\n');
    }
    out
}

struct Counted<'a, R: Read> {
    r: &'a mut R,
    total: u64,
    cap: Option<u64>,
}

impl<R: Read> Counted<'_, R> {
    fn count(&mut self, n: u64) -> Result<(), Bad> {
        self.total = self.total.saturating_add(n);
        match self.cap {
            Some(cap) if self.total > cap => Err(Bad::Oversize(cap)),
            _ => Ok(()),
        }
    }

    fn exact(&mut self, n: usize, what: &str) -> Result<Vec<u8>, Bad> {
        self.count(n as u64)?;
        let mut buf = vec![0u8; n];
        let mut got = 0usize;
        while got < n {
            match self.r.read(&mut buf[got..]) {
                Ok(0) => return Err(Bad::Truncated(what.to_string())),
                Ok(k) => got += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(Bad::Io(e.to_string())),
            }
        }
        Ok(buf)
    }

    fn line(&mut self, at_start: bool, what: &str) -> Result<Option<String>, Bad> {
        let mut acc: Vec<u8> = Vec::new();
        loop {
            let mut one = [0u8; 1];
            match self.r.read(&mut one) {
                Ok(0) if at_start && acc.is_empty() => return Ok(None),
                Ok(0) => return Err(Bad::Truncated(what.to_string())),
                Ok(_) => self.count(1)?,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Bad::Io(e.to_string())),
            }
            if one[0] == b'\n' {
                break;
            }
            acc.push(one[0]);
            if acc.len() > LINE_MAX {
                return Err(Bad::Path(String::from_utf8_lossy(&acc[..64]).into_owned()));
            }
        }
        String::from_utf8(acc).map(Some).map_err(|e| Bad::Path(String::from_utf8_lossy(e.as_bytes()).into_owned()))
    }
}

pub fn each(r: &mut impl Read, shape: Shape, caps: Caps, item: impl FnMut(&str, u64, &mut dyn Read) -> Result<(), Bad>) -> Result<usize, Bad> {
    walk(r, shape, caps, None, item)
}

pub fn first(r: &mut impl Read, shape: Shape, cap: u64) -> Result<Option<(String, Vec<u8>)>, Bad> {
    let mut got: Option<(String, Vec<u8>)> = None;
    walk(r, shape, Caps { total: Some(cap), items: None }, Some(1), |name, _, body| {
        let mut bytes = Vec::new();
        body.read_to_end(&mut bytes).map_err(|e| Bad::Io(e.to_string()))?;
        got = Some((name.to_string(), bytes));
        Ok(())
    })?;
    Ok(got)
}

fn walk(r: &mut impl Read, shape: Shape, caps: Caps, stop_after: Option<usize>, mut item: impl FnMut(&str, u64, &mut dyn Read) -> Result<(), Bad>) -> Result<usize, Bad> {
    let mut c = Counted { r, total: 0, cap: caps.total };
    let wrong = || Bad::Magic(shape.magic.trim_end().to_string());
    let magic = c.exact(shape.magic.len(), "magic").map_err(|b| match b {
        Bad::Truncated(_) => wrong(),
        other => other,
    })?;
    if magic != shape.magic.as_bytes() {
        return Err(wrong());
    }
    let mut last: Option<String> = None;
    let mut count = 0usize;
    while let Some(name) = c.line(true, "")? {
        match &last {
            None if shape.first.is_some_and(|f| f != name) => return Err(Bad::Path(name)),
            Some(l) if order_key(&shape, l) >= order_key(&shape, &name) => return Err(Bad::Order(name)),
            _ => {}
        }
        if !(shape.name_ok)(&name) {
            return Err(Bad::Path(name));
        }
        if caps.items.is_some_and(|cap| count >= cap) {
            return Err(Bad::Oversize(caps.items.unwrap_or_default() as u64));
        }
        let size = c.line(false, &name)?.unwrap_or_default();
        let canonical = !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()) && (size == "0" || !size.starts_with('0'));
        if !canonical {
            return Err(Bad::Truncated(name));
        }
        let n: u64 = size.parse().map_err(|_| Bad::Oversize(caps.total.unwrap_or(u64::MAX)))?;
        c.count(n)?;
        let mut body = (&mut *c.r).take(n);
        item(&name, n, &mut body)?;
        std::io::copy(&mut body, &mut std::io::sink()).map_err(|e| Bad::Io(e.to_string()))?;
        if body.limit() != 0 {
            return Err(Bad::Truncated(name));
        }
        if c.exact(1, &name)? != b"\n" {
            return Err(Bad::Truncated(name));
        }
        last = Some(name);
        count += 1;
        if stop_after == Some(count) {
            break;
        }
    }
    Ok(count)
}

pub fn decode_from(r: &mut impl Read, shape: Shape, caps: Caps) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    each(r, shape, caps, |name, n, body| {
        let mut bytes = Vec::with_capacity(n.min(1 << 20) as usize);
        body.read_to_end(&mut bytes).map_err(|e| Bad::Io(e.to_string()))?;
        if bytes.len() as u64 != n {
            return Err(Bad::Truncated(name.to_string()));
        }
        out.push((name.to_string(), bytes));
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(name: &str) -> bool {
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'/' || b == b'.')
    }

    const SHAPE: Shape = Shape { magic: "test-file/1\n", first: Some("head"), name_ok: plain };

    #[test]
    fn a_set_lays_out_the_same_whatever_order_it_comes_in_and_reads_back_whole() {
        let pairs = vec![("head".to_string(), b"h".to_vec()), ("b".to_string(), vec![]), ("a/x".to_string(), b"xyz".to_vec())];
        let mut shuffled = pairs.clone();
        shuffled.reverse();
        let one = lay(SHAPE, &pairs);
        assert_eq!(one, lay(SHAPE, &shuffled), "同一组装出同一份字节");
        let mut w = Writer::start(Vec::new(), SHAPE).ok().expect("starts");
        for (n, b) in [("head", &b"h"[..]), ("a/x", &b"xyz"[..]), ("b", &b""[..])] {
            w.put(n, b.len() as u64, &mut &b[..]).ok().expect("puts");
        }
        assert_eq!(w.finish(), one, "流式写与一次排出同字节");
        assert_eq!(&one[..], &b"test-file/1\nhead\n1\nh\na/x\n3\nxyz\nb\n0\n\n"[..]);
        let back = decode_from(&mut &one[..], SHAPE, Caps::default()).ok().expect("reads");
        assert_eq!(back.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["head", "a/x", "b"]);
    }

    #[test]
    fn the_writer_refuses_a_name_out_of_shape_or_order_before_writing_it() {
        let mut w = Writer::start(Vec::new(), SHAPE).ok().expect("starts");
        assert_eq!(w.put("a", 0, &mut &b""[..]), Err(Bad::Path("a".into())), "头一件须是形制的头一件");
        w.put("head", 0, &mut &b""[..]).ok().expect("head");
        assert_eq!(w.put("B", 0, &mut &b""[..]), Err(Bad::Path("B".into())));
        w.put("c", 1, &mut &b"c"[..]).ok().expect("c");
        assert_eq!(w.put("c", 0, &mut &b""[..]), Err(Bad::Order("c".into())), "同名两件");
        assert_eq!(w.put("b", 0, &mut &b""[..]), Err(Bad::Order("b".into())));
        assert_eq!(w.put("d", 3, &mut &b"dd"[..]), Err(Bad::Truncated("d".into())), "体短于所报");
    }

    #[test]
    fn a_stream_is_read_item_by_item_without_a_bound_and_an_unread_rest_is_skipped() {
        let big = vec![7u8; 3 << 20];
        let file = lay(SHAPE, &[("head".into(), b"h".to_vec()), ("z".into(), big.clone()), ("y".into(), b"y".to_vec())]);
        let mut seen: Vec<(String, u64, usize)> = Vec::new();
        let n = each(&mut &file[..], SHAPE, Caps::default(), |name, len, body| {
            let mut one = [0u8; 1];
            let got = if len > 0 { body.read(&mut one).unwrap_or(0) } else { 0 };
            seen.push((name.to_string(), len, got));
            Ok(())
        })
        .ok()
        .expect("reads");
        assert_eq!(n, 3);
        assert_eq!(seen, vec![("head".into(), 1, 1), ("y".into(), 1, 1), ("z".into(), big.len() as u64, 1)]);
        assert_eq!(decode_from(&mut &file[..], SHAPE, Caps { total: Some(1 << 20), items: None }), Err(Bad::Oversize(1 << 20)));
        assert_eq!(decode_from(&mut &file[..], SHAPE, Caps { total: None, items: Some(2) }), Err(Bad::Oversize(2)));
        assert_eq!(first(&mut &file[..], SHAPE, 64), Ok(Some(("head".to_string(), b"h".to_vec()))));
    }

    #[test]
    fn each_bad_form_is_named() {
        let good = lay(SHAPE, &[("head".into(), b"hi".to_vec()), ("x".into(), b"x".to_vec())]);
        let read = |b: &[u8]| decode_from(&mut &b[..], SHAPE, Caps::default());
        assert_eq!(read(b"PK\x03\x04"), Err(Bad::Magic("test-file/1".into())));
        assert_eq!(read(b"test"), Err(Bad::Magic("test-file/1".into())), "短于头");
        assert!(matches!(read(&good[..good.len() - 1]), Err(Bad::Truncated(_))), "缺末尾换行");
        assert!(matches!(read(&good[..good.len() - 3]), Err(Bad::Truncated(_))));
        assert_eq!(read(b"test-file/1\nx\n0\n\n"), Err(Bad::Path("x".into())), "头一件不对");
        assert_eq!(read(b"test-file/1\nhead\n0\n\nb\n0\n\na\n0\n\n"), Err(Bad::Order("a".into())));
        assert_eq!(read(b"test-file/1\nhead\n0\n\nX\n0\n\n"), Err(Bad::Path("X".into())));
        assert_eq!(read(b"test-file/1\nhead\n1\nhX"), Err(Bad::Truncated("head".into())), "体后不是换行");
        for broken in ["-1", "x", "", "1.5", " 2", "+2", "02", "00", "2 "] {
            let mut v = b"test-file/1\nhead\n".to_vec();
            v.extend_from_slice(format!("{broken}\n").as_bytes());
            assert!(matches!(read(&v), Err(Bad::Truncated(_))), "{broken:?}");
        }
        let mut huge = b"test-file/1\nhead\n".to_vec();
        huge.extend_from_slice(b"99999999999999999999\n");
        assert!(matches!(read(&huge), Err(Bad::Oversize(_))), "过 64 位即过界");
        let mut long = b"test-file/1\n".to_vec();
        long.extend_from_slice(&vec![b'a'; LINE_MAX + 1]);
        assert!(matches!(read(&long), Err(Bad::Path(_))), "头行过长");
        assert!(matches!(read(b"test-file/1\n\xff\n0\n\n"), Err(Bad::Path(_))));
    }
}
