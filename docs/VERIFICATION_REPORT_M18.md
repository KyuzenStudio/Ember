# Verification Report — Milestone 18: animasi sprite

Tanggal: 2026-10-16
Commit: `14e8434` + perubahan M18 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Animasi sprite data-driven selesai: komponen `SpriteAnim` (klip bernama,
frame, fps, loop), jam animasi maju per tick, renderer menggambar frame aktif,
`anim.play` mengganti klip + reset waktu (klip tak dikenal dicatat di log).
Scene format naik v2→v3 dengan loader kompatibel v1–v3. Antre `anim.play` dari
`on_create`/`on_update`/callback collision kini dikonsumsi runtime.

## Perubahan

- `crates/ember-scene`: `AnimClip`/`SpriteAnim` + `frame_at`/`has_clip`;
  `Entity::frame_texture`/`texture_paths`; `SCENE_FORMAT_VERSION = 3`;
  loader terima 1–3; tes kompat v2 + round-trip + pilih frame.
- `crates/ember-runtime`: jam `anim.t += dt`; antre `anim.play` diterapkan
  (klip ada → ganti + reset; tak ada → log); `start()` tampung output host
  `on_create`/`$init`; tes jam + ganti + klip tak dikenal.
- `crates/ember-renderer`: `scene_sprites` pakai frame aktif; tes draws animasi.
- `crates/ember-runtime/tests/coin_dash.rs`: end-to-end ganti klip → gambar berubah.
- `crates/ember-editor` + `crates/ember-player`: muat frame klip (trainer/sizes)
  lewat `texture_paths`.
- `docs/project-format.md`, `docs/host-api.md`: dokumentasi v3 + anim.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 227 lulus, 0 gagal, 0 diabaikan | +7 dari M17 |
| cargo clippy --all-targets --locked -- -D warnings | bersih | |
| cargo fmt --check | lulus | |

## Demonstrasi

- Script `anim.play("walk")` → frame `w0.png`, setelah 0.2 dtk (8 fps) → `w1.png`.
- Klip sekali-putar berhenti di frame terakhir; klip loop berputar.
- Tanpa komponen anim, gambar statis seperti semula (kompatibel mundur).

## Acceptance checklist yang terpengaruh

Butir v0.2 terkait (tetap kosong sampai M22): animasi berganti frame + ganti
klip dari script — mekanisme terbukti; konten klip game di M21.

## Belum diverifikasi

- Authoring klip di editor (M20); preview Inspector (M20).
- Aset frame nyata (pakai PNG hasil generate di tes).

## Masalah dan risiko yang diketahui

- `t` ikut tersimpan di RON (transien tapi tak berbahaya; lanjut dari posisi itu).
- Nama klip duplikat: yang pertama menang; tak divalidasi.

## Asumsi yang dibuat

- Jam maju selalu (termasuk saat Play dijeda? tidak ada jeda di v0.2).

## Rekomendasi langkah berikutnya

1. M19 (audio): crate `ember-audio` + konsumsi antre audio.
2. M20: authoring + preview klip di Inspector.
