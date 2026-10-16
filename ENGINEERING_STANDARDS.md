# Engineering Standards — Ember Engine

Aturan harian untuk kontributor dan agen. Ringkas, actionable, enforceable.
Pelanggaran soft = review trigger, bukan auto-fail, kecuali dinyatakan hard.

## 1. File size (hand-written Rust, hitung code lines tanpa blank/comment)

| Kategori | Target | Review threshold |
|---|---:|---:|
| Util/helper kecil | 30–120 | 200 |
| Modul implementasi | 80–250 | 400 |
| Subsistem kompleks | 150–350 | 500 |
| Public API/interface | 30–200 | 300 |
| Unit-test module | 50–300 | 500 |
| Integration test | 50–400 | 600 |

- Lewat threshold = wajib nilai: satu tanggung jawab kohesif atau pecah.
- File kohesif boleh lewat dengan alasan teknis di `EXCEPTIONS.md`
  (bagian 8) — jangan pecah artifisial.
- Generated/fixture/data tabel dikecualikan (tandai `generated` di checker config).
- Jangan kembangkan/kempiskan file artifisial demi angka.

## 2. Function size (body, tanpa nested fn)

- Target 10–40 LOC. Review > 60. Strong review > 100.
- > 60: cari operasi logis berbeda, pecah di batas control-flow.
- > 100: wajib justificasi di `EXCEPTIONS.md` atau refactor.
- Larangan: wrapper 1-baris, rantai helper tak bermakna, indireksi demi angka.

## 3. Complexity

Review bila ada: nesting > 3, cabang berulang, sekuens operasi tak
berhubungan, match raksasa multi-tanggung-jawab, mutable state luas,
konversi error berulang, kopling antar-subsistem tinggi.
Obat: early return, helper bermakna, tipe jelas, alur lurus.
Tanpa angka CC wajib global; ukur LOC + nesting + review arsitektur.

## 4. Naming (Rust standar, hard)

- Crate/mod/fn/var: `snake_case`. Tipe/trait/enum/variant: `UpperCamelCase`.
- Konstanta/statik: `SCREAMING_SNAKE_CASE`. Boolean: `is_/has_/should_`.
- Larangan nama generik (`Manager`, `Handler`, `Utils`, `Common`, `Helper`)
  bila ada istilah domain presisi.
- Jangan rename public API, crate, id serialisasi, simbol bahasa script,
  tanpa cek kompat + update caller + docs. (hard)

## 5. Folder dan module

- `lib.rs`: deklarasi mod + public API intencional + re-export. Bukan dump implementasi.
- `main.rs`: bootstrap + delegasi ke modul app. Logika > ~50 baris pindah ke `src/app.rs`/`src/commands.rs` (binary-internal, bukan lib artifisial).
- Private default. `pub(crate)` untuk dalam-crate. `pub` hanya API intencional.
- `mod.rs` hanya bila ada manfaat namespace nyata; prefer `mod.rs`-less (`foo.rs` + `foo/`).
- Test unit dekat implementasi; integrasi di `tests/`; contoh di `examples/`.
- Larangan direktori `misc/`, `common/`, `utils/`.
- Crate `ember-*` dipertahankan; crate baru hanya untuk subsistem/batas
  dependensi bermakna. Jangan gabung crate independen demi sedikit manifest.

## 6. Kualitas kode Rust

- Ownership eksplisit, lifetime seperlunya, tipe kecil kohesif.
- `Result` untuk recoverable; `unwrap`/`expect` hanya dengan invarian
  beralasan di jalur produksi. (hard-ish: clippy `unwrap_used` dianjurkan,
  belum hard)
- Match exhaustive bila tepat. `unsafe` minimal + invarian safety terdokumentasi
  per block. (hard)
- Tanpa `allow` luas, clone/allokasi sia-sia, global state raksasa,
  framework generik overengineered, re-export wildcard yang bocorkan internal.
- Iterator idiomatis bila lebih terbaca. Stdlib dulu sebelum dependensi baru.
- Jangan tambah dependensi untuk gantikan implementasi stdlib sepele. (hard)

## 7. Batas API dan dependensi (hard)

Arah yang diizinkan (panah = boleh depend on):

```text
editor, player, cli  ->  runtime, project, renderer, audio, lang, vm, core
runtime              ->  lang, vm, scene, core (+ rapier2d)
project              ->  lang, scene, vm
renderer             ->  scene, core
lang                 ->  vm, core
scene, vm            ->  core
core, audio          ->  (leaf; hanya crates.io)
```

- Fondasi (core/vm/lang/scene) dilarang depend ke executable/editor.
- Tanpa cyclic deps. Pindah kode antar-crate hanya dengan alasan ownership jelas.
- Interface antar-crate eksplisit dan stabil bila praktis.
- Kompatibilitas dibekukan: format EMBC (`MAGIC`/`VERSION`/urutan field/`Ty`
  codes/opcode bytes), format scene v1–v5 + migrasi, format project v1,
  urutan `HOST_FUNCTIONS` (index dipakai verifier+VM), perilaku CLI
  (exit code + teks), startup editor/player. Perubahan butuh versi +
  tes kompat/penolakan.

## 8. Pengecualian tercatat (reviewed, tetap berlaku)

| Lokasi | Fakta | Alasan |
|---|---|---|
| `editor/src/ui.rs` (~600) | panel toolbar/hierarchy/viewport + root `eframe::App` | satu composition root UI; pecah lagi = scatter layout |
| `editor/src/inspector.rs` (~490) | 8 section fn, tiap < 110 | kohesif satu panel; terbesar sudah dipecah |
| `vm/src/vm.rs` (~1350, incl test) | `run` 234, `host_call` 189 | dispatch loop + tabel bridge 1:1 `HOST_FUNCTIONS`; split naikkan borrow-coupling |
| `runtime/src/runtime.rs` (~970, incl test 555) | `start` 78, `tick` 63 | orkestrasi inti; lifecycle test kohesif |
| `lang/src/{compile,semantic,parser}.rs` (~640–710) | fase compiler satu-pass + test | satu tanggung jawab per file |
| `scene/src/lib.rs` (~500) | data + serde + io tipis | split picu cycle model/io |
| `renderer/src/{backend,frame}.rs` (~410–430) | init GPU sekuensial / frame lifecycle | pecah = abstraksi artifisial |
| `vm/src/verifier.rs` typing ~190 | abstract-interpret satu worklist | sudah dipisah struktur vs tipe |
| Fungsi > 100 yang dipertahankan | Fakta | Alasan |
| `editor/ui.rs::inspector` 107, `ui` 143, `viewport` 135 | komposisi sekuensial panel | root layout; section sudah keluar ke `inspector.rs` |
| `lang/lexer.rs::lex` 105 | satu loop char | pecah = state machine artifisial |
| `renderer/backend.rs::new` 124 | init GPU sekuensial | pecah = abstraksi palsu atas 1 urutan |
| `vm/vm.rs::run` 234, `host_call` 189 | dispatch loop + bridge 1:1 tabel | split naikkan borrow-coupling / indireksi |

## 9. Enforcement

- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`: hard.
- `python3 scripts/check_source_limits.py`: lapor pelanggar threshold;
  exit nonzero hanya untuk modul baru tanpa entri pengecualian.
- Ukur dengan code-lines (tanpa blank/comment); fungsi > 60 wajib disebut
  di review, > 100 wajib di tabel §8 atau dipecah.
