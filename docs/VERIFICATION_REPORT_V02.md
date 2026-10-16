# Verification Report — v0.2: membuat game sungguhan (Coin Dash)

Tanggal: 2026-10-16
Commit: (diisi saat commit)
Platform: Windows x86-64, rustc 1.99.0 (b940084d7 2026-09-28)

## Ringkasan

v0.2 ditutup dengan satu game contoh non-trivial yang selesai dimainkan dari
awal sampai menang, dari editor dan dari package. Semua acceptance 7b
terpenuhi kecuali CI Windows (terblokir payment; pengganti: seluruh langkah
CI dijalankan lokal dan hijau). Nol perubahan grammar/bytecode/host API
selama M15–M21; larangan bahasa utuh.

## Milestone yang dicakup

| M | Hasil | Laporan |
| --- | --- | --- |
| 15 | Utang v0.1: CI lokal, fuzz, baseline | `docs/VERIFICATION_REPORT_M15.md` |
| 16 | Host API v2 (15 fungsi) + collision lifecycle | `docs/VERIFICATION_REPORT_M16.md` |
| 17 | Bukti bahasa cukup via mekanik Coin Dash | `docs/VERIFICATION_REPORT_M17.md` |
| 18 | Animasi sprite + ganti klip dari script | `docs/VERIFICATION_REPORT_M18.md` |
| 19 | Audio dasar (WAV/Vorbis, SFX + loop) | `docs/VERIFICATION_REPORT_M19.md` |
| 20 | Editor v2 (drag, undo/redo, multi-select) + Inspector Unity-style + scene v5 multi-script | `docs/VERIFICATION_REPORT_M20.md` |
| 21 | Coin Dash mainable + tes paket/audio | `docs/VERIFICATION_REPORT_M21.md` |

## Perintah penutup yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| `cargo test --locked --workspace` | 249 lulus, 0 gagal | target dir temporer, build bersih |
| `cargo clippy --all-targets --locked -- -D warnings` | bersih | |
| `cargo fmt --check` | lulus | |
| `cargo tree --locked` (M15–v0.1) | tanpa llvm/cranelift/jit | larangan native utuh |

## Demonstrasi penutup

- Playtest manual penguji di editor: char gerak (Kanan/Kiri + Spasi),
  menang, musik + SFX keluar. Aman.
- Playtest manual penguji di exe paket: sama, menang. Aman.
- Sebelumnya (M21): exe paket headless hidup ±10 detik tanpa stderr dan
  satu run mencapai `WIN` end-to-end tanpa source.

## Acceptance checklist v0.2 (final)

- [ ] CI Windows hijau — BELUM: payment GitHub terblokir. Pengganti: build +
  test + clippy + fmt dijalankan lokal, tercatat hijau di tiap laporan M15–M21.
- [x] Fuzz/corpus malformed tidak menjatuhkan lexer/parser/verifier.
  Bukti: M15, 512 iterasi deterministik, nihil temuan.
- [x] Baseline performa tercatat. Bukti: `docs/PERF_BASELINE.md`.
- [x] Script membaca posisi entity lain + kamera ikut player. Bukti:
  `samples/coin-dash/scripts/cam.em`, tes samakan Cam/Player, manual.
- [x] `input.pressed` lompat tepat sekali per tekan. Bukti: tes
  `jump_rises_and_lands`, manual Spasi.
- [x] Koin hilang + skor (collision). Bukti: `score:1..3`, manual.
- [x] Musuh patroli + nyawa + respawn. Bukti: tes patroli + `lives:*`,
  manual sampai `GAME OVER`.
- [x] Animasi jalan + ganti klip dari script. Bukti: tes `anim_switch_*`,
  `idle`/`walk` di sampel (panggil hanya saat transisi).
- [x] SFX + musik loop di Play dan package. Bukti: tes antre audio +
  decode wav, manual dengar di editor dan paket.
- [x] `ember package` sertakan audio; paket bersuara tanpa source. Bukti:
  tes paket (4 wav + 3 embc, tanpa `scripts/`), tes load tanpa source,
  exe hidup 10 detik.
- [x] Drag, undo/redo, multi-select tanpa merusak scene. Bukti: M20, 4 tes.
- [x] Coin Dash awal–menang dari editor dan package. Bukti: playthrough
  headless + WIN exe paket + manual dua-duanya.
- [x] Tanpa fitur bahasa di luar kebutuhan; larangan utuh. Bukti: nol
  perubahan grammar/bytecode/host API M15–M21.

## Belum diverifikasi

- Run CI GitHub nyata (payment). Wajib sebelum klaim CI hijau.
- Tes speaker di mesin lain / OGG langsung (sampel pakai WAV).
- Preview anim di Inspector (runtime terbukti; UI preview belum ada).

## Masalah dan risiko yang diketahui

- `docs/PERF_BASELINE.md` ditulis ulang tiap run tes (by design).
- Log `WIN` bisa ganda (kontak darat pasca-menang); idempoten, kosmetik.
- Penghapusan worktree tak-ter-stage (`assets/icons/apps/*`,
  `.github/workflows/ci.yml`) milik reorganisasi user; sengaja tak
  di-commit. Sama untuk untracked eksternal (`.github-frezee/`,
  `assets/music/`, `assets/sprite/`, `assets/icons/*`).
- Stability gate native: belum dibuka (butuh keputusan tertulis pemilik).

## Rekomendasi langkah berikutnya

1. Selesaikan payment → run CI → centang butir CI.
2. v0.3 sesuai `PRODUCT.md` berikutnya; jaga disiplin milestone kecil.
