//! Secret strings: passcodes, passwords, recovery words and private keys all use this type.
//!
//! As `String`s they leaked: `clear()` only sets the length to zero and leaves bytes on the heap; a growing
//! `String` moves to a larger allocation and returns the old one unwiped; and a derived `Debug` prints them
//! in plain text. The type answers "is this a secret", and wiping, staying in place and masked debug output
//! live here.
//!
//! 1. Wiped when dropped: `Drop` writes zeros over the whole allocation (capacity included); deleting
//! characters or clearing also zeros the freed part at once.
//! 2. Never moves: it reserves [`CAP`] bytes when built and only edits inside that block; text that does not
//! fit is not taken (the input control learns how much was taken from `insert_text`), so no old block is ever
//! left behind.
//! 3. Debug is masked: `Debug` prints only "secret · N chars".
//!
//! The input control edits this block through [`egui::TextBuffer`] without first copying into a `String`.
//! Copies egui makes within a frame (undo stack, previous text) are outside this type: the undo stack is
//! cleared every frame by the input control (`input::secret_edit`), and the previous-text copy is a temporary
//! inside egui's frame.

/// Most bytes one secret holds. Eight passcode cells, twelve words (a whole phrase pasted into one cell is
/// about a hundred bytes), backup passwords and hex private keys all fit well within it; what does not fit is
/// not taken, and the block never moves.
pub const CAP: usize = 1024;

pub struct Secret {
    buf: String,
}

impl Secret {
    /// An empty secret (the whole block reserved at once).
    pub fn new() -> Secret {
        Secret { buf: String::with_capacity(CAP) }
    }

    /// Build from text (a tail that does not fit is not taken).
    pub fn of(s: &str) -> Secret {
        let mut out = Secret::new();
        out.push_str(s);
        out
    }

    /// Read the text. The name is the reminder: this is plain text.
    pub fn expose(&self) -> &str {
        &self.buf
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Length in characters, not bytes.
    pub fn chars(&self) -> usize {
        self.buf.chars().count()
    }

    /// Append text; characters that do not fit are not taken. Returns how many were taken.
    pub fn push_str(&mut self, s: &str) -> usize {
        let mut n = 0;
        for c in s.chars() {
            if self.buf.len() + c.len_utf8() > self.buf.capacity() {
                break;
            }
            self.buf.push(c);
            n += 1;
        }
        n
    }

    /// Append one character (not taken when it does not fit; returns false).
    pub fn push(&mut self, c: char) -> bool {
        self.push_str(c.encode_utf8(&mut [0u8; 4])) == 1
    }

    /// Remove the last character; the freed bytes are zeroed.
    pub fn pop(&mut self) {
        let n = self.chars();
        if n > 0 {
            self.remove_chars(n - 1..n);
        }
    }

    /// Clear; the freed bytes are zeroed.
    pub fn clear(&mut self) {
        let n = self.chars();
        self.remove_chars(0..n);
    }

    /// Remove a character range (by character index); the freed bytes are zeroed.
    fn remove_chars(&mut self, r: std::ops::Range<usize>) {
        let start = byte_at(&self.buf, r.start);
        let end = byte_at(&self.buf, r.end);
        if start >= end {
            return;
        }
        let old = self.buf.len();
        self.buf.replace_range(start..end, "");
        wipe_tail(&mut self.buf, old);
    }

    /// Insert text at a character index; characters that do not fit are not taken. Returns how many were
    /// inserted.
    fn insert_chars(&mut self, text: &str, at: usize) -> usize {
        let at_b = byte_at(&self.buf, at);
        let room = self.buf.capacity().saturating_sub(self.buf.len());
        let mut take = 0usize;
        let mut n = 0usize;
        for c in text.chars() {
            if take + c.len_utf8() > room {
                break;
            }
            take += c.len_utf8();
            n += 1;
        }
        // It fits the capacity, so `insert_str` does not move.
        self.buf.insert_str(at_b, &text[..take]);
        n
    }
}

/// The byte offset of a character index (past the end means the end).
fn byte_at(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(s.len())
}

/// After shrinking, zero `[new length, old length)` (within capacity, so nothing moves).
fn wipe_tail(s: &mut String, old: usize) {
    let new = s.len();
    // SAFETY: only zeros are written, only within capacity and past the length, and the length stays `new`;
    // zero bytes are valid UTF-8.
    let v = unsafe { s.as_mut_vec() };
    let p = v.as_mut_ptr();
    for i in new..old.min(v.capacity()) {
        unsafe { std::ptr::write_volatile(p.add(i), 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Wipe plain text held in hand. Everything that held plain-text bytes calls this.
pub fn wipe(b: &mut [u8]) {
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Zero the whole allocation, including unused capacity (where deleted characters used to be).
        let v = unsafe { self.buf.as_mut_vec() };
        let cap = v.capacity();
        let p = v.as_mut_ptr();
        for i in 0..cap {
            unsafe { std::ptr::write_volatile(p.add(i), 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        unsafe { v.set_len(0) };
    }
}

impl Default for Secret {
    fn default() -> Self {
        Secret::new()
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Secret::of(s)
    }
}

/// Take in a `String` and zero it in place, so its heap is zeros when returned to the allocator.
impl From<String> for Secret {
    fn from(mut s: String) -> Self {
        let out = Secret::of(&s);
        wipe(unsafe { s.as_mut_vec() });
        out
    }
}

impl Clone for Secret {
    fn clone(&self) -> Self {
        Secret::of(&self.buf)
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        // Compare every byte before answering (no early return on the first difference); with different
        // lengths, the shorter span is still walked.
        let a = self.buf.as_bytes();
        let b = other.buf.as_bytes();
        let mut diff = (a.len() != b.len()) as u8;
        for i in 0..a.len().min(b.len()) {
            diff |= a[i] ^ b[i];
        }
        diff == 0
    }
}

impl Eq for Secret {}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret(••• {} chars)", self.chars())
    }
}

impl egui::TextBuffer for Secret {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        &self.buf
    }

    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        self.insert_chars(text, char_index)
    }

    fn delete_char_range(&mut self, char_range: std::ops::Range<usize>) {
        self.remove_chars(char_range);
    }

    fn clear(&mut self) {
        Secret::clear(self);
    }

    fn replace_with(&mut self, text: &str) {
        Secret::clear(self);
        self.push_str(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_stay_in_one_block_and_debug_hides_the_text() {
        let mut s = Secret::new();
        let before = s.buf.as_ptr();
        s.push_str("K7m2abcd");
        s.pop();
        s.push('Z');
        egui::TextBuffer::insert_text(&mut s, "xy", 2);
        egui::TextBuffer::delete_char_range(&mut s, 0..2);
        assert_eq!(s.expose(), "xym2abcZ");
        assert_eq!(s.buf.as_ptr(), before, "不许挪窝");
        let d = format!("{s:?}");
        assert!(!d.contains("xym2"), "调试输出印出了明文:{d}");
        // Deleted parts were zeroed: everything past the length within capacity is zero.
        let len = s.buf.len();
        s.clear();
        let v = unsafe { s.buf.as_mut_vec() };
        let p = v.as_ptr();
        for i in 0..len {
            assert_eq!(unsafe { *p.add(i) }, 0);
        }
    }

    #[test]
    fn a_full_block_takes_no_more() {
        let mut s = Secret::new();
        let long = "a".repeat(CAP + 10);
        let cap = s.buf.capacity();
        assert_eq!(s.push_str(&long), cap);
        assert_eq!(egui::TextBuffer::insert_text(&mut s, "b", 0), 0);
        assert_eq!(s.chars(), cap);
    }
}
