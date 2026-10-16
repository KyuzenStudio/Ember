# Architecture — Ember Engine (aktual, pasca-refactor)

## Crate dan tanggung jawab

| Crate | Milik | Bukan milik |
|---|---|---|
| `ember-core` | `Vec2`, `Ty`, tabel host (`HOST_FUNCTIONS`, `CONSTS`, `ROOTS`) | logika engine apa pun |
| `ember-lang` | lexer → parser → semantic → compile (`Em` → `Chunk`), error/span, fasad `parse` | eksekusi, verifikasi bytecode |
| `ember-vm` | opcode (`op`), format EMBC + disasm (`format`), verifier struktur+tipe (`verifier`), error (`error`), loop eksekusi + bridge `Host` (`vm`) | bahasa, scene, fisika |
| `ember-scene` | komponen + `Scene` + RON v1–v5 + migrasi | runtime, render |
| `ember-renderer` | PNG (`image`), tipe publik (`types`), device/pipeline (`backend`), quad + bridge scene (`sprite`), frame cache + offscreen (`frame`) | gameplay |
| `ember-runtime` | orkestrasi load/start/tick, fisika (`physics`), input edge (`input`), adapter `Host` + konversi (`host`) | render device, audio device |
| `ember-audio` | gate ekstensi, decode rodio, engine headless-safe + recorded log | gameplay |
| `ember-project` | error/model, validasi path, scaffold, `check`, aset, build `.embc`, package | UI, eksekusi |
| `ember-player` | lib: `load_game`; bin: `app` (winit + frame loop + audio drain), `main` bootstrap | compile, edit |
| `ember-editor` | bin: `state` (state+undo), `play` (lifecycle), `project` (IO/sync), `ui` (panel+viewport), `inspector` (8 section), `app` (boot state + test) | bytecode, fisika detail |
| `ember-cli` | bin: `main` (router), `commands` (new/check/build/package) | semua logika proyek (di `ember-project`) |

Arah dependensi: lihat `ENGINEERING_STANDARDS.md` §7. Siklus: tidak ada.

## Batas modul penting

- Bahasa: `ast` (murni data) → `lexer`/`parser` → `semantic::check` →
  `compile::compile` → `vm::{verify, Vm}`. `ember_lang::host` = re-export
  `ember_core::host` (tanpa shim).
- Runtime per tick: fisika fixed-step → `anim.t` → `on_update` → kontak →
  drain antre (audio/anim/title/restart/destroy). `CtxHost` satu-satunya
  penulis antre dari script.
- Scene adalah data; runtime pegang `Scene`, renderer baca via
  `scene_sprites` + `texture_size`. Tidak ada ownership ganda.
- Project: pure-fs + compile pipeline; CLI/editor panggil, tak ada callback.

## Public API ownership

- `ember-core::host`: kontrak 15 fungsi + tabel (append-only).
- `ember-vm`: `Chunk`/`encode`/`decode`/`verify`/`Vm`/`Host`/`Value`/error.
- `ember-scene`: komponen + `Scene::{save,load}_*` + migrasi versi.
- `ember-project`: `create/load/check/build/package/import/list/validate`.
- `ember-runtime`: `Runtime::{load,load_prebuilt,start,tick,...}` + drain.
- `ember-renderer`: `SceneRenderer`/`Offscreen`/`scene_sprites`/`decode_png`.
- `ember-player::load_game`: bundle `GameDef` untuk bin dan embedder.

## Testing strategy

- Unit dekat implementasi (`semantic`, `verifier` accept/reject, `physics`,
  editor state/play, project round-trip, renderer quad/camera).
- Robustness: `malformed.rs` (lang+vm, korpus + fuzz `EMBER_FUZZ_ITERS`).
- Gameplay: `coin_dash.rs` (mekanik), `coin_dash_sample.rs` (playthrough
  sampel sampai WIN), `perf_baseline.rs` (angka, tulis-ulang by design).
- Aturan: jangan hapus tes gagal — diagnosis. Baseline waktu bukan assertion
  keras lintas-mesin.

## Tambah modul/crate

- Modul baru = satu tanggung jawab + di bawah threshold §1, atau masuk §8.
- Cross-crate move butuh alasan ownership; crate baru butuh batas
  dependensi bermakna. Update kedua dokumen ini + re-export paths.
