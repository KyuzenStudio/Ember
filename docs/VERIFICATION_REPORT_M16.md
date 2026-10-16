# Verification Report — Milestone 16: host API v2

Tanggal: 2026-10-16
Commit: perubahan M16 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Host API v2 terimplementasi sesuai hasil review: 13 fungsi kept + `game.restart`
+ `game.set_title` (15 fungsi); `f32.of_i`, `i32.of_f`, `str.of_f` tetap tak
terdaftar dan ditolak checker. Semua terdaftar di `ember-core/src/host.rs`
(append-only, indeks bytecode lama stabil). Lifecycle `on_collision_begin/end`
dihidupkan lewat contact event Rapier. Scene format naik v1→v2 (field `sensor`,
loader kompatibel v1).

## Perubahan

- `crates/ember-core/src/host.rs`: 15 entri registry + roots
  entity/str/audio/anim/game.
- `crates/ember-vm/src/vm.rs`: trait `Host` + dispatch 15 path (`str.*` murni
  di VM); `TestHost` diperbarui.
- `crates/ember-lang`: `TestHost` compile.rs diperbarui; tes penolakan
  `f32.of_i`/`i32.of_f`/`str.of_f`; tes `unknown_host_fn` diperbaiki
  (`input.pressed` kini valid).
- `crates/ember-scene`: `Collider2D.sensor` + `SCENE_FORMAT_VERSION = 2`;
  loader terima v1/v2, tolak lainnya; tes kompat v1 + round-trip sensor.
- `crates/ember-runtime`: edge `pressed` di `InputState` (`tick` kini
  `&mut InputState`); antre destroy/audio/anim/title/restart; dispatch
  collision; rebuild fisika setelah destroy; tes 10 fungsi + lifecycle + restart.
- `crates/ember-runtime/src/physics.rs`: `ActiveEvents::COLLISION_EVENTS`,
  sensor, peta collider→entity, kolektor `EventHandler`, `take_contacts`.
- `crates/ember-editor`: mapping `key_pressed`→edge; `tick_play` rebuild saat
  restart + terapkan judul (restore "Ember" saat Stop); tes restart Play.
- `crates/ember-player`: terapkan judul + rebuild saat restart.
- `docs/host-api.md`: tabel v2 + lifecycle collision.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 216 lulus, 0 gagal, 0 diabaikan | +14 dari M15 (10 runtime, 1 editor, 1 lang, 2 scene) |
| cargo clippy --all-targets --locked -- -D warnings | bersih | |
| cargo fmt --check | lulus | |

## Demonstrasi

- Tiap fungsi dibuktikan tes script: pressed sekali-per-tekan; baca/tulis
  transform; baca entity lain + error bernama bila tak ada; `str` log skor;
  destroy hapus entity+instance (berlaku antre: script korban masih jalan tick
  berjalan); audio/anim/title tercatat; restart: flag → rebuild → global
  ter-reset + `on_create` jalan lagi + posisi awal.
- Collision: kotak jatuh ke tanah → kedua skrip terima begin dengan nama lawan;
  teleport timer → end diterima. Tes awal gagal karena ambang teleport di atas
  titik kontak (kesalahan tes, bukan engine) — diperbaiki, kini lulus.
- Deferral: `f32.of_i` dkk ditolak checker sebagai unknown host function.

## Acceptance checklist yang terpengaruh

Butir v0.2 terkait (tetap kosong sampai M22, bukti dicatat di sini):
- Script baca entity lain / kamera ikut — terbukti di tingkat API.
- `input.pressed` sekali per tekan — terbukti.
- Koin hilang + skor (event collision) — mekanisme terbukti; game contoh di M21.

## Belum diverifikasi

- Backend audio nyata (M19; antrean M16 belum dikonsumsi).
- Komponen klip animasi (M18; `anim.play` baru mencatat permintaan).
- Restart/title di windowed interaktif (wiring ada, uji manual tertunda).
- Run CI GitHub (terblokir payment; lokal hijau).

## Masalah dan risiko yang diketahui

- `tick(&mut InputState)` mengubah signature publik internal; semua call site
  diperbarui (editor, player, 20+ tes).
- Scene v2: file v1 tetap dibaca; save selalu tulis v2.
- `entity.destroy` memakai kecocokan nama pertama; nama duplikat tak dideteksi.

## Asumsi yang dibuat

- Satu antrean destroy per tick cukup; tidak ada destroy bersarang antar-entity
  dalam satu tick selain berurutan.
- Urutan dispatch collision mengikuti urutan event Rapier (deterministik untuk
  input sama).

## Rekomendasi langkah berikutnya

1. M17 (bahasa v2): verifikasi tidak ada perubahan grammar yang dibutuhkan;
   jika M21 membuktikan sebaliknya, usulkan terpisah.
2. Uji manual windowed: pressed lompat, judul berubah, restart via Space.
