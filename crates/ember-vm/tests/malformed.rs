use ember_vm::{Chunk, Function, SpanEntry, Vm, decode, encode, op, verify};

const SEED: u64 = 0xC10A_5EED;
const DEFAULT_ITERS: usize = 512;

fn fuzz_iters() -> usize {
    std::env::var("EMBER_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_ITERS)
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn valid_chunk() -> Chunk {
    Chunk {
        source_file: "fuzz".to_string(),
        strings: Vec::new(),
        globals: Vec::new(),
        functions: vec![Function {
            name: "main".to_string(),
            params: 0,
            locals: Vec::new(),
            code: vec![op::RETURN],
            spans: vec![SpanEntry {
                offset: 0,
                line: 1,
                col: 1,
            }],
        }],
    }
}

fn mutate(state: &mut u64, src: &[u8]) -> Vec<u8> {
    let mut out = src.to_vec();
    match xorshift(state) % 4 {
        0 => {
            if !out.is_empty() {
                let i = (xorshift(state) as usize) % out.len();
                out[i] = xorshift(state) as u8;
            }
        }
        1 => {
            let n = (xorshift(state) as usize) % (out.len() + 1);
            out.truncate(n);
        }
        _ => {
            let n = 1 + (xorshift(state) as usize) % 16;
            for _ in 0..n {
                out.push(xorshift(state) as u8);
            }
        }
    }
    out
}

#[test]
fn valid_roundtrip() {
    let bytes = encode(&valid_chunk());
    let chunk = decode(&bytes).expect("valid bytes decode");
    verify(&chunk).expect("valid chunk verifies");
    Vm::new(chunk).expect("valid chunk builds vm");
}

#[test]
fn fixed_malformed_rejected() {
    assert!(decode(&[]).is_err());
    assert!(decode(b"XXXX\x01\x00").is_err());
    let mut bad_version = encode(&valid_chunk());
    bad_version[4] = 0xFF;
    bad_version[5] = 0xFF;
    assert!(decode(&bad_version).is_err());
}

#[test]
fn fuzz_never_panics_and_rejects() {
    let iters = fuzz_iters();
    let base = encode(&valid_chunk());
    let mut state = SEED;
    let mut panics = Vec::new();
    let mut accepted = 0usize;
    for i in 0..iters {
        let bytes = mutate(&mut state, &base);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let chunk = match decode(&bytes) {
                Ok(c) => c,
                Err(_) => return false,
            };
            if verify(&chunk).is_err() {
                return false;
            }
            Vm::new(chunk).is_ok()
        }));
        match r {
            Err(_) => panics.push(i),
            Ok(true) => accepted += 1,
            Ok(false) => {}
        }
    }
    assert!(
        panics.is_empty(),
        "panics at iters (repro SEED={SEED:#x}): {panics:?}"
    );
    assert!(
        accepted * 20 < iters,
        "too many mutated blobs accepted: {accepted}/{iters}"
    );
}
