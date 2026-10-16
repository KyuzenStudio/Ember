use std::panic::AssertUnwindSafe;

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

const SEED_PROGRAMS: &[&str] = &[
    "var speed: f32 = 6.0\nfn on_update(dt: f32) {\nif input.down(Key.Right) {\ntransform.move(speed * dt, 0.0)\n}\n}\n",
    "var n: i32 = 0\nfn on_create() {\nlog(\"hi\")\n}\nfn on_update(dt: f32) {\nn = n + 1\n}\n",
    "fn f(x: i32) {\nif x > 0 {\nlog(\"pos\")\n} else {\nlog(\"neg\")\n}\n}\n",
];

const ALPHABET: &[u8] = b"var fn{}()=:,\"<>.+-*/! \n0123456789_true";

fn mutate(state: &mut u64, src: &[u8]) -> Vec<u8> {
    let mut out = src.to_vec();
    let ops = 1 + (xorshift(state) % 3) as usize;
    for _ in 0..ops {
        if out.is_empty() {
            out.push(ALPHABET[(xorshift(state) as usize) % ALPHABET.len()]);
            continue;
        }
        match xorshift(state) % 4 {
            0 => {
                let i = (xorshift(state) as usize) % out.len();
                out[i] = ALPHABET[(xorshift(state) as usize) % ALPHABET.len()];
            }
            1 => {
                let i = (xorshift(state) as usize) % out.len();
                let n = 1 + (xorshift(state) as usize) % 8;
                out.drain(i..(i + n).min(out.len()));
            }
            2 => {
                let i = (xorshift(state) as usize) % (out.len() + 1);
                let b = ALPHABET[(xorshift(state) as usize) % ALPHABET.len()];
                out.insert(i, b);
            }
            _ => {
                let n = (xorshift(state) as usize) % (out.len() + 1);
                out.truncate(n);
            }
        }
    }
    out
}

fn run_pipeline(src: &str) -> Result<(), String> {
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let program = match ember_lang::parse("fuzz", src) {
            Ok(p) => p,
            Err(e) => return Err(format!("parse: {e}")),
        };
        let _ = program;
        match ember_lang::semantic::check("fuzz", src) {
            Ok(_) => (),
            Err(e) => return Err(format!("check: {}", e[0])),
        }
        match ember_lang::compile::compile("fuzz", src) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("compile: {}", e[0])),
        }
    }));
    match caught {
        Ok(r) => r,
        Err(_) => Err("panic".to_string()),
    }
}

const CORPUS: &[&str] = &[
    "var x = \n",
    "var : i32 = 1\n",
    "fn f( { }\n",
    "\"unterminated\n",
    "var x: i32 = true\n",
    "unknown_fn()\n",
    "if 1 { }\n",
    "1 + 2\n",
    "return 1\n",
    "var x: i32 = 1\nvar x: i32 = 2\n",
    "fn on_update(dt: f32) { transform.move(1.0) }\n",
    "var x: f32 = 1\n",
    "while true { var y: i32 = }\n",
    "fn f(x: i32) { }\nf(1, 2)\n",
    "var s: string = 1\n",
];

#[test]
fn corpus_rejected_without_panic() {
    for (i, src) in CORPUS.iter().enumerate() {
        let r = run_pipeline(src);
        assert!(
            r.is_err(),
            "corpus #{i} unexpectedly accepted: {src:?} (panics fail here too)"
        );
        assert!(
            !r.unwrap_err().contains("panic"),
            "corpus #{i} panicked: {src:?}"
        );
    }
}

#[test]
fn fuzz_never_panics() {
    let iters = fuzz_iters();
    let mut state = SEED;
    let mut failures = Vec::new();
    for i in 0..iters {
        let seed = SEED_PROGRAMS[(xorshift(&mut state) as usize) % SEED_PROGRAMS.len()];
        let bytes = mutate(&mut state, seed.as_bytes());
        let src = String::from_utf8_lossy(&bytes);
        if let Err(e) = run_pipeline(&src)
            && e == "panic"
        {
            failures.push(format!("iter {i}: {src:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "panics (repro: SEED={SEED:#x}):\n{}",
        failures.join("\n")
    );
}
