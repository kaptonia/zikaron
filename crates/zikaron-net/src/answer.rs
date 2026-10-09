//! Incremental reader for one HTTP/1.1 response: the head once, then the body by its framing (a length,
//! chunks, or until the peer closes), each byte examined once.
//!
//! Header names are case-insensitive and values trimmed; interim responses (`1xx`, except a protocol switch)
//! are skipped; chunk lengths are summed with an overflow check. It also decides whether the connection can be
//! reused: the peer did not ask to close, the response is HTTP/1.1 (or HTTP/1.0 with `Connection: keep-alive`),
//! its end was known from its framing, and nothing followed it.

use crate::Fail;

/// Longest chunk-size or trailer line accepted (guards against a peer that never ends a line).
const MAX_LINE: usize = 4096;

/// The head of the final response.
pub(crate) struct Head {
    pub(crate) status: u16,
    pub(crate) location: Option<String>,
    /// The peer sent `Connection: close`, or used HTTP/1.0 without keep-alive.
    close: bool,
}

/// How the body's end is determined.
enum Frame {
    /// The head has not arrived yet.
    Head,
    /// This many body bytes remain.
    Length(u64),
    /// Chunked transfer, at this step.
    Chunks(Step),
    /// No framing: the body ends when the peer closes.
    ToEnd,
}

#[derive(Clone, Copy)]
enum Step {
    /// A chunk-size line.
    Size,
    /// This many bytes of the current chunk remain.
    Data(u64),
    /// The line end after a chunk's bytes.
    DataEnd,
    /// Trailer lines after the last chunk, until an empty one.
    Trailer,
}

/// One response being read.
pub(crate) struct Reader {
    raw: Vec<u8>,
    at: usize,
    head: Option<Head>,
    body: Vec<u8>,
    frame: Frame,
    done: bool,
    /// Both a length and chunked transfer were given: the chunks are read and the connection is not reused.
    mixed: bool,
}

/// Find `\r\n\r\n` in `raw`, looking only from `from` on.
fn head_end(raw: &[u8], from: usize) -> Option<usize> {
    raw.get(from..)?.windows(4).position(|w| w == b"\r\n\r\n").map(|i| from + i)
}

/// Find a line end in `raw` from `from` on.
fn line_end(raw: &[u8], from: usize) -> Option<usize> {
    raw.get(from..)?.windows(2).position(|w| w == b"\r\n").map(|i| from + i)
}

impl Reader {
    pub(crate) fn new() -> Reader {
        Reader { raw: Vec::new(), at: 0, head: None, body: Vec::new(), frame: Frame::Head, done: false, mixed: false }
    }

    /// Every byte read so far (the response size cap counts these, head and framing included).
    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Whether the whole response has arrived.
    #[cfg(test)]
    pub(crate) fn done(&self) -> bool {
        self.done
    }

    /// Feed more bytes; returns whether the response is now complete.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Result<bool, Fail> {
        self.raw.extend_from_slice(bytes);
        while !self.done {
            if !self.step()? {
                break;
            }
        }
        Ok(self.done)
    }

    /// The peer closed. A close-delimited response ends here; any other unfinished response is an error.
    pub(crate) fn closed(&mut self) -> Result<(), Fail> {
        if self.done {
            return Ok(());
        }
        match self.frame {
            Frame::ToEnd => {
                self.body.extend_from_slice(&self.raw[self.at..]);
                self.at = self.raw.len();
                self.done = true;
                Ok(())
            }
            Frame::Head => Err(Fail::Stream("the answer has no end of head".into())),
            Frame::Length(_) => Err(Fail::Stream("the body is shorter than its length".into())),
            Frame::Chunks(_) => Err(Fail::Stream("the chunked answer is cut short".into())),
        }
    }

    /// Whether the connection can be reused: the response is complete, its end was known from its framing, the
    /// peer did not ask to close, and no byte followed it.
    pub(crate) fn keeps(&self) -> bool {
        self.done
            && !self.mixed
            && !matches!(self.frame, Frame::ToEnd)
            && self.head.as_ref().map(|h| !h.close).unwrap_or(false)
            && self.at == self.raw.len()
    }

    /// The final head and the body (`None` until the response is complete).
    pub(crate) fn answer(self) -> Option<(Head, Vec<u8>)> {
        match (self.done, self.head) {
            (true, Some(h)) => Some((h, self.body)),
            _ => None,
        }
    }

    /// One step of reading; `false` when more bytes are needed.
    fn step(&mut self) -> Result<bool, Fail> {
        match self.frame {
            Frame::Head => {
                let Some(end) = head_end(&self.raw, self.at) else { return Ok(false) };
                let text = String::from_utf8_lossy(&self.raw[self.at..end]).to_string();
                self.at = end + 4;
                self.read_head(&text)?;
                Ok(true)
            }
            Frame::Length(left) => {
                let have = (self.raw.len() - self.at) as u64;
                let take = have.min(left) as usize;
                self.body.extend_from_slice(&self.raw[self.at..self.at + take]);
                self.at += take;
                let left = left - take as u64;
                self.frame = Frame::Length(left);
                if left == 0 {
                    self.done = true;
                }
                Ok(left == 0)
            }
            Frame::Chunks(step) => self.chunk(step),
            // Only the peer's close ends it.
            Frame::ToEnd => Ok(false),
        }
    }

    /// Read a head: an interim response is skipped; the final one sets the framing.
    fn read_head(&mut self, text: &str) -> Result<(), Fail> {
        let mut lines = text.split("\r\n");
        let status_line = lines.next().unwrap_or("");
        let mut parts = status_line.split_whitespace();
        let version = parts.next().unwrap_or("");
        let status: u16 = match (version.starts_with("HTTP/"), parts.next().and_then(|c| c.parse().ok())) {
            (true, Some(s)) => s,
            _ => return Err(Fail::Stream("the answer's status line does not read".into())),
        };
        if status == 101 {
            return Err(Fail::Stream("the answer switched protocols".into()));
        }
        if (100..200).contains(&status) {
            // Interim (`100 Continue`, `103 Early Hints`): the final response follows.
            return Ok(());
        }
        let mut chunked = false;
        let mut length: Option<u64> = None;
        let mut said_close = false;
        let mut keep_alive = false;
        let mut location = None;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else { continue };
            let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
            match name.as_str() {
                "transfer-encoding" => {
                    let last = value.rsplit(',').next().unwrap_or("").trim();
                    chunked = last.eq_ignore_ascii_case("chunked");
                }
                "content-length" => {
                    for v in value.split(',') {
                        let n: u64 = v.trim().parse().map_err(|_| Fail::Stream("the answer's length does not read".into()))?;
                        if length.is_some_and(|m| m != n) {
                            return Err(Fail::Stream("the answer gives two different lengths".into()));
                        }
                        length = Some(n);
                    }
                }
                "connection" => {
                    for token in value.split(',') {
                        let token = token.trim();
                        if token.eq_ignore_ascii_case("close") {
                            said_close = true;
                        } else if token.eq_ignore_ascii_case("keep-alive") {
                            keep_alive = true;
                        }
                    }
                }
                "location" if location.is_none() => location = Some(value.to_string()),
                _ => {}
            }
        }
        // HTTP/1.1 is persistent unless the peer says close; HTTP/1.0 only with keep-alive.
        let close = said_close || (version != "HTTP/1.1" && !(version == "HTTP/1.0" && keep_alive));
        self.head = Some(Head { status, location, close });
        self.mixed = chunked && length.is_some();
        self.frame = if status == 204 || status == 304 {
            self.done = true;
            Frame::Length(0)
        } else if chunked {
            Frame::Chunks(Step::Size)
        } else if let Some(n) = length {
            if n == 0 {
                self.done = true;
            }
            Frame::Length(n)
        } else {
            Frame::ToEnd
        };
        Ok(())
    }

    /// One step of chunked transfer.
    fn chunk(&mut self, step: Step) -> Result<bool, Fail> {
        let cut = || Fail::Stream("the chunked answer is cut short".into());
        match step {
            Step::Size => {
                let Some(eol) = line_end(&self.raw, self.at) else {
                    if self.raw.len() - self.at > MAX_LINE {
                        return Err(Fail::Stream("a chunk length does not read".into()));
                    }
                    return Ok(false);
                };
                let line = String::from_utf8_lossy(&self.raw[self.at..eol]);
                let size = line.split(';').next().unwrap_or("").trim();
                if size.is_empty() || size.len() > 16 || !size.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(Fail::Stream("a chunk length does not read".into()));
                }
                let n = u64::from_str_radix(size, 16).map_err(|_| Fail::Stream("a chunk length does not read".into()))?;
                // The running total must fit: a chunk of `ffffffffffffffff` is refused with a named error.
                if (self.body.len() as u64).checked_add(n).is_none_or(|t| t > usize::MAX as u64) {
                    return Err(Fail::Stream("a chunk length overflows".into()));
                }
                self.at = eol + 2;
                self.frame = Frame::Chunks(if n == 0 { Step::Trailer } else { Step::Data(n) });
                Ok(true)
            }
            Step::Data(left) => {
                let have = (self.raw.len() - self.at) as u64;
                if have == 0 {
                    return Ok(false);
                }
                let take = have.min(left) as usize;
                self.body.extend_from_slice(&self.raw[self.at..self.at + take]);
                self.at += take;
                let left = left - take as u64;
                self.frame = Frame::Chunks(if left == 0 { Step::DataEnd } else { Step::Data(left) });
                Ok(true)
            }
            Step::DataEnd => {
                if self.raw.len() - self.at < 2 {
                    return Ok(false);
                }
                if &self.raw[self.at..self.at + 2] != b"\r\n" {
                    return Err(cut());
                }
                self.at += 2;
                self.frame = Frame::Chunks(Step::Size);
                Ok(true)
            }
            Step::Trailer => {
                let Some(eol) = line_end(&self.raw, self.at) else {
                    if self.raw.len() - self.at > MAX_LINE {
                        return Err(cut());
                    }
                    return Ok(false);
                };
                let empty = eol == self.at;
                self.at = eol + 2;
                if empty {
                    self.done = true;
                }
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a whole response in one piece and again one byte at a time (the two must agree).
    fn read(raw: &[u8]) -> Result<(u16, Vec<u8>, bool), Fail> {
        let whole = {
            let mut r = Reader::new();
            r.feed(raw)?;
            r.closed()?;
            let keeps = r.keeps();
            let (h, b) = r.answer().expect("whole");
            (h.status, b, keeps)
        };
        let mut r = Reader::new();
        for b in raw {
            r.feed(std::slice::from_ref(b))?;
        }
        r.closed()?;
        let keeps = r.keeps();
        let (h, b) = r.answer().expect("whole");
        assert_eq!((h.status, b.clone(), keeps), whole.clone(), "bytewise and whole differ");
        Ok(whole)
    }

    /// A chunk length of `ffffffffffffffff` is a named refusal, never an overflow or panic; a truncated
    /// response is named.
    #[test]
    fn a_chunk_length_past_the_bound_is_named_not_a_crash() {
        // The maximum length: read without a crash, and reported as truncated when the peer closes.
        let mut r = Reader::new();
        assert_eq!(r.feed(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\nab").ok(), Some(false));
        assert!(matches!(r.closed(), Err(Fail::Stream(_))));
        // A second chunk whose length overflows the running total is refused by name.
        let mut r = Reader::new();
        let e = r.feed(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\nffffffffffffffff\r\n");
        assert!(matches!(e, Err(Fail::Stream(_))), "{:?}", e.ok());
        let mut r = Reader::new();
        assert!(r.feed(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10000000000000000\r\n").is_err(), "seventeen hex digits");
        assert!(read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab").is_err());
        assert!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nabc").is_err());
        assert!(read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcX\r\n0\r\n\r\n").is_err(), "a chunk not ending at its length");
        assert!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 3").is_err(), "no end of head");
    }

    /// Case-insensitive header names, trimmed values, the last transfer coding wins, interim responses skipped,
    /// a status line without a reason phrase, two different lengths refused.
    #[test]
    fn the_head_is_read_once_by_name_without_case() {
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding:chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n").ok().map(|x| x.1), Some(b"abc".to_vec()));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nContent-Length : 5\r\n\r\nhello").ok().map(|x| x.1), Some(b"hello".to_vec()));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\ntransfer-encoding: gzip, chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n").ok().map(|x| x.1), Some(b"ab".to_vec()));
        // Another header's value mentioning chunked transfer does not set the framing.
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nX-Note: transfer-encoding: chunked\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.1), Some(b"ok".to_vec()));
        assert_eq!(read(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </x>\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| (x.0, x.1)), Some((200, b"ok".to_vec())));
        assert_eq!(read(b"HTTP/1.1 200\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.0), Some(200));
        assert!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\nokk").is_err());
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.1), Some(b"ok".to_vec()));
        assert!(read(b"SMTP 220 hello\r\n\r\n").is_err());
        // Chunk extensions and trailers.
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;x=y\r\nabc\r\n0\r\nX-T: 1\r\n\r\n").ok().map(|x| x.1), Some(b"abc".to_vec()));
    }

    /// Connection reuse: only a complete HTTP/1.1 response whose end was known and whose peer did not ask to
    /// close.
    #[test]
    fn a_connection_is_kept_only_when_the_peer_leaves_it_open() {
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.2), Some(true));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").ok().map(|x| x.2), Some(false));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nconnection: Keep-Alive, Close\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.2), Some(false));
        assert_eq!(read(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.2), Some(false));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\n\r\nto the end").ok().map(|x| (x.1, x.2)), Some((b"to the end".to_vec(), false)));
        assert_eq!(read(b"HTTP/1.1 204 No Content\r\n\r\n").ok().map(|x| (x.1, x.2)), Some((Vec::new(), true)));
        let mut r = Reader::new();
        r.feed(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nokEXTRA").expect("reads");
        assert!(r.done() && !r.keeps(), "bytes after the answer: not kept");
    }
    /// 101 Switching Protocols is refused by name, never skipped as an interim response (also after a
    /// `100 Continue`).
    #[test]
    fn a_protocol_switch_is_refused_by_name() {
        let said = Some(Fail::Stream("the answer switched protocols".into()));
        assert_eq!(Reader::new().feed(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n").err(), said);
        assert_eq!(Reader::new().feed(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 101 Switching Protocols\r\n\r\n").err(), said);
    }

    /// 304 Not Modified with `Content-Length: 5` and no body: complete at the end of its head, empty body, the
    /// length not waited for, the connection reused.
    #[test]
    fn a_not_modified_answer_has_no_body_whatever_its_length() {
        let mut r = Reader::new();
        assert_eq!(r.feed(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 5\r\n\r\n").ok(), Some(true));
        assert!(r.keeps());
        assert_eq!(read(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 5\r\n\r\n").ok(), Some((304, Vec::new(), true)));
    }

    /// Both `Content-Length` and chunked transfer: the chunks are read (not the length), in either order, and
    /// the connection is not reused.
    #[test]
    fn a_length_with_chunks_is_read_as_chunks_and_not_kept() {
        let mut r = Reader::new();
        assert_eq!(r.feed(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n").ok(), Some(true));
        assert!(!r.keeps());
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n").ok(), Some((200, b"hello".to_vec(), false)));
        assert_eq!(read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 30\r\n\r\n5\r\nhello\r\n0\r\n\r\n").ok(), Some((200, b"hello".to_vec(), false)));
    }

    /// The line cap (`MAX_LINE`, 4096): a chunk-size line at the cap without a line end is still awaited; one
    /// byte past it is refused by name.
    #[test]
    fn a_chunk_size_line_past_the_line_cap_is_refused() {
        assert_eq!(MAX_LINE, 4096);
        let mut r = Reader::new();
        assert_eq!(r.feed(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").ok(), Some(false));
        assert_eq!(r.feed(&[b'0'; MAX_LINE]).ok(), Some(false), "at the cap: still waited on");
        assert_eq!(r.feed(b"0").err(), Some(Fail::Stream("a chunk length does not read".into())));
    }

    /// HTTP/1.0 with `Connection: keep-alive` and a known length: reused.
    #[test]
    fn an_http_1_0_answer_asking_keep_alive_is_kept() {
        assert_eq!(read(b"HTTP/1.0 200 OK\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\nok").ok(), Some((200, b"ok".to_vec(), true)));
        assert_eq!(read(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nok").ok().map(|x| x.2), Some(false), "without asking: not kept");
    }
}
