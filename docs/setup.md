# Setup

## Prasyarat

- Rust stable terbaru via `rustup` (`rustc 1.99.0` teruji).
- Target `x86_64-pc-windows-msvc` (default di Windows MSVC).
- Windows x86-64 untuk run windowed (window, GPU, editor, player).
- Tes unit compiler/VM/scene headless (jalan di mana saja).

## Perintah

```powershell
cargo build --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
powershell -ExecutionPolicy Bypass -File scripts\check.ps1
```

`--locked` wajib: `Cargo.lock` di-commit agar versi dependency terkunci.

## Alur kerja game

```powershell
ember new MyGame        # project starter valid
ember check MyGame      # syntax + nama + tipe, tanpa window
ember build MyGame      # bytecode build/*.embc terverifikasi
ember package MyGame    # dist/MyGame-windows-x86_64 (player + bytecode + aset)
ember-editor MyGame     # editor visual (Save, Play, Stop)
```

## Masalah umum

- `cargo build --locked` gagal setelah tambah crate: hapus flag sekali (`cargo build`) untuk regenerasi lock, lalu commit hasilnya.
- `ExecutionPolicy`: jalankan `check.ps1` dengan `-ExecutionPolicy Bypass`.
- Contoh windowed (`sprite`, player, editor) butuh display dan GPU. Tanpa itu hanya `--version` yang bisa jalan.
