pub const CAP: usize = 1024;

pub struct Secret {
    buf: String,
    overflowed: bool,
}

impl Secret {
    pub fn new() -> Secret {
        Secret { buf: String::with_capacity(CAP), overflowed: false }
    }

    pub fn of(s: &str) -> Secret {
        let mut out = Secret::new();
        out.push_str(s);
        out
    }

    pub fn expose(&self) -> &str {
        &self.buf
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn chars(&self) -> usize {
        self.buf.chars().count()
    }

    pub fn push_str(&mut self, s: &str) -> usize {
        let mut n = 0;
        for c in s.chars() {
            if self.buf.len() + c.len_utf8() > self.buf.capacity() {
                self.overflowed = true;
                break;
            }
            self.buf.push(c);
            n += 1;
        }
        n
    }

    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub fn push(&mut self, c: char) -> bool {
        self.push_str(c.encode_utf8(&mut [0u8; 4])) == 1
    }

    pub fn pop(&mut self) {
        let n = self.chars();
        if n > 0 {
            self.remove_chars(n - 1..n);
        }
    }

    pub fn clear(&mut self) {
        let n = self.chars();
        self.remove_chars(0..n);
        self.overflowed = false;
    }

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

    fn insert_chars(&mut self, text: &str, at: usize) -> usize {
        let at_b = byte_at(&self.buf, at);
        let room = self.buf.capacity().saturating_sub(self.buf.len());
        let mut take = 0usize;
        let mut n = 0usize;
        for c in text.chars() {
            if take + c.len_utf8() > room {
                self.overflowed = true;
                break;
            }
            take += c.len_utf8();
            n += 1;
        }
        self.buf.insert_str(at_b, &text[..take]);
        n
    }
}

fn byte_at(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(s.len())
}

fn wipe_tail(s: &mut String, old: usize) {
    zikaron_os::wipe_string_tail(s, old);
}

pub use zikaron_os::wipe;

impl Drop for Secret {
    fn drop(&mut self) {
        zikaron_os::wipe_string(&mut self.buf);
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

impl From<String> for Secret {
    fn from(mut s: String) -> Self {
        let out = Secret::of(&s);
        zikaron_os::wipe_string(&mut s);
        out
    }
}

impl Clone for Secret {
    fn clone(&self) -> Self {
        let mut out = Secret::of(&self.buf);
        out.overflowed = self.overflowed;
        out
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
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

    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
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
