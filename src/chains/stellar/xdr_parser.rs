//! Zero-copy parser for the header fields of a Stellar `TransactionV1Envelope`
//! (issue #345). Benchmarked in `benches/xdr_parser.rs`.
//!
//! Scope: this decodes the fixed-shape envelope header — envelope type,
//! source account, fee, sequence number, preconditions, and memo — reading
//! primitives straight out of the input slice with no intermediate
//! allocation; variable-length fields (the source account key, memo bytes)
//! are returned as borrows into the original buffer.
//!
//! It does not decode operation bodies. XDR does not length-prefix array
//! elements, so skipping past the operations array requires fully
//! type-aware decoding of every operation variant, which is out of scope
//! for this header parser. Envelopes with one or more operations return
//! [`XdrParseError::UnsupportedOperations`].

use std::convert::TryInto;

/// XDR discriminant for `ENVELOPE_TYPE_TX` (`TransactionV1Envelope`).
pub const ENVELOPE_TYPE_TX: u32 = 2;

/// Minimum byte length of a `TransactionV1Envelope` with no time bounds,
/// no memo, and no operations (envelope type + account type + 32-byte key
/// + fee + sequence number + preconditions discriminant + memo discriminant
/// + operation count + ext discriminant + signature count).
pub const MIN_TX_V1_LEN: usize = 72;

const ACCOUNT_TYPE_ED25519: u32 = 0;

const PRECOND_NONE: u32 = 0;
const PRECOND_TIME: u32 = 1;

const MEMO_NONE: u32 = 0;
const MEMO_TEXT: u32 = 1;
const MEMO_ID: u32 = 2;
const MEMO_HASH: u32 = 3;
const MEMO_RETURN: u32 = 4;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum XdrParseError {
    #[error("buffer too short: need at least {need} bytes, have {have}")]
    Truncated { need: usize, have: usize },
    #[error("unsupported envelope type: {0}")]
    UnsupportedEnvelopeType(u32),
    #[error("unsupported source account type: {0}")]
    UnsupportedAccountType(u32),
    #[error("unsupported preconditions discriminant: {0}")]
    UnsupportedPreconditions(u32),
    #[error("unsupported memo discriminant: {0}")]
    UnsupportedMemo(u32),
    #[error("operation decoding is out of scope for this header parser ({0} present)")]
    UnsupportedOperations(u32),
}

/// Borrowed view over a parsed `TransactionV1Envelope` header. Variable-length
/// data are slices into the original input — parsing performs no allocation.
#[derive(Debug, PartialEq, Eq)]
pub struct Envelope<'a> {
    pub source_account: &'a [u8; 32],
    pub fee: u32,
    pub sequence_number: i64,
    pub time_bounds: Option<(u64, u64)>,
    pub memo: Memo<'a>,
    pub operation_count: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Memo<'a> {
    None,
    Text(&'a [u8]),
    Id(u64),
    Hash(&'a [u8; 32]),
    Return(&'a [u8; 32]),
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], XdrParseError> {
        if self.buf.len() - self.pos < n {
            return Err(XdrParseError::Truncated { need: self.pos + n, have: self.buf.len() });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, XdrParseError> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn i64(&mut self) -> Result<i64, XdrParseError> {
        let s = self.take(8)?;
        Ok(i64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }

    fn u64(&mut self) -> Result<u64, XdrParseError> {
        let s = self.take(8)?;
        Ok(u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }

    fn array32(&mut self) -> Result<&'a [u8; 32], XdrParseError> {
        let slice = self.take(32)?;
        slice
            .try_into()
            .map_err(|_| XdrParseError::Truncated { need: 32, have: slice.len() })
    }
}

/// XDR pads opaque/string data out to a 4-byte boundary.
fn padding(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

/// Parse the header of a `TransactionV1Envelope` out of raw XDR bytes.
pub fn parse_envelope(raw: &[u8]) -> Result<Envelope<'_>, XdrParseError> {
    let mut cur = Cursor::new(raw);

    let envelope_type = cur.u32()?;
    if envelope_type != ENVELOPE_TYPE_TX {
        return Err(XdrParseError::UnsupportedEnvelopeType(envelope_type));
    }

    let account_type = cur.u32()?;
    if account_type != ACCOUNT_TYPE_ED25519 {
        return Err(XdrParseError::UnsupportedAccountType(account_type));
    }
    let source_account = cur.array32()?;

    let fee = cur.u32()?;
    let sequence_number = cur.i64()?;

    let precond = cur.u32()?;
    let time_bounds = match precond {
        PRECOND_NONE => None,
        PRECOND_TIME => Some((cur.u64()?, cur.u64()?)),
        other => return Err(XdrParseError::UnsupportedPreconditions(other)),
    };

    let memo_type = cur.u32()?;
    let memo = match memo_type {
        MEMO_NONE => Memo::None,
        MEMO_TEXT => {
            let len = cur.u32()? as usize;
            let bytes = cur.take(len)?;
            cur.take(padding(len))?;
            Memo::Text(bytes)
        }
        MEMO_ID => Memo::Id(cur.u64()?),
        MEMO_HASH => Memo::Hash(cur.array32()?),
        MEMO_RETURN => Memo::Return(cur.array32()?),
        other => return Err(XdrParseError::UnsupportedMemo(other)),
    };

    let operation_count = cur.u32()?;
    if operation_count != 0 {
        return Err(XdrParseError::UnsupportedOperations(operation_count));
    }

    Ok(Envelope { source_account, fee, sequence_number, time_bounds, memo, operation_count })
}

/// Pool of reusable byte buffers so repeated parse/build cycles (e.g. request
/// handling under load) avoid re-allocating a fresh `Vec` every time.
pub mod pool {
    use std::sync::Mutex;

    pub struct BufferPool {
        capacity: usize,
        buffers: Mutex<Vec<Vec<u8>>>,
    }

    impl BufferPool {
        pub fn new(capacity: usize) -> Self {
            Self { capacity, buffers: Mutex::new(Vec::with_capacity(capacity)) }
        }

        pub fn acquire(&self) -> Vec<u8> {
            self.buffers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop()
                .unwrap_or_default()
        }

        pub fn release(&self, mut buf: Vec<u8>) {
            buf.clear();
            let mut buffers = self.buffers.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if buffers.len() < self.capacity {
                buffers.push(buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BufMut;

    fn minimal_envelope_bytes() -> Vec<u8> {
        let mut buf = Vec::with_capacity(MIN_TX_V1_LEN);
        buf.put_u32(ENVELOPE_TYPE_TX);
        buf.put_u32(0);
        buf.put_bytes(0x42, 32);
        buf.put_u32(100);
        buf.put_i64(9_999_999);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf
    }

    #[test]
    fn parses_minimal_envelope() {
        let raw = minimal_envelope_bytes();
        let env = parse_envelope(&raw).expect("parse failed");
        assert_eq!(env.fee, 100);
        assert_eq!(env.sequence_number, 9_999_999);
        assert_eq!(env.source_account, &[0x42u8; 32]);
        assert_eq!(env.time_bounds, None);
        assert_eq!(env.memo, Memo::None);
        assert_eq!(env.operation_count, 0);
        assert_eq!(raw.len(), MIN_TX_V1_LEN);
    }

    #[test]
    fn rejects_truncated_buffer() {
        let raw = minimal_envelope_bytes();
        let err = parse_envelope(&raw[..10]).unwrap_err();
        assert!(matches!(err, XdrParseError::Truncated { .. }));
    }

    #[test]
    fn rejects_unsupported_envelope_type() {
        let mut raw = minimal_envelope_bytes();
        raw[3] = 5; // envelope type = TX_FEE_BUMP
        let err = parse_envelope(&raw).unwrap_err();
        assert_eq!(err, XdrParseError::UnsupportedEnvelopeType(5));
    }

    #[test]
    fn rejects_nonzero_operations() {
        let mut raw = minimal_envelope_bytes();
        let ops_offset = raw.len() - 8; // operation count precedes ext discriminant
        raw[ops_offset + 3] = 1;
        let err = parse_envelope(&raw).unwrap_err();
        assert_eq!(err, XdrParseError::UnsupportedOperations(1));
    }

    #[test]
    fn parses_time_bounds_precondition() {
        let mut buf = Vec::new();
        buf.put_u32(ENVELOPE_TYPE_TX);
        buf.put_u32(0);
        buf.put_bytes(0x01, 32);
        buf.put_u32(100);
        buf.put_i64(1);
        buf.put_u32(PRECOND_TIME);
        buf.put_u64(1_000);
        buf.put_u64(2_000);
        buf.put_u32(MEMO_NONE);
        buf.put_u32(0);

        let env = parse_envelope(&buf).expect("parse failed");
        assert_eq!(env.time_bounds, Some((1_000, 2_000)));
    }

    #[test]
    fn buffer_pool_reuses_released_buffers() {
        let pool = pool::BufferPool::new(2);
        let mut buf = pool.acquire();
        buf.put_slice(b"hello");
        let cap = buf.capacity();
        pool.release(buf);

        let reused = pool.acquire();
        assert!(reused.is_empty());
        assert_eq!(reused.capacity(), cap);
    }
}
