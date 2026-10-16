# Verification Report — Milestone 19: audio dasar

Tanggal: 2026-10-16
Commit: `8814f4b` + perubahan M19 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Audio dasar selesai: crate `ember-audio` di atas rodio 0.23.0 (fitur
`playback`+`wav`+`vorbis`), SFX satu-tembak + musik loop + stop, validasi path/
ekstensi/header yang selalu jalan termasuk tanpa device (noop mencatat).
Antre `take_audio` dikonsumsi Play editor dan player; error tampil di log.
Aset audio ikut package otomatis (seluruh `assets/` disalin).

## Perubahan

- `crates/ember-audio` (baru): engine + validasi + 4 tes. rodio 0.23 API baru
  (`MixerDeviceSink`/`Player`; tidak ada `OutputStream`/`Sink`).
- `crates/ember-editor` + `crates/ember-player`: dependensi + engine malas +
  konsumsi antre per tick/frame; tes Play editor end-to-end (wav asli).
- `Cargo.lock`: rodio 0.23.0 + tree-nya (cpal dkk).
- `DESIGN.md`: catatan implementasi M19.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 232 lulus, 0 gagal, 0 diabaikan | +5 dari M18 (4 audio, 1 editor) |
| cargo clippy --all-targets --locked -- -D warnings | bersih | |
| cargo fmt --check | lulus | |

## Demonstrasi

- WAV valid (dibuat per-bita di tes) diterima + tercatat; file hilang sebut path;
  `.mp3` ditolak; isi sampah ditolak sebagai tak ter-decode.
- Script `audio.play("assets/s.wav")` di Play editor → engine mencatat; tanpa
  device pun validasi tetap jalan (is_live dilaporkan).

## Acceptance checklist yang terpengaruh

Butir v0.2 terkait (tetap kosong sampai M22): SFX + loop di Play dan package;
package tanpa source — mekanisme terbukti, konten + dengar nyata di M21.

## Belum diverifikasi

- Bunyi nyata di speaker (butuh dengar manusia, M21).
- OGG ter-decode (hanya WAV diuji; decoder sama-sama symphonia per fitur).
- Musik loop pergantian trek dan volume.

## Masalah dan risiko yang diketahui

- rodio 0.23 API berbeda dari dokumentasi lama; pin di lockfile.
- Tanpa device audio: sunyi tapi validasi tetap jalan (disengaja).

## Asumsi yang dibuat

- Satu trek musik cukup (sesuai desain); `stop` sebelum `loop` berikutnya.

## Rekomendasi langkah berikutnya

1. M20 (editor v2): drag viewport, undo/redo, multi-select (+ authoring klip).
2. M21: konten WAV/OGG nyata + dengar manusia.
