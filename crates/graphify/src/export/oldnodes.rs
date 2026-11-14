//! Port of `oldnodes.go`: the nodes of the graph.json on disk, for the shrink guard.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use gostd::json::{Error as JsonError, unmarshal};
use gostd::strings::{bytes_trim_space, equal_fold};

/// Go `oldNode` (`oldnodes.go:13`): an alias of an unnamed struct, so a decode error prints the
/// struct type, not a name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct OldNode {
    pub(super) id: String,
    pub(super) source_file: String,
}

gostd::json_fields!(decode OldNode, "struct { ID string \"json:\\\"id\\\"\"; SourceFile string \"json:\\\"source_file\\\"\" }", {
    id => "id",
    source_file => "source_file",
});

/// Go `struct { Nodes []oldNode }` of `oldNodes`.
#[derive(Default)]
struct Existing {
    nodes: Vec<OldNode>,
}

gostd::json_fields!(decode Existing, "struct { Nodes []struct { ID string \"json:\\\"id\\\"\"; SourceFile string \"json:\\\"source_file\\\"\" } \"json:\\\"nodes\\\"\" }", {
    nodes => "nodes",
});

/// Go `oldNodes` (`oldnodes.go:20`): the nodes of the graph.json at `path`: none for an absent,
/// unreadable or empty file, an error for one that does not parse.
pub(super) fn old_nodes(path: &Path) -> Result<Vec<OldNode>, JsonError> {
    if let Ok(f) = File::open(path)
        && let Some(nodes) = scan_old_nodes(f)
    {
        return Ok(nodes);
    }
    let Ok(raw) = fs::read(path) else {
        return Ok(Vec::new());
    };
    if bytes_trim_space(&raw).is_empty() {
        return Ok(Vec::new());
    }
    unmarshal::<Existing>(&raw).map(|e| e.nodes)
}

/// Go `scanOldNodes` (`oldnodes.go:47`): reads the nodes of a graph.json without holding the
/// file in memory: on a large repo that buffer was the peak of an update. It takes the plain
/// case only, an object with one "nodes" array, and reports `None` (Go's `ok == false`) for
/// everything else, a syntax error included; the caller then decodes the file whole. So it may
/// refuse a document the whole decode accepts, but what it accepts must decode to the same
/// nodes.
pub(super) fn scan_old_nodes(r: impl Read) -> Option<Vec<OldNode>> {
    let mut s = NodeScan {
        r,
        buf: vec![0; 256 << 10],
        i: 0,
        n: 0,
        eof: false,
        keep: Vec::new(),
        from: None,
    };
    if s.space() != b'{' {
        return None;
    }
    let mut nodes = Vec::new();
    let mut seen = false;
    let mut c = s.space();
    while c != b'}' {
        if c != b'"' {
            return None;
        }
        let key = s.key()?;
        if s.space() != b':' {
            return None;
        }
        c = s.space();
        if key == "nodes" {
            if seen || c != b'[' {
                return None;
            }
            seen = true;
            nodes = s.nodes()?;
        } else if equal_fold(&key, "nodes") {
            // The decoder matches names without case.
            return None;
        } else if !s.value(c) {
            return None;
        }
        c = s.space();
        if c == b',' {
            c = s.space();
            if c == b'}' {
                return None;
            }
        } else if c != b'}' {
            return None;
        }
    }
    if s.space() != 0 || !s.eof {
        return None;
    }
    Some(nodes)
}

/// Go `nodeScan` (`oldnodes.go:95`): walks JSON text from `r`. A zero byte stands for the end of
/// the input; no caller takes one as valid, so a literal NUL fails the same way.
struct NodeScan<R> {
    r: R,
    buf: Vec<u8>,
    i: usize,
    n: usize,
    eof: bool,
    /// While `from` is set the bytes read since `buf[from]` are being copied to `keep`.
    keep: Vec<u8>,
    from: Option<usize>,
}

impl<R: Read> NodeScan<R> {
    /// Go `nodeScan.fill` (`oldnodes.go:105`).
    fn fill(&mut self) -> bool {
        if self.eof {
            return false;
        }
        if let Some(from) = self.from {
            self.keep.extend_from_slice(&self.buf[from..self.i]);
            self.from = Some(0);
        }
        loop {
            match self.r.read(&mut self.buf) {
                Ok(0) => {
                    self.eof = true;
                    self.i = 0;
                    self.n = 0;
                    return false;
                }
                Ok(n) => {
                    self.i = 0;
                    self.n = n;
                    return true;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.i = 0;
                    self.n = 0;
                    return false;
                }
            }
        }
    }

    /// Go `nodeScan.next` (`oldnodes.go:127`).
    fn next(&mut self) -> u8 {
        if self.i == self.n && !self.fill() {
            return 0;
        }
        let c = self.buf[self.i];
        self.i += 1;
        c
    }

    /// Go `nodeScan.space` (`oldnodes.go:137`): the next byte that is not white space.
    fn space(&mut self) -> u8 {
        loop {
            // Indentation comes in runs.
            while self.i < self.n && self.buf[self.i] == b' ' {
                self.i += 1;
            }
            let c = self.next();
            if !matches!(c, b' ' | b'\n' | b'\t' | b'\r') {
                return c;
            }
        }
    }

    /// Go `nodeScan.key` (`oldnodes.go:151`): reads a top-level member name after its opening
    /// quote. Names with an escape or a byte outside ASCII are refused: none of ours has one.
    fn key(&mut self) -> Option<String> {
        let mut k = Vec::new();
        loop {
            let c = self.next();
            if c == b'"' {
                return String::from_utf8(k).ok();
            }
            if c < 0x20 || c == b'\\' || c >= 0x80 || k.len() == 64 {
                return None;
            }
            k.push(c);
        }
    }

    /// Go `decode`, the closure of `nodeScan.nodes`: decodes the batch in `keep` into `nodes`.
    fn decode(&mut self, nodes: &mut Vec<OldNode>) -> bool {
        self.keep.push(b']');
        // Fresh each time: the decoder does not clear elements it reuses.
        let Ok(part) = unmarshal::<Vec<OldNode>>(&self.keep) else {
            return false;
        };
        nodes.extend(part);
        self.keep.clear();
        self.keep.push(b'[');
        true
    }

    /// Go `nodeScan.nodes` (`oldnodes.go:167`): reads the elements of the nodes array after its
    /// opening bracket, decoding them a batch at a time.
    fn nodes(&mut self) -> Option<Vec<OldNode>> {
        let mut nodes = Vec::new();
        self.keep.clear();
        self.keep.push(b'[');
        let mut c = self.space();
        if c == b']' {
            return Some(nodes);
        }
        loop {
            self.from = Some(self.i.saturating_sub(1));
            if !self.value(c) {
                return None;
            }
            let from = self.from.take().unwrap_or(0);
            self.keep.extend_from_slice(&self.buf[from..self.i]);
            match self.space() {
                b',' => {
                    if self.keep.len() < 256 << 10 {
                        self.keep.push(b',');
                    } else if !self.decode(&mut nodes) {
                        return None;
                    }
                    c = self.space();
                }
                b']' => return self.decode(&mut nodes).then_some(nodes),
                _ => return None,
            }
        }
    }

    /// Go `nodeScan.value` (`oldnodes.go:209`): consumes the value that starts with `c` and
    /// reports whether it is well formed.
    fn value(&mut self, mut c: u8) -> bool {
        let mut open: Vec<u8> = Vec::new(); // '{' or '[' per level
        loop {
            if matches!(c, b'{' | b'[') {
                if open.len() == 1000 {
                    return false;
                }
                let top = c;
                open.push(top);
                c = self.space();
                if c == top + 2 {
                    // Empty: '}' is '{'+2, ']' is '['+2.
                    open.pop();
                } else {
                    if top == b'{' {
                        if c != b'"' || !self.str() || self.space() != b':' {
                            return false;
                        }
                        c = self.space();
                    }
                    continue;
                }
            } else {
                let ok = match c {
                    b'"' => self.str(),
                    b'-' | b'0'..=b'9' => self.number(c),
                    b't' => self.literal(b"rue"),
                    b'f' => self.literal(b"alse"),
                    b'n' => self.literal(b"ull"),
                    _ => false,
                };
                if !ok {
                    return false;
                }
            }
            // A value just ended: close levels until a comma asks for another.
            loop {
                let Some(&top) = open.last() else {
                    return true;
                };
                c = self.space();
                if c == top + 2 {
                    open.pop();
                    continue;
                }
                if c != b',' {
                    return false;
                }
                c = self.space();
                if top == b'{' {
                    if c != b'"' || !self.str() || self.space() != b':' {
                        return false;
                    }
                    c = self.space();
                }
                break;
            }
        }
    }

    /// The bytes of `lit` in order, stopping at the first that differs (Go's `&&` chain).
    fn literal(&mut self, lit: &[u8]) -> bool {
        lit.iter().all(|&b| self.next() == b)
    }

    /// Go `nodeScan.str` (`oldnodes.go:280`): consumes a string after its opening quote.
    fn str(&mut self) -> bool {
        loop {
            // Most of the file is string bytes: run over them in one go.
            let run = self.buf[self.i..self.n]
                .iter()
                .position(|&b| b < 0x20 || b == b'"' || b == b'\\')
                .unwrap_or(self.n - self.i);
            self.i += run;
            match self.next() {
                b'"' => return true,
                b'\\' => match self.next() {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                    b'u' => {
                        for _ in 0..4 {
                            if !self.next().is_ascii_hexdigit() {
                                return false;
                            }
                        }
                    }
                    _ => return false,
                },
                c if c < 0x20 => return false,
                _ => {}
            }
        }
    }

    /// Go `nodeScan.digits`, the closure of `number`: skips the digits at `c`.
    fn digits(&mut self, c: &mut u8) -> bool {
        if !c.is_ascii_digit() {
            return false;
        }
        while c.is_ascii_digit() {
            *c = self.next();
        }
        true
    }

    /// Go `nodeScan.number` (`oldnodes.go:312`): consumes a number that starts with `c`, leaving
    /// the byte after it unread.
    fn number(&mut self, mut c: u8) -> bool {
        if c == b'-' {
            c = self.next();
        }
        if c == b'0' {
            c = self.next();
        } else if !self.digits(&mut c) {
            return false;
        }
        if c == b'.' {
            c = self.next();
            if !self.digits(&mut c) {
                return false;
            }
        }
        if c == b'e' || c == b'E' {
            c = self.next();
            if c == b'+' || c == b'-' {
                c = self.next();
            }
            if !self.digits(&mut c) {
                return false;
            }
        }
        if c == 0 {
            return false; // the input ended: nothing to unread
        }
        self.i -= 1;
        true
    }
}
