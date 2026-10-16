# Verification Report — Milestone 21: Game contoh non-trivial (Coin Dash)

Tanggal: 2026-10-16
Commit: (diisi saat commit)
Platform: Windows x86-64, rustc (lihat `rustc --version` di bawah)

## Ringkasan

Coin Dash selesai sebagai game sungguhan di `samples/coin-dash/`: player
lari/lompat, 3 koin sensor, 1 musuh patroli, skor + nyawa + menang/kalah,
SFX + musik loop, animasi jalan ganti klip dari script, kamera ikut player.
Tes headless memainkan dari awal sampai menang; `ember package` menyertakan
audio dan exe paket berjalan tanpa source (satu run bahkan WIN end-to-end).

## Perubahan

- `samples/coin-dash/`: `ember.project`, `scenes/main.ron` (format v5,
  7 entity, `music: Some("assets/music.wav")`), `scripts/player.em`,
  `scripts/cam.em`, `scripts/enemy.em`, 5 PNG 8x8 + 4 WAV (generator
  sekali-pakai di luar repo, tidak di-commit).
- `crates/ember-runtime/tests/coin_dash_sample.rs`: tes playthrough
  (tahan Kanan + lompat ganda terjadwal) sampai `WIN score:3`, kamera ==
  player, sfx koin, loop musik; tes semua texture PNG ter-decode.
- `crates/ember-audio/src/lib.rs`: tes `sample_assets_decode`
  (3 SFX + musik loop ter-decode rodio tanpa device).
- `crates/ember-project/src/lib.rs`: tes `package_sample_includes_audio_without_sources`
  (4 wav + 3 embc di paket, tanpa direktori `scripts/`).
- `crates/ember-player/src/lib.rs`: tes `loads_sample_without_sources`
  (build lalu hapus `scripts/`, 3 script tetap ke-load + musik scene).
- Tanpa perubahan grammar/bytecode/host API (nol fitur bahasa baru).

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| `cargo test --locked --workspace` | 249 lulus, 0 gagal | target dir temporer |
| `cargo clippy --all-targets --locked -- -D warnings` | bersih | |
| `cargo fmt --check` | lulus | |
| `ember check dashcheck` | `ok: 3 script(s) checked` | salinan sampel di temp |
| `ember build dashcheck` | `built 3 script(s)`, cam/enemy/player.embc | |
| `ember package dashpkg` | paket `CoinDash-windows-x86_64`, 9 aset + 3 embc + exe, tanpa `scripts/` | player exe debug |
| exe paket run ±10 detik | hidup, stderr kosong, exit via kill | 3 run, lihat Demonstrasi |

## Demonstrasi

- Headless (`coin_dash_sample`): tahan Kanan, lompat ganda di x>5 dan x>8,
  jalan kembali ke kiri untuk koin 3 → `WIN`, judul `WIN score:3`.
- Exe paket run 1 (10 dtk): `score:1, score:2, lives:2, lives:1`, stderr kosong.
- Exe paket run 2 (8 dtk): `score:1, score:2, score:3, WIN, WIN`, stderr kosong.
- Exe paket run 3 (8 dtk, instrumentasi keyboard sementara): hanya log
  `on_create`, nol event key, player diam — idle stabil, lalu instrumentasi
  di-revert.
- Run 1–2 menerima input hantu dari lingkungan OS (bukan bug engine:
  tanpa event tidak ada gerakan; dengan input, semua reaksi benar).
  Run 2 adalah bukti WIN end-to-end di exe paket tanpa source.

## Acceptance checklist yang terpengaruh

v0.2 (bukti di sini; pencentangan resmi di laporan v0.2):

- Script membaca posisi entity lain + kamera ikut player: ya (`cam.em` +
  `scene_sprites` pakai transform kamera; tes samakan posisi Cam/Player).
- `input.pressed` lompat sekali per tekan: ya (edge; lompat ganda tes
  butuh 2 tekan terpisah).
- Koin hilang + skor (collision): ya (`score:1..3`, entity di-destroy).
- Musuh patroli + nyawa + respawn: ya (`lives:2/1`, `transform.set(0,0.5)`).
- Animasi jalan + ganti klip dari script: ya (`idle`/`walk`, `anim.play`
  hanya saat transisi agar frame tidak me-reset tiap tick).
- SFX + musik loop di Play dan package: ya (antre `Play`/`Loop`/`Stop`;
  musik scene auto-`Loop`; wav paket terbukti ada + ter-decode).
- `ember package` sertakan audio; paket bersuara tanpa source: ya
  (file ada; `load_game` tanpa `scripts/`; exe hidup 10 dtk tanpa stderr).
- Coin Dash awal–menang: ya headless + ya sekali di exe paket.
- Tanpa fitur bahasa di luar kebutuhan: ya (nol perubahan grammar).

## Belum diverifikasi

- Main manual pakai keyboard sungguhan di editor (Play) dan di exe paket
  (lompat terasa, musik/SFX terdengar di speaker, judul WIN terlihat).
- CI Windows hijau (terblokir payment; tetap terbuka).
- Suara aktual di speaker (tidak ada perangkat audio saat uji headless).

## Masalah dan risiko yang diketahui

- Log `WIN` bisa ganda (kontak darat setelah menang memicu
  `on_collision_begin` lagi saat `score == 3`); idempoten, kosmetik.
- Input hantu OS pada run paket (Right/Space sesekali tanpa keyboard);
  engine merespons benar; bukan bug input.
- Aset sampel placeholder (8x8 piksel, nada dasar); ganti saat ada artis.
- `docs/PERF_BASELINE.md` ditulis ulang tiap run tes baseline (by design);
  angka bergerak, di-commit terpisah.

## Asumsi yang dibuat

- Lompat teleport `transform.move(0, 2.2)` + gravitasi = lompat cukup
  (terbukti headless + M17).
- `anim.play` me-reset `t` tiap panggil, jadi script hanya panggil saat
  transisi state (pola `moving`/`played`).

## Rekomendasi langkah berikutnya

1. Playtest manual (editor + exe paket) dengan keyboard dan speaker.
2. M22: acceptance ulang 7b, tutup v0.2 + laporan verifikasi v0.2.
