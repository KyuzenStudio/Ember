# Verification Report — Milestone 15: utang v0.1

Tanggal: 2026-10-16
Commit: `2adef93` + perubahan M15 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Milestone 15 menutup utang v0.1: CI Windows, fuzz malformed, dan baseline performa.
Workflow CI sudah ada dan langkahnya dijalankan hijau secara lokal. Fuzz korpus +
generator deterministik (seed tetap, stable) berjalan tanpa panic dan tanpa temuan.
Baseline performa tertulis ke `docs/PERF_BASELINE.md` termasuk frame GPU riil.

## Perubahan

- `.github/workflows/ci.yml`: sudah ada sebelumnya (tidak diubah); diverifikasi
  langkahnya identik dengan gate lokal yang hijau.
- `crates/ember-lang/tests/malformed.rs` (baru): 15 korpus malformed ditolak tanpa
  panic + fuzz 512 iterasi (xorshift64, SEED tetap, dapat direproduksi dari seed).
- `crates/ember-vm/tests/malformed.rs` (baru): roundtrip valid + kasus fixed
  (kosong, magic salah, versi salah) + fuzz 512 mutasi byte atas chunk valid.
- `crates/ember-runtime/tests/perf_baseline.rs` (baru): ukur VM update, fisika,
  kompilasi, draw count, frame GPU; tulis `docs/PERF_BASELINE.md`.
- `crates/ember-runtime/Cargo.toml`: tambah dev-dependencies
  (ember-renderer path, pollster 1, wgpu 30, image 0.25/png) — hanya untuk tes
  perf, bukan fitur. Lisensi (tercatat di manifest registry): pollster
  Apache-2.0/MIT; wgpu MIT OR Apache-2.0; image MIT OR Apache-2.0
  (dependensi ini sudah ada di tree via ember-renderer, pin sama).
  `Cargo.lock`: +4 entri dev-only terkait.
- `docs/PERF_BASELINE.md` (ditulis oleh tes): tabel debug + release berlabel.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 202 lulus, 0 gagal, 0 diabaikan | 196 lama + 2 lang fuzz + 3 vm fuzz + 1 perf |
| cargo clippy --all-targets --locked -- -D warnings | bersih | 1 temuan `collapsible_if` diperbaiki saat sesi |
| cargo fmt --check | lulus | |
| langkah CI (.github/workflows/ci.yml) | hijau lokal | run GitHub-hosted butuh push; tautan dicatat di sini setelah ada — syarat BELUM terpenuhi |

## Demonstrasi

- `cargo test -p ember-lang --test malformed`: 2 lulus (korpus 15/15 ditolak, fuzz 512 tanpa panic).
- `cargo test -p ember-vm --test malformed`: 3 lulus (roundtrip valid, fixed cases ditolak, fuzz 512 tanpa panic, tingkat terima mutasi di bawah ambang).
- `cargo test -p ember-runtime --test perf_baseline`: 1 lulus; baseline (debug,
  windows x86_64): VM update 335.44 us/tick; fisika 50 body 792.39 us/tick;
  kompilasi 50.9 us; 200 draws; frame GPU 4.707 ms/frame (200 sprite, offscreen
  800x600). Profil release: VM 67.18 us/tick; fisika 25.66 us/tick; kompilasi
  8.0 us; draws 28.9 us; frame GPU 3.872 ms/frame.
- Fuzz: temuan nihil. Korpus + seed tercatat di file tes untuk reproduksi;
  iterasi default 512/crate, dapat dinaikkan via `EMBER_FUZZ_ITERS`.

## Acceptance checklist yang terpengaruh

- [ ] CI Windows hijau tercatat — hijau lokal; tautan run GitHub menunggu push (belum terpenuhi penuh).
- [x] Fuzz/corpus malformed tidak menjatuhkan lexer/parser/verifier — bukti: 5 tes fuzz/korpus lulus, temuan nihil. (Checklist v0.2 butir ini menunggu status final saat M22; bukti dicatat di sini.)
- [x] Baseline performa tercatat — bukti: `docs/PERF_BASELINE.md` (angka, profil debug).

## Belum diverifikasi

- Run CI di GitHub-hosted runner (butuh push; workflow file siap).
- Angka profil release (baseline ini profil debug).
- Frame di window nyata vsync (baseline ini offscreen tanpa present).

## Masalah dan risiko yang diketahui

- `target/` repo dikunci proses lain seperti sebelumnya; gate memakai `CARGO_TARGET_DIR` temporer.
- Angka baseline khusus mesin ini (debug); perbandingan antar-mesin belum berarti.
- Fuzz 512 iterasi/sesi: cukup untuk regresi, bukan eksplorasi mendalam.

## Asumsi yang dibuat

- Langkah CI lokal = langkah workflow (file tidak diubah, isi dibandingkan).
- Tidak ada ambang performa yang digagalkan; tes hanya mencatat + cek kewarasan.

## Rekomendasi langkah berikutnya

1. Push → catat tautan run CI hijau pertama, lalu centang penuh butir CI.
2. M16 (host API v2) menunggu keputusan display (opsi B di `DESIGN.md` §21).
3. Ukur ulang baseline profil release sebelum M22.
