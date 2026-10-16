# Verification Report — Milestone 17: bahasa v2

Tanggal: 2026-10-16
Commit: `5049935` + perubahan M17 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Vonis M17: **tidak ada perubahan grammar atau bytecode**. Seluruh mekanik Coin
Dash (lompat, koin+skor, musuh+nyawa, patroli, kamera ikut, menang) terbukti
berjalan di atas bahasa v0.1 + Host API v2 lewat skenario headless
`coin_dash.rs` (4 tes). Satu bug nyata ditemukan saat sesi: output host dari
callback collision (destroy/audio/dll) dibuang — diperbaiki + teruji.

## Perubahan

- `crates/ember-runtime/tests/coin_dash.rs` (baru): skenario 4 tes.
- `crates/ember-runtime/src/runtime.rs`: dispatch collision kini kumpulkan
  audio/anim/title/restart/destroy (sebelumnya hanya logs); destroy dari
  callback diterapkan setelah dispatch.
- Tidak ada perubahan grammar, bytecode, opcode, atau versi format.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 220 lulus, 0 gagal, 0 diabaikan | +4 dari M16 |
| cargo clippy --all-targets --locked -- -D warnings | bersih | |
| cargo fmt --check | lulus | |

## Demonstrasi

- `jump_rises_and_lands`: `input.pressed` teleport +2.2 sekali, jatuh, mendarat.
- `coins_score_and_win`: 2 koin sensor → destroy + skor + SFX + "WIN" + judul.
- `enemy_hit_loses_life_and_respawns`: sentuh musuh → nyawa 3→2 + respawn.
- `enemy_patrols_between_bounds`: musuh bolak-balik 7–9 via global + `transform.x`.
- Kamera: `transform.set(entity.x("Player"), entity.y("Player"))` tiap tick.

## Acceptance checklist yang terpengaruh

Butir v0.2 terkait (tetap kosong sampai M22): bahasa tanpa fitur di luar game
contoh — terbukti (nol penambahan).

## Belum diverifikasi

- Level 3-koin + SFX/musik nyata (M21, butuh M18/M19).
- Lompat teleport vs impuls velocity: teleport cukup untuk Coin Dash; bila M21
  butuh fisika lompat lebih halus, usulkan `velocity.set` terpisah.

## Masalah dan risiko yang diketahui

- Bug dispatch collision (output non-log dibuang) sudah diperbaiki sesi ini.
- Tes skenario memakai 2 koin (asli 3) agar tick hemat; mekanik identik.

## Asumsi yang dibuat

- Skenario headless mewakili kebutuhan mekanik M21 (tanpa audio/animasi nyata).

## Rekomendasi langkah berikutnya

1. M18 (animasi sprite): komponen `SpriteAnim` + konsumsi antre `anim.play`.
2. M19 (audio): crate `ember-audio` + konsumsi antre audio.
