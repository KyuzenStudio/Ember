# Keterbatasan yang diketahui (v0.1, penutupan 2026-10-16)

## Performa: belum diukur

- Tidak ada benchmark VM (instruksi/detik), renderer (fps/scene), atau waktu load project.
- Fisika memakai fixed timestep 1/60 + akumulator; benar secara simulasi, biaya CPU per
  step belum diprofilkan. Batas 1024 sprite belum diuji bebannya.

## Fuzzing: belum ada

- Tidak ada fuzz input untuk parser, checker, verifier bytecode, loader RON, atau decoder PNG.
- Input malformed ditangani lewat error eksplisit + tes kasus, bukan fuzz.

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
