# Keterbatasan yang diketahui (v0.1 + M15, 2026-10-16)

## Performa: baseline awal, bukan benchmark

- `docs/PERF_BASELINE.md` (profil debug + release, mesin penulis): VM update,
  fisika 50 body, kompilasi, draw count 200 sprite, frame GPU offscreen.
- Bukan klaim umum: angka khusus mesin + profil; beban 1024 sprite dan waktu load
  project belum diukur; frame window nyata vsync belum diukur.

## Fuzzing: cakupan kecil

- M15: korpus malformed + generator deterministik (seed tetap) untuk
  lexer/parser/checker/compiler (`ember-lang`) dan decode/verifier/VM
  (`ember-vm`); 512 iterasi per crate secara default, temuan nihil.
- Cakupan kecil: naikkan lewat variabel lingkungan `EMBER_FUZZ_ITERS`
  (mis. `EMBER_FUZZ_ITERS=10000 cargo test -p ember-lang --test malformed`).
  Seed tetap sehingga kegagalan dapat direproduksi dari seed + indeks.
- Belum ada fuzz untuk loader RON, decoder PNG, dan loader scene.

## CI: belum ada

- Belum ada run GitHub CI tercatat. Gate yang dijalankan: build + test + clippy
  (`-D warnings`) + `fmt --check` lokal, dari direktori target bersih (lihat
  `docs/VERIFICATION_REPORT_V01.md`).

## Verifikasi manual penguji

- Penguji: ditdev. Lingkungan: Windows (detail OS/GPU/driver tidak dicatat penguji).
- Hasil: S1–S4, S8–S12 lulus (tercatat di `docs/stability-proof.md`, ditandatangani 16-10-2026).
- S5 (player dari package) GAGAL saat itu: `script 'scripts/player.em' not found`.
  Akar sudah diperbaiki (player/package muat tanpa sumber `.em`, ada tes regresi);
  uji ulang S5 windowed oleh penguji masih tertunda. Penulis memverifikasi 2026-10-16:
  exe package windowed hidup 10 detik tanpa source, tanpa stderr, tanpa crash —
  interaksi dan exit rapi (Escape) belum diverifikasi.

## Hal lain yang belum diverifikasi

- Exit rapi player via Escape (kode ada, belum dieksekusi terpantau).
- Build `--release` untuk package (package memakai biner debug/dev).
- GPU selain milik penguji; perilaku vsync/refresh-rate selain yang dilaporkan (bug
  120Hz+ sudah diperbaiki via akumulator + tes `substep_dt_accumulates`).
- Multi-window, multi-monitor, minimize/restore saat Play.
- Upgrade Rust toolchain di atas 1.99.0; dependensi baru di luar `Cargo.lock`.
- Skala project: ratusan entity, script besar, PNG besar.
