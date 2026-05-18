//! Sparse encoders.
//!
//! [`SparseEncoder`] is the trait surface used by `actant-index`'s hybrid
//! retrieval planner. [`Bm25Encoder`] is the pure-Rust default — plain BM25
//! over a small in-memory document collection. Heavier neural sparse models
//! (SPLADE, BM42) are deferred behind future feature flags.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Sparse vector — token-id -> weight, with an explicit dimension upper
/// bound for serialization. For BM25 the "token id" is a stable hash of the
/// surface form; consumers that need a shared vocabulary should swap in a
/// vocab-backed encoder.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SparseVector {
    /// `(token_id, weight)` pairs, sorted by token_id ascending.
    pub indices: Vec<u32>,
    /// Weights aligned with `indices`.
    pub values: Vec<f32>,
}

impl SparseVector {
    /// Number of non-zero entries.
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Dot product against another sparse vector. Both inputs must be sorted
    /// ascending by `indices`.
    pub fn dot(&self, other: &SparseVector) -> f32 {
        let (mut i, mut j) = (0usize, 0usize);
        let mut acc = 0.0f32;
        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    acc += self.values[i] * other.values[j];
                    i += 1;
                    j += 1;
                }
            }
        }
        acc
    }
}

/// Sparse encoder trait. Implementations may be stateful (vocab, IDF cache).
#[async_trait]
pub trait SparseEncoder: Send + Sync + 'static {
    /// Stable provider id (e.g. `"bm25"`, `"splade-v3"`).
    fn provider(&self) -> &'static str;
    /// Encode one document or query into a sparse vector.
    fn encode(&self, text: &str) -> SparseVector;
}

/// Pure-Rust BM25 encoder over a small in-memory collection.
///
/// Construction order:
///
/// ```ignore
/// let mut enc = Bm25Encoder::new();
/// enc.index_document("doc-1", "the quick brown fox");
/// enc.index_document("doc-2", "the lazy dog");
/// enc.finalize(); // computes idf + avgdl
/// let q = enc.encode("quick fox");
/// ```
///
/// Tokenization is `split_whitespace` + lowercase + strip ASCII punctuation;
/// `unicode-segmentation` is intentionally not a workspace dep.
#[derive(Debug, Clone)]
pub struct Bm25Encoder {
    k1: f32,
    b: f32,
    /// token hash -> document frequency
    df: HashMap<u32, u32>,
    /// total number of indexed documents
    n_docs: u32,
    /// average document length in tokens
    avgdl: f32,
    /// total token count across all docs (used to compute `avgdl`).
    total_tokens: u64,
    /// `true` once `finalize` has been called.
    finalized: bool,
}

impl Default for Bm25Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Bm25Encoder {
    /// New encoder with the classic BM25 defaults (`k1 = 1.5`, `b = 0.75`).
    pub fn new() -> Self {
        Self {
            k1: 1.5,
            b: 0.75,
            df: HashMap::new(),
            n_docs: 0,
            avgdl: 0.0,
            total_tokens: 0,
            finalized: false,
        }
    }

    /// Tokenize: lowercase, ASCII whitespace split, strip leading/trailing
    /// punctuation, drop empty fragments.
    fn tokenize(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|t| {
                t.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Stable token hash. We use FNV-1a 32-bit so the hash is portable across
    /// runs and platforms (`HashMap`'s default `RandomState` is not stable).
    fn token_id(tok: &str) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        for b in tok.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        hash
    }

    /// Add a document to the collection. The `_doc_id` is accepted for API
    /// symmetry with retrieval systems but not stored — this encoder only
    /// tracks corpus statistics.
    pub fn index_document(&mut self, _doc_id: &str, text: &str) {
        let tokens = Self::tokenize(text);
        let mut seen = HashMap::<u32, ()>::new();
        for tok in &tokens {
            let id = Self::token_id(tok);
            seen.entry(id).or_insert(());
        }
        for id in seen.keys() {
            *self.df.entry(*id).or_insert(0) += 1;
        }
        self.n_docs += 1;
        self.total_tokens += tokens.len() as u64;
        self.finalized = false;
    }

    /// Finalize corpus statistics. Safe to call multiple times.
    pub fn finalize(&mut self) {
        if self.n_docs == 0 {
            self.avgdl = 0.0;
        } else {
            self.avgdl = self.total_tokens as f32 / self.n_docs as f32;
        }
        self.finalized = true;
    }

    /// Inverse-document-frequency for a token (Robertson BM25 form).
    fn idf(&self, tok_id: u32) -> f32 {
        let df = *self.df.get(&tok_id).unwrap_or(&0) as f32;
        let n = self.n_docs as f32;
        // BM25 idf: ln( (N - df + 0.5) / (df + 0.5) + 1 ); +1 keeps it
        // strictly positive even for very common terms.
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }
}

#[async_trait]
impl SparseEncoder for Bm25Encoder {
    fn provider(&self) -> &'static str {
        "bm25"
    }

    fn encode(&self, text: &str) -> SparseVector {
        let tokens = Self::tokenize(text);
        if tokens.is_empty() {
            return SparseVector::default();
        }
        let dl = tokens.len() as f32;
        let avgdl = if self.avgdl > 0.0 { self.avgdl } else { dl };

        // Term frequency in the encoded text.
        let mut tf: HashMap<u32, u32> = HashMap::new();
        for tok in &tokens {
            *tf.entry(Self::token_id(tok)).or_insert(0) += 1;
        }

        let mut pairs: Vec<(u32, f32)> = tf
            .into_iter()
            .map(|(id, count)| {
                let f = count as f32;
                let idf = self.idf(id);
                let denom = f + self.k1 * (1.0 - self.b + self.b * (dl / avgdl));
                let numer = f * (self.k1 + 1.0);
                let weight = if denom > 0.0 { idf * (numer / denom) } else { 0.0 };
                (id, weight)
            })
            .collect();

        pairs.sort_by_key(|p| p.0);
        let (indices, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
        SparseVector { indices, values }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_yields_empty_vector() {
        let enc = Bm25Encoder::new();
        let v = enc.encode("");
        assert_eq!(v.nnz(), 0);
    }

    #[test]
    fn provider_is_bm25() {
        let enc = Bm25Encoder::new();
        assert_eq!(enc.provider(), "bm25");
    }

    #[test]
    fn tokenization_strips_punctuation() {
        let toks = Bm25Encoder::tokenize("Hello, world! It's fine.");
        assert_eq!(toks, vec!["hello", "world", "it's", "fine"]);
    }

    #[test]
    fn token_id_is_stable() {
        assert_eq!(Bm25Encoder::token_id("hello"), Bm25Encoder::token_id("hello"));
        assert_ne!(Bm25Encoder::token_id("a"), Bm25Encoder::token_id("b"));
    }
}
