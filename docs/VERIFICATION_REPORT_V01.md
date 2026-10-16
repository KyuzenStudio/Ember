# Verification Report — v0.1 keseluruhan (M0–M14)

Tanggal: 2026-10-16
Commit kode: `f56fdf4` (penutupan docs ini belum di-commit, diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Seluruh fase v0.1 selesai: fondasi, window+sprite, scene, CLI, lexer/parser, semantik,
bytecode+verifier, VM, runtime, player+packaging, editor, aset+diagnostik, Play/Stop,
fisika 2D opsional, hardening. Gate penutupan dijalankan dari kompilasi bersih:
196 tes lulus, 0 gagal; clippy dan fmt bersih. Pipeline `new → check → package`
berjalan dan exe package hidup windowed tanpa source.

## Perubahan (penutupan ini, tanpa fitur baru)

- `AGENT.md` bagian 7: 15/15 butir dicentang, masing-masing dengan referensi bukti.
- `docs/KNOWN_LIMITATIONS.md` (baru): performa, fuzz, CI, verifikasi manual, hal belum terverifikasi.
- `docs/acceptance.md`, `docs/limits.md`, `docs/stability-proof.md`: disinkronkan sesi sebelumnya.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| `cargo build --locked` | lulus | dari nol, selesai 1m18s |
| `cargo test --locked` | 196 lulus, 0 gagal, 0 diabaikan | 16+4+13+60+4+21+14+18+10+36 per suite |
| `cargo clippy --all-targets --locked -- -D warnings` | bersih | 0 peringatan |
| `cargo fmt --check` | lulus | exit 0 |
| `cargo tree --locked` (pindai llvm/cranelift/jit) | 0 kena | tanpa native compiler |
| `ember new CloseGame` | lulus | 5 file terbuat |
| `ember check CloseGame` | lulus | `ok: 1 script(s) checked`, tanpa window |
| `ember build CloseGame` | lulus | `build/player.embc` |
| `ember package CloseGame` | lulus | folder `CloseGame-windows-x86_64`: exe + `.embc` + aset + scene |
| jalankan `ember-player.exe` package, 10 detik | hidup, tanpa stderr, tanpa crash | tanpa source, tanpa editor; lalu dihentikan |

## Demonstrasi

Project `CloseGame` dibuat di direktori temporer (di luar repo, sudah dihapus),
melewati seluruh pipeline, dan exe hasil package dijalankan windowed dari foldernya
sendiri (tanpa file `.em`). Proses tetap hidup 10 detik tanpa keluaran error.
Interaksi keyboard dan exit rapi (Escape) tidak diamati sesi ini.

## Acceptance checklist yang terpengaruh

15/15 butir `AGENT.md` bagian 7 terpenuhi; masing-masing mencantumkan bukti
(tes bernama, demo, atau skenario penguji S1–S12). Tidak ada butir kosong.

## Belum diverifikasi

- Uji ulang S5 windowed oleh penguji (interaksi + exit rapi player dari package).
- Performa VM/renderer/fisika (belum diukur). Fuzz input malformed. CI GitHub.
- Build `--release`, GPU selain milik penguji, multi-monitor, minimize/restore saat Play.
- Detail OS/GPU/driver milik penguji tidak tercatat (hanya "Windows").

## Masalah dan risiko yang diketahui

- `cargo clean` gagal: `target/debug` dikunci proses lain (os error 32, bukan proses ember).
  Kompilasi bersih dilakukan via `CARGO_TARGET_DIR` temporer — seluruh artefak dari nol.
- Satu bug nyata ditemukan dan diperbaiki sebelum penutupan: sisa dt sub-step dibuang
  sehingga sim diam di 120Hz+; kini diakumulasi + tes `substep_dt_accumulates`.
- `DemoGame/` milik penguji, untracked, tetap di luar commit.

## Asumsi yang dibuat

- Hasil S1–S12 penguji (ditdev, `docs/stability-proof.md`) diterima sebagai bukti windowed.
- "Hidup 10 detik tanpa error" diterima sebagai bukti peluncuran windowed package,
  bukan bukti permainan dapat dimainkan penuh.

## Rekomendasi langkah berikutnya

1. Penguji jalankan ulang S5: gerak 4 arah + resize + Escape, catat exit code.
2. Ukur performa dasar (fps scene 1024 sprite, waktu load) sebelum klaim rilis.
3. Nyalakan CI (build+test+clippy+fmt) dan catat run pertama.
4. Putuskan penomoran rilis/tag secara tertulis — tag TIDAK dibuat sesi ini sesuai instruksi.
