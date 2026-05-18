//! Property test: 1000 random path strings against `validate_path`.
//!
//! AC: zero out-of-bound writes regardless of input. The fuzz harness builds
//! a sandboxed `base` directory and then throws an adversarial mix of paths
//! at `validate_path`: parent traversal, absolute escapes, weird unicode,
//! null bytes, very long paths, classic CTF tricks. We then assert two
//! properties on every iteration:
//!
//! 1. If `validate_path` returns `Ok(resolved)`, then `resolved` is inside
//!    `base` (lexically — no string that starts past the boundary).
//! 2. If we then try to *write* to `resolved`, the file ends up inside
//!    `base` (no actual out-of-bound write occurred).
//!
//! We don't need `proptest` for this — a deterministic PRNG over a curated
//! set of components is plenty for 1000 trials and keeps the dev-deps slim.

use std::path::{Component, Path, PathBuf};

use actant_worker_file::validate_path;

/// Tiny deterministic PRNG. Avoids pulling `rand` into dev-deps just for fuzz.
struct Xorshift64(u64);
impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

/// Adversarial path components. Mixed: traversal, absolutes, unicode,
/// long names, dangerous Windows-style names, weird sequences.
const COMPONENTS: &[&str] = &[
    "..",
    "..",
    "../",
    "..\\",
    "/etc/passwd",
    "/tmp/escape",
    "C:\\Windows\\System32",
    "%2e%2e",
    "....//",
    "ok",
    "sub",
    "deep",
    "fil\u{00e9}",          // unicode latin
    "\u{1f600}",            // emoji
    "\u{202e}gnp.exe",      // right-to-left override
    "\0null",               // embedded NUL
    "name\0name",
    "name with spaces",
    "name\twith\ttabs",
    "name\nwith\nnewlines",
    "CON",                  // Windows reserved
    "PRN",
    "AUX",
    "nul",
    ".",
    "./",
    ".hidden",
    "verylongnameverylongnameverylongnameverylongnameverylongnameverylongnameverylongnameverylongname",
    "a/b/c",
    "a\\b\\c",
    "/",
    "",
    "~",
    "$HOME",
    "..%2F..%2Fetc",
    "..;/etc",
    "x/../y",
    "x/./y",
    "x/../../y",
];

fn random_path(rng: &mut Xorshift64) -> PathBuf {
    let depth = rng.range(6) + 1;
    let mut s = String::new();
    for i in 0..depth {
        if i > 0 {
            // Mix separator styles to stress the parser.
            s.push(if rng.range(2) == 0 { '/' } else { '\\' });
        }
        let c = COMPONENTS[rng.range(COMPONENTS.len())];
        s.push_str(c);
    }
    PathBuf::from(s)
}

fn is_inside_lex(base: &Path, candidate: &Path) -> bool {
    // Normalize lexically (no FS access) and check prefix.
    let mut norm: Vec<Component<'_>> = Vec::new();
    for c in candidate.components() {
        match c {
            Component::ParentDir => return false,
            Component::CurDir => {}
            other => norm.push(other),
        }
    }
    let resolved: PathBuf = norm.iter().collect();
    let base_norm: PathBuf = base
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    resolved.starts_with(base_norm)
}

#[test]
fn fuzz_validate_path_never_escapes_base() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().to_path_buf();

    // Seed the RNG deterministically so failures are reproducible.
    let mut rng = Xorshift64::new(0xa11ce_b0b_d34db33fu64);

    let trials = 1000usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for i in 0..trials {
        let req = random_path(&mut rng);
        match validate_path(&base, &req) {
            Ok(resolved) => {
                accepted += 1;
                // PROPERTY 1: the validated path must be inside `base`.
                assert!(
                    is_inside_lex(&base, &resolved),
                    "trial {i}: validate_path returned Ok({}) but it escapes base {}",
                    resolved.display(),
                    base.display()
                );
                // PROPERTY 2: a write to the resolved path must land inside
                // base. We create only files that have a parent inside base.
                if let Some(parent) = resolved.parent() {
                    if !parent.exists() {
                        if std::fs::create_dir_all(parent).is_err() {
                            // Some adversarial inputs (e.g. weird unicode)
                            // legitimately fail on the host FS; the property
                            // we care about is "no escape", which we already
                            // asserted lexically.
                            continue;
                        }
                    }
                    if std::fs::write(&resolved, b"x").is_err() {
                        continue;
                    }
                    // The newly written file must be under `base`.
                    let canon = resolved.canonicalize().expect("canonicalize");
                    let base_canon = base.canonicalize().expect("base canon");
                    assert!(
                        canon.starts_with(&base_canon),
                        "trial {i}: write landed at {} which escapes {}",
                        canon.display(),
                        base_canon.display()
                    );
                }
            }
            Err(_) => {
                rejected += 1;
            }
        }
    }

    // Sanity: the adversarial corpus must reject *some* paths (otherwise the
    // validator is a no-op). The actual safety properties were asserted on
    // every accepted iteration above.
    assert!(
        rejected > 0,
        "expected at least one rejection (accepted={accepted}, rejected={rejected})"
    );
    let _ = accepted;
}

#[test]
fn null_byte_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res = validate_path(tmp.path(), Path::new("nul\0byte"));
    assert!(res.is_err(), "expected null-byte rejection");
}

#[test]
fn parent_dir_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res = validate_path(tmp.path(), Path::new("../outside"));
    assert!(res.is_err(), "expected `..` rejection");
}

#[test]
fn absolute_escape_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res = validate_path(tmp.path(), Path::new("/etc/passwd"));
    assert!(res.is_err(), "expected absolute-escape rejection");
}

#[test]
fn ok_for_clean_relative_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res = validate_path(tmp.path(), Path::new("sub/dir/file.txt"));
    assert!(
        res.is_ok(),
        "expected clean relative path to validate: {res:?}"
    );
}
