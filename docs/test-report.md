# Test report — 2026-10-16

Platform: Windows 11 x64, `x86_64-pc-windows-msvc`, rustc 1.99.0.
Perintah: `cargo test --locked`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo fmt --check`, `scripts\check.ps1`. Semua hijau.

| Crate | Tes | Lulus | Gagal |
| --- | --- | --- | --- |
| ember-cli | 16 | 16 | 0 |
| ember-core | 4 | 4 | 0 |
| ember-editor | 12 | 12 | 0 |
| ember-lang | 60 | 60 | 0 |
| ember-player | 3 | 3 | 0 |
| ember-project | 21 | 21 | 0 |
| ember-renderer | 14 | 14 | 0 |
| ember-runtime | 10 | 10 | 0 |
| ember-scene | 9 | 9 | 0 |
| ember-vm | 36 | 36 | 0 |
| **Total** | **185** | **185** | **0** |

Cakupan: lexer/span, parser presedensi dan input invalid, name resolution,
type checking, kompilasi bytecode, verifier malformed, VM (aritmetika, branch,
call, host call, budget, error lokasi), scene round-trip ID stabil, validasi path,
project new/build/package, editor mapping/picking/play-stop, render offscreen
dengan readback piksel GPU.

Di luar tes otomatis: contoh windowed M1, `ember-player` windowed, editor windowed,
dan Play windowed belum dijalankan sesi interaktif oleh developer. Lihat `limits.md`.
