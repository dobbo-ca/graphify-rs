//! Port of `cache.go`: the extraction cache and the stat index. Byte parity of the cache files
//! with Go is a non-goal (spec section 1); the files must round-trip through this port.

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::iter;
use std::mem;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use gostd::json::{marshal, unmarshal_into};
use gostd::sha256;

use crate::extract::Result as ExtractResult;
use crate::fsutil::{write_atomic, write_file_atomic};
use crate::model::append_json_string;

/// A cache error; `Display` is Go's error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// An error of another type as a cache error: Go returns it unchanged, so its text is kept.
fn err(e: impl fmt::Display) -> Error {
    Error(e.to_string())
}

/// Go `cache.FileName` (cache.go:31): the cache file written under graphify-out alongside
/// graph.json.
pub const FILE_NAME: &str = ".graphify_cache.json";

/// Go `cache.StatFileName` (cache.go:36): the stat sidecar written alongside [`FILE_NAME`].
pub const STAT_FILE_NAME: &str = ".graphify_stat.json";

/// Go `cache.Entry` (cache.go:39): one file's content hash and its cached extraction result.
/// JSON keys `hash` and `result`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub hash: String,
    pub result: ExtractResult,
}
gostd::json_fields!(decode Entry, "cache.Entry", { hash => "hash", result => "result" });

/// Go `cache.Cache` (cache.go:45): a slash-relative file path to its cached entry (R4: a
/// `BTreeMap`; Go's writer sorts the keys, so the order is the same).
pub type Cache = BTreeMap<String, Entry>;

/// Go `cache.StatEntry` (cache.go:52): a file's size, modification time and content hash, so an
/// unchanged file (matching size and mtime) can reuse its hash without being read.
/// `indexed_at_ns` is the wall clock taken just before the content was read; 0 (an entry from
/// an older graphify-go) is untrusted. JSON keys `size`, `mtime_ns`, `hash`, `indexed_at_ns`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatEntry {
    pub size: i64,
    pub mtime_ns: i64,
    pub hash: String,
    pub indexed_at_ns: i64,
}
gostd::json_fields!(decode StatEntry, "cache.StatEntry", {
    size => "size",
    mtime_ns => "mtime_ns",
    hash => "hash",
    indexed_at_ns => "indexed_at_ns",
});

/// Go `mtimeGranularityNs` (cache.go:68): the assumed filesystem mtime granularity, 2 s.
const MTIME_GRANULARITY_NS: i64 = 2_000_000_000;

/// Go `mtimeGranularity`'s parse (cache.go:73) of `GRAPHIFY_MTIME_GRANULARITY_MS` (`raw`, `""`
/// when unset), in nanoseconds: a float of milliseconds; empty, malformed or negative gives the
/// default 2,000,000,000. A function of its own because `set_var` is `unsafe` (edition 2024).
pub(super) fn parse_granularity(raw: &str) -> i64 {
    if raw.is_empty() {
        return MTIME_GRANULARITY_NS;
    }
    match gostd::strconv::parse_float(raw) {
        Ok(ms) if ms >= 0.0 || ms.is_nan() => {
            let ns = ms * 1_000_000.0;
            // Go `int64(f)` on amd64 (the parity build): out of range and NaN are MinInt64.
            // [-2^63, 2^63): `i64::MIN as f64` is exactly -2^63.
            let lim = -(i64::MIN as f64);
            if (-lim..lim).contains(&ns) {
                ns as i64
            } else {
                i64::MIN
            }
        }
        _ => MTIME_GRANULARITY_NS,
    }
}

/// Go `mtimeGranularity` (cache.go:73), read fresh from the environment on every call.
fn mtime_granularity() -> i64 {
    parse_granularity(&std::env::var("GRAPHIFY_MTIME_GRANULARITY_MS").unwrap_or_default())
}

/// Go `statSigFresh` (cache.go:91): whether `prev` provably describes the file's CURRENT
/// content. Beyond matching (size, mtime), the entry must be racily clean in git's sense: the
/// content must have been read strictly after the file's mtime tick had already closed.
/// `granularity_ns` is [`mtime_granularity`]; the sum wraps as Go's `int64` does.
fn stat_sig_fresh(prev: &StatEntry, size: i64, mtime: i64, granularity_ns: i64) -> bool {
    prev.size == size
        && prev.mtime_ns == mtime
        && prev.indexed_at_ns != 0
        && mtime.wrapping_add(granularity_ns) <= prev.indexed_at_ns
}

/// Go `cache.StatIndex` (cache.go:102): a slash-relative file path to its stat fastpath entry.
pub type StatIndex = BTreeMap<String, StatEntry>;

/// Go `schema` (cache.go:106): the on-disk cache layout revision.
const SCHEMA: u32 = 25;

/// Go `cache.Stamp` (cache.go:112): the cache identity for a binary, its version plus the
/// on-disk schema revision (`<version>-s<schema>`).
pub fn stamp(version: &str) -> String {
    format!("{version}-s{SCHEMA}")
}

/// Go `cacheFile` (cache.go:117): the stamped envelope written to disk,
/// `{"v":..., "files":{...}}`. Go's nil `Files` and an empty one both load as an empty cache,
/// so the nil state is not kept.
#[derive(Default)]
pub(super) struct CacheFile {
    pub v: String,
    pub files: Cache,
}
gostd::json_fields!(decode CacheFile, "cache.cacheFile", { v => "v", files => "files" });

/// Go `statFile` (cache.go:122). `files` is an `Option`: Go's nil `StatIndex` (`null` or a
/// missing key) is told from an empty one, as `LoadStatOut` does.
#[derive(Default)]
struct StatFile {
    v: String,
    files: Option<StatIndex>,
    out: Option<Output>,
}
gostd::json_fields!(decode StatFile, "cache.statFile", {
    v => "v",
    files => "files",
    out => "out" omitempty,
});

/// Go `cache.Output` (cache.go:130): the last write of graph.json and the report, so an update
/// that finds nothing changed can skip rewriting them. JSON keys `fp`, `nodes`, `edges`,
/// `communities`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub fp: String,
    pub nodes: i64,
    pub edges: i64,
    pub communities: i64,
}
gostd::json_fields!(decode Output, "cache.Output", {
    fp => "fp",
    nodes => "nodes",
    edges => "edges",
    communities => "communities",
});

/// Go `io.ReadFull`: reads until `buf` is full or the reader ends (a short count). `Err` is an
/// I/O error other than the end of the input, which Go's `ErrUnexpectedEOF` and `EOF` are.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> (usize, io::Result<()>) {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(m) => n += m,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return (n, Err(e)),
        }
    }
    (n, Ok(()))
}

/// Go `cache.Load` (cache.go:140): a missing, unreadable or differently-stamped file is an
/// empty cache. `workers` is the number of decoders of the split decode (Go: `min(GOMAXPROCS,
/// 8)`).
pub fn load(path: &Path, stamp: &str, workers: usize) -> Cache {
    if let Ok(f) = File::open(path)
        && let Some(c) = load_split(f, stamp, workers)
    {
        return c;
    }
    let Ok(data) = fs::read(path) else {
        return Cache::new();
    };
    let mut f = CacheFile::default();
    if unmarshal_into(&data, &mut f).is_err() || f.v != stamp {
        return Cache::new();
    }
    drop_bad_calls(&mut f.files);
    f.files
}

/// Go `cache.Stale` (cache.go:163): whether the file at `path` is a cache written under another
/// stamp. A missing file and one that is not a cache are not stale.
pub fn stale(path: &Path, stamp: &str) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut want = br#"{"v":"#.to_vec();
    append_json_string(&mut want, stamp);
    let mut got = vec![0; want.len()];
    let (n, _) = read_full(&mut f, &mut got);
    got[..n].starts_with(br#"{"v":""#) && got[..n] != want[..]
}

/// Go `dropBadCalls` (cache.go:178): removes the entries whose calls number a text that is not
/// there, so those files are parsed again.
fn drop_bad_calls(c: &mut Cache) {
    c.retain(|_, e| e.result.calls_ok());
}

/// Go `loadSplit` (cache.go:196): decodes a cache file in pieces on `workers` threads; `None`
/// when the file is not in `write_cache`'s layout or a piece fails, and the caller decodes it
/// whole. It cuts the files object where an entry looks to end and decodes each piece as an
/// object of its own; pieces that all decode hold between them exactly the members of the whole
/// object. It reads `r` as it goes. Go's goroutines are scoped threads, its unbuffered `work`
/// channel a rendezvous channel shared through a mutex, its `free` channel a bounded one.
pub(super) fn load_split(mut r: impl Read, stamp: &str, workers: usize) -> Option<Cache> {
    let mut head = br#"{"v":"#.to_vec();
    append_json_string(&mut head, stamp);
    head.extend_from_slice(br#","files":{"#);
    let mut got = vec![0; head.len()];
    let (n, e) = read_full(&mut r, &mut got);
    if e.is_err() || n < got.len() || got != head {
        return None;
    }
    // Small pieces: bigger ones raised the peak.
    const WANT: usize = 256 << 10;
    let parts: Mutex<Vec<Cache>> = Mutex::new(Vec::new());
    let bad = AtomicBool::new(false);
    let (work_tx, work_rx) = sync_channel::<(usize, Vec<u8>)>(0);
    let work_rx = Mutex::new(work_rx);
    let (free_tx, free_rx) = sync_channel::<Vec<u8>>(16);
    std::thread::scope(|s| {
        // Past 8 decoders the wall stops falling and the heap peak keeps rising. At least
        // one: with none the producer would wait for a decoder forever.
        for _ in 0..workers.clamp(1, 8) {
            s.spawn(|| {
                loop {
                    let received = work_rx
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .recv();
                    let Ok((i, mut b)) = received else { return };
                    let mut c = Cache::new();
                    if unmarshal_into(&b, &mut c).is_err() {
                        bad.store(true, Ordering::SeqCst);
                    }
                    drop_bad_calls(&mut c);
                    parts.lock().unwrap_or_else(PoisonError::into_inner)[i] = c;
                    b.clear();
                    let _ = free_tx.try_send(b);
                }
            });
        }
        let send = |b: Vec<u8>| {
            let mut parts = parts.lock().unwrap_or_else(PoisonError::into_inner);
            let i = parts.len();
            parts.push(Cache::new());
            drop(parts);
            let _ = work_tx.send((i, b));
        };
        // pend is always an object of its own so far: '{' and whole entries.
        let mut pend = Vec::with_capacity(2 * WANT);
        pend.push(b'{');
        let mut eof = false;
        while !eof && !bad.load(Ordering::SeqCst) {
            // At least double on a miss, so one huge entry is not rescanned per read.
            let n = WANT.max(pend.len());
            let start = pend.len();
            pend.resize(start + n, 0);
            let (m, e) = read_full(&mut r, &mut pend[start..]);
            pend.truncate(start + m);
            eof = m < n || e.is_err();
            if e.is_err() {
                bad.store(true, Ordering::SeqCst);
            }
            while pend.len() > WANT {
                let Some(cut) = entry_end(&pend, WANT) else {
                    break;
                };
                let mut next = free_rx
                    .try_recv()
                    .unwrap_or_else(|_| Vec::with_capacity(2 * WANT));
                next.push(b'{');
                next.extend_from_slice(&pend[cut + 1..]);
                pend[cut] = b'}';
                pend.truncate(cut + 1);
                send(mem::replace(&mut pend, next));
            }
        }
        // What is left ends the files object and the document.
        if pend.ends_with(b"}}}") || pend == b"{}}" {
            pend.pop();
            send(pend);
        } else {
            bad.store(true, Ordering::SeqCst);
        }
        drop(work_tx);
    });
    if bad.load(Ordering::SeqCst) {
        return None;
    }
    let mut c = Cache::new();
    for p in parts.into_inner().unwrap_or_else(PoisonError::into_inner) {
        // In file order, so a repeated key resolves as in a whole decode.
        c.extend(p);
    }
    Some(c)
}

/// Go `entryEnd` (cache.go:299): the offset, at or after `from`, of a comma that looks to
/// separate two entries: `}},"<key>":{"hash":"`. `None` when there is none.
fn entry_end(b: &[u8], mut from: usize) -> Option<usize> {
    loop {
        let i = b[from..].windows(4).position(|w| w == br#"}},""#)?;
        let cut = from + i + 2;
        let mut k = cut + 2;
        while k < b.len() && b[k] != b'"' {
            if b[k] == b'\\' {
                k += 1;
            }
            k += 1;
        }
        if b.get(k..).is_some_and(|t| t.starts_with(br#"":{"hash":""#)) {
            return Some(cut);
        }
        from = cut;
    }
}

/// Go `cache.Save` (cache.go:321): writes the cache as compact JSON under `stamp`.
pub fn save(path: &Path, stamp: &str, c: &Cache) -> Result<(), Error> {
    write_atomic(path, 0o644, |w| {
        write_entries(w, stamp, c.iter()).map_err(|e| io::Error::other(e.0))
    })
    .map_err(err)
}

/// Go `cache.SaveDrain` (cache.go:330): [`save`], deleting each entry from `c` once it is
/// written. `c` is left empty, or partly emptied on an error.
pub fn save_drain(path: &Path, stamp: &str, c: &mut Cache) -> Result<(), Error> {
    write_atomic(path, 0o644, |w| {
        write_cache(w, stamp, c, true).map_err(|e| io::Error::other(e.0))
    })
    .map_err(err)
}

/// Go `writeCache` (cache.go:340): streams the cache, entry by entry in sorted key order, as
/// `{"v":<stamp>,"files":{"<key>":{"hash":<hash>,"result":<Result::append_json>},...}}`. With
/// `drain` each entry is removed from `c` as it is written. Go's nil `Cache` (`"files":null`)
/// has no `BTreeMap` form: an empty one is written as `{}`. A `Save` has only `&Cache`, so the
/// loop is [`write_entries`], over an iterator of entries; here `drain` makes it `pop_first`,
/// which removes each entry before it is encoded, as Go's `delete` does.
pub(super) fn write_cache(
    w: &mut dyn Write,
    stamp: &str,
    c: &mut Cache,
    drain: bool,
) -> Result<(), Error> {
    if drain {
        write_entries(w, stamp, iter::from_fn(|| c.pop_first()))
    } else {
        write_entries(w, stamp, c.iter())
    }
}

/// The body of Go `writeCache` over `entries`, which are in sorted key order: one `Write` call
/// for the head, one for each entry and one for the tail.
fn write_entries<K: AsRef<str>, E: Borrow<Entry>>(
    w: &mut dyn Write,
    stamp: &str,
    entries: impl Iterator<Item = (K, E)>,
) -> Result<(), Error> {
    // Go's head ends in `"files":null}`; the entries replace the null.
    let mut b = br#"{"v":"#.to_vec();
    append_json_string(&mut b, stamp);
    b.extend_from_slice(br#","files":"#);
    w.write_all(&b).map_err(err)?;
    // One buffer, one thread: the direct encoder outruns the writes.
    let mut sep = b'{';
    for (k, e) in entries {
        let e: &Entry = e.borrow();
        b.clear();
        b.push(sep);
        append_json_string(&mut b, k.as_ref());
        b.extend_from_slice(br#":{"hash":"#);
        append_json_string(&mut b, &e.hash);
        b.extend_from_slice(br#","result":"#);
        e.result.append_json(&mut b).map_err(err)?;
        b.push(b'}');
        w.write_all(&b).map_err(err)?;
        sep = b',';
    }
    w.write_all(if sep == b'{' { b"{}}" } else { b"}}" })
        .map_err(err)
}

/// Go `cache.LoadStat` (cache.go:388): a missing, unreadable or differently-stamped file is an
/// empty index.
pub fn load_stat(path: &Path, stamp: &str) -> StatIndex {
    load_stat_out(path, stamp).0
}

/// Go `cache.LoadStatOut` (cache.go:394): [`load_stat`] plus the recorded [`Output`], `None`
/// when absent.
pub fn load_stat_out(path: &Path, stamp: &str) -> (StatIndex, Option<Output>) {
    let Ok(data) = fs::read(path) else {
        return (StatIndex::new(), None);
    };
    let mut f = StatFile::default();
    if unmarshal_into(&data, &mut f).is_err() || f.v != stamp {
        return (StatIndex::new(), None);
    }
    match f.files {
        Some(files) => (files, f.out),
        None => (StatIndex::new(), None),
    }
}

/// Go `cache.SaveStat` (cache.go:407): writes the stat sidecar as compact JSON under `stamp`.
/// Go marshals a `statFile{V, Files, Out}`; this writes the same fields in the same order
/// without copying the index into one (`out` is omitted when `None`).
pub fn save_stat(
    path: &Path,
    stamp: &str,
    s: &StatIndex,
    out: Option<&Output>,
) -> Result<(), Error> {
    let mut data = br#"{"v":"#.to_vec();
    append_json_string(&mut data, stamp);
    data.extend_from_slice(br#","files":"#);
    data.extend_from_slice(&marshal(s).map_err(err)?);
    if let Some(out) = out {
        data.extend_from_slice(br#","out":"#);
        data.extend_from_slice(&marshal(out).map_err(err)?);
    }
    data.push(b'}');
    write_file_atomic(path, &data, 0o644).map_err(err)
}

/// Go `cache.HashBytes` (cache.go:416): the hex SHA-256 of `b`.
pub fn hash_bytes(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(64);
    for x in sha256::sum256(b) {
        s.push(char::from(HEX[usize::from(x >> 4)]));
        s.push(char::from(HEX[usize::from(x & 15)]));
    }
    s
}

/// Go `ioSlots` (cache.go:423): at most 4 threads inside a stat or a read, a counting semaphore
/// (past 4 the kernel spends more time contending on path lookups than the extra readers save).
static IO_SLOTS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

/// The slots held by one [`IoSlot`] at most.
const IO_SLOT_CAP: usize = 4;

/// One of the [`IO_SLOTS`], released on drop (Go's `<-ioSlots`).
struct IoSlot;

impl IoSlot {
    /// Go `ioSlots <- struct{}{}`: waits for a free slot.
    fn acquire() -> IoSlot {
        let (used, free) = &IO_SLOTS;
        let mut n = used.lock().unwrap_or_else(PoisonError::into_inner);
        while *n >= IO_SLOT_CAP {
            n = free.wait(n).unwrap_or_else(PoisonError::into_inner);
        }
        *n += 1;
        IoSlot
    }
}

impl Drop for IoSlot {
    fn drop(&mut self) {
        let (used, free) = &IO_SLOTS;
        *used.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        free.notify_one();
    }
}

/// Go `time.Time.UnixNano`.
fn unix_nano(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i64,
        Err(e) => (e.duration().as_nanos() as i64).wrapping_neg(),
    }
}

/// Go `fi.ModTime().UnixNano()`.
fn mtime_of(fi: &Metadata) -> i64 {
    fi.modified().map_or(0, unix_nano)
}

/// Go `fi.Size()`.
fn size_of(fi: &Metadata) -> i64 {
    i64::try_from(fi.len()).unwrap_or(i64::MAX)
}

/// Go `cache.StatFresh` (cache.go:426): whether [`hash_file`] would reuse `prev` without
/// reading the file.
pub fn stat_fresh(abs_path: &Path, prev: &StatEntry) -> bool {
    let slot = IoSlot::acquire();
    let fi = fs::metadata(abs_path);
    drop(slot);
    fi.is_ok_and(|fi| stat_sig_fresh(prev, size_of(&fi), mtime_of(&fi), mtime_granularity()))
}

/// Go `cache.HashFile` (cache.go:440), returning `(hash, entry, src)` or `None` where Go
/// returns `ok=false`. `src` is `None` when `prev` (the previous run's entry, `prev_ok`)
/// matched size and mtime and its hash was reused; else the file is read, and `src` holds its
/// bytes. A stat failure falls through to a full read; a read failure is `None`.
pub fn hash_file(
    abs_path: &Path,
    prev: &StatEntry,
    prev_ok: bool,
) -> Option<(String, StatEntry, Option<Vec<u8>>)> {
    hash_file_with(abs_path, prev, prev_ok, mtime_granularity())
}

/// [`hash_file`] with the mtime granularity (ns) given, not read from the environment.
pub(super) fn hash_file_with(
    abs_path: &Path,
    prev: &StatEntry,
    prev_ok: bool,
    granularity_ns: i64,
) -> Option<(String, StatEntry, Option<Vec<u8>>)> {
    let slot = IoSlot::acquire();
    let st = fs::metadata(abs_path);
    let (mtime, now) = match &st {
        Ok(fi) => {
            let mtime = mtime_of(fi);
            if prev_ok && stat_sig_fresh(prev, size_of(fi), mtime, granularity_ns) {
                drop(slot);
                return Some((prev.hash.clone(), prev.clone(), None));
            }
            (mtime, unix_nano(SystemTime::now()))
        }
        Err(_) => (0, 0),
    };
    let read = fs::read(abs_path);
    drop(slot);
    let b = read.ok()?;
    let h = hash_bytes(&b);
    let Ok(fi) = st else {
        // stat failed: no stat entry to store.
        return Some((h, StatEntry::default(), Some(b)));
    };
    let entry = StatEntry {
        size: size_of(&fi),
        mtime_ns: mtime,
        hash: h.clone(),
        indexed_at_ns: now,
    };
    Some((h, entry, Some(b)))
}
