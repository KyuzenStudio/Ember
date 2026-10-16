# PRODUCT.md — Ember

> Status dokumen: spesifikasi awal. Dokumen ini bukan laporan status repository. Tidak ada bagian yang boleh dibaca sebagai klaim bahwa sesuatu sudah diimplementasikan.
> Nama "Ember" adalah nama kerja. Branding dan logo ditunda sampai vertical slice berhasil.

## 1. Visi

Ember adalah game engine 2D buatan sendiri dengan Rust, lengkap dengan bahasa scripting baru (EmberScript), compiler, virtual machine, dan visual editor.

Tujuannya bukan sekadar renderer 2D, melainkan ekosistem kecil tempat developer bisa membuat game tanpa menulis seluruh game dalam Rust.

Filosofi: Ember harus terasa seperti alat pengembangan game sungguhan, bukan eksperimen yang hanya berhasil menampilkan sprite.

## 2. Pengguna target

| Aspek | Keputusan |
| --- | --- |
| Pengguna awal | Solo developer |
| Sistem operasi awal | Windows |
| Arsitektur target | x86-64 |
| Workflow utama | Visual editor + scripting |
| Jenis game | 2D |

Pengguna awal juga mencakup pembuat Ember sendiri (dogfooding). Prioritas v0.1 adalah satu developer dapat menyelesaikan satu game kecil dari awal sampai paket Windows.

## 3. Workflow yang dituju

1. Developer membuat project lewat `ember new`.
2. Membuka editor visual 2D.
3. Membuat scene dan menambahkan entity.
4. Mengimpor sprite dan mengatur properti lewat Inspector.
5. Menulis script EmberScript.
6. Menekan Play untuk langsung menguji game.
7. Memperbaiki script berdasarkan diagnostic.
8. Membuild dan mengemas game menjadi aplikasi Windows mandiri.

## 4. Prinsip produk

- **Vertical slice dulu.** Setiap milestone harus menghasilkan sesuatu yang berjalan dan teruji.
- **Editor sejak awal.** Editor dibangun bertahap di sekitar vertical slice, tidak ditunda sampai engine selesai.
- **Bahasa untuk game nyata.** Fitur bahasa ditambah hanya jika dibutuhkan game contoh.
- **Diagnostic yang jelas.** Error harus menyebut file, baris, dan kolom. Script yang salah tidak boleh membuat editor atau runtime crash.
- **Packaging dibuktikan awal.** Risiko deployment tidak boleh menumpuk di akhir.
- **Jujur soal klaim.** Tidak ada klaim "native" atau "berhasil di Windows" tanpa bukti.
- **Scope dijaga.** Lebih baik fitur sedikit yang bekerja daripada banyak yang setengah jadi.

## 5. Scope v0.1

Bagian ini dipertahankan sebagai arsip. v0.1 dinyatakan selesai
(`docs/VERIFICATION_REPORT_V01.md`). Isi di bawah tidak diubah.

### Termasuk

- Cargo workspace dan CLI `ember` (`new`, `check`, `run`, `build`, `package`).
- Window, renderer sprite 2D, kamera 2D, input keyboard, delta time.
- Scene, entity, dan component sederhana; save/load dengan entity ID stabil.
- EmberScript statis: compiler frontend, bytecode, verifier, VM, host API minimal.
- Visual editor: Viewport, Hierarchy, Inspector, Asset Browser, Script/Diagnostics, Console, Toolbar (Save, Play, Stop).
- Play/Stop dengan scene runtime terpisah dari scene edit-time.
- Packaging Windows x86-64: player + runtime + aset + bytecode terverifikasi.
- Collision dan physics dasar hanya jika layak setelah inti stabil (Rapier2D adalah kandidat).
- Sample game, automated tests, dan dokumentasi.

### Tidak termasuk (non-goals v0.1)

- Native compiler, JIT, LLVM, atau Cranelift.
- 3D engine, multiplayer/networking, mobile, console.
- Visual scripting, plugin marketplace.
- Custom physics engine, custom UI framework, custom renderer abstraction untuk banyak backend.
- Kompatibilitas Lua penuh.
- Dynamic typing, generics, closures, macros, async, garbage collector.
- Hot reload yang dijamin production-ready.
- IDE lengkap (custom text editor, node editor, docking framework kompleks).
- Audio (ditunda sampai workflow utama berfungsi).

## 6. Roadmap

| Fase | Nama | Isi |
| --- | --- | --- |
| 1 | Fondasi | Cargo workspace, window, sprite, scene model, save/load, sample project |
| 2 | Bahasa | Lexer, parser, type checker, bytecode, verifier, VM, host API minimal |
| 3 | Editor | Viewport, hierarchy, inspector, asset workflow, diagnostics, Play/Stop |
| 4 | Integrasi | Input, script lifecycle, collision dasar bila layak, CLI, player standalone, integration tests |
| 5 | Hardening | Bug fixing, clean build, dokumentasi, test report, pemeriksaan acceptance criteria |

Estimasi kasar: 12–16 minggu untuk satu developer jika scope dijaga. Ini estimasi perencanaan, bukan janji penyelesaian. Risiko terbesar: integrasi editor, bahasa scripting, dan packaging.

## 7. Success criteria (acceptance criteria v0.1)

Ember v0.1 dianggap layak ketika semua butir berikut terpenuhi dan terbukti:

- [ ] Project baru dapat dibuat lewat CLI.
- [ ] Editor dapat membuka, mengedit, menyimpan, dan membuka ulang scene.
- [ ] Sprite dapat ditampilkan dan properti dasarnya diubah.
- [ ] EmberScript statis dapat dikompilasi ke bytecode.
- [ ] Verifier menolak bytecode malformed.
- [ ] VM menjalankan control flow dan function call yang didukung.
- [ ] Script dapat membaca input dan mengubah transform entity.
- [ ] Syntax/type error menunjukkan lokasi sumber.
- [ ] Play/Stop menggunakan scene runtime terpisah.
- [ ] Sample game dapat dijalankan dari editor.
- [ ] `ember check` bekerja tanpa membuka window.
- [ ] `ember package` menghasilkan game Windows standalone.
- [ ] Automated tests dan clean build memiliki hasil yang tercatat.
- [ ] Dokumentasi menjelaskan setup, syntax, workflow, dan keterbatasan.
- [ ] Tidak ada native compiler yang diselipkan ke v0.1.

## 8. Keputusan jangka panjang: native compilation

Game berbasis EmberScript ingin dibuild menjadi executable native di masa depan. Ini bukan target v0.1. Native backend (misalnya Cranelift) hanya dievaluasi sebagai milestone terpisah setelah bahasa dan host API terbukti stabil lewat game contoh dan packaged game berjalan memakai VM. Syarat rinci ada di `AGENT.md` (stability gate).

Output `ember package` v0.1 adalah player native Rust yang menjalankan bytecode VM. Output ini tidak boleh disebut "executable native hasil kompilasi EmberScript".

## 9. Risiko utama

| Risiko | Mitigasi |
| --- | --- |
| Scope creep pada bahasa | Daftar non-goals; fitur bahasa harus dipicu kebutuhan game contoh |
| Integrasi editor, VM, dan runtime sulit | Vertical slice kecil sejak milestone awal |
| Deployment Windows bermasalah di akhir | Player minimal dan packaging dibuktikan sejak awal |
| Dependency (wgpu, winit, egui, Rapier) berubah atau bermasalah | Verifikasi versi, kompatibilitas, lisensi, dan pemeliharaan sebelum implementasi |
| Klaim tes tidak sesuai kenyataan | Tes Windows hanya dilaporkan jika benar-benar dijalankan di Windows |

## 10. Asumsi eksplisit (arsip v0.1, tidak diubah)

Brief asli menyisakan beberapa hal terbuka. Dokumen ini memilih opsi paling sederhana yang mendukung workflow v0.1:

1. Grammar EmberScript v0.1 berbasis fungsi biasa dengan entry point tetap, bukan blok `entity`. Blok `entity` dapat dievaluasi kembali setelah parser stabil (rincian di `DESIGN.md`).
2. Editor Script/Diagnostics v0.1 menampilkan diagnostic dan dapat membuka editor eksternal; tidak ada text editor kustom.
3. Format scene dan project: RON saja.
4. Collision/physics masuk v0.1 hanya jika milestone inti sebelumnya selesai dan stabil.
5. Audio tidak masuk v0.1.
6. Versi dependency tidak ditetapkan di dokumen ini dan harus diverifikasi saat implementasi.

## 11. Scope v0.2 — "membuat game sungguhan"

Tema: satu developer membuat satu platformer kecil yang lengkap (judul kerja:
Coin Dash) memakai fitur v0.2, dari editor sampai paket Windows. Fitur masuk v0.2
hanya jika game contoh membutuhkannya.

### Termasuk

- Utang v0.1: CI Windows, fuzz/corpus malformed untuk lexer/parser/verifier,
  baseline performa (VM, renderer, fisika, waktu load).
- Host API v2 bertipe (rincian + hasil review `DESIGN.md` §14): baca posisi entity,
  teleport, hancurkan entity (terantre), `input.pressed` (edge), konversi eksplisit
  yang lolos review (`str.of_i`, `str.cat`; `f32.of_i`/`i32.of_f`/`str.of_f` ditunda),
  lifecycle `on_collision_begin/end`, audio play/loop/stop, ganti klip animasi,
  ditambah usulan `game.restart` (kontrak menang/kalah) dan `game.set_title`
  (opsi display, menunggu keputusan).
- Event collision dari Rapier (begin/end, nama entity lawan) untuk koin, musuh,
  dan pijakan.
- Animasi sprite data-driven (rangkaian frame + fps + loop, tanpa array di script).
- Audio dasar: SFX satu-tembak + musik loop (format WAV/Vorbis), sinkron ke
  `ember package`.
- Editor v2: geser entity di viewport, undo/redo, multi-select.
- Game contoh non-trivial (Coin Dash): player lari/lompat, 3 koin, 1 musuh patroli,
  skor, SFX + musik, animasi jalan, kamera ikut, kondisi menang.
- Hardening v0.2: acceptance ulang, docs, verification report.

### Tidak termasuk (non-goals v0.2)

- Semua non-goals v0.1 tetap berlaku (native compiler/JIT/LLVM/Cranelift, 3D,
  multiplayer, mobile, visual scripting, marketplace, GC, dynamic typing,
  generics, closures, macros, async, reflection, modul kompleks).
- UI teks dalam game (skor via judul window/log di M16–M20; HUD sprite digit di M21;
  tanpa library teks).
- Fisika lanjutan: sendi, raycast, CCD selektif, kecuali game contoh membutuhkannya.
- Format audio selain WAV/Vorbis (MP3 opsional, diputuskan di M19).
- Text editor kustom di dalam editor (tetap buka editor eksternal).
- Hot reload production-ready; network; lokalisasi.

## 12. Success criteria (acceptance criteria v0.2)

Semua kosong sampai terbukti di verification report v0.2:

- [ ] CI Windows hijau (build + test + clippy + fmt) tercatat.
- [ ] Fuzz/corpus malformed tidak menjatuhkan lexer/parser/verifier.
- [ ] Baseline performa tercatat (angka, bukan klaim).
- [ ] Script membaca posisi entity lain dan memindahkan kamera mengikuti player.
- [ ] `input.pressed` memicu lompat tepat sekali per tekan.
- [ ] Koin hilang saat disentuh dan skor bertambah (event collision).
- [ ] Musuh patroli mengurangi nyawa dan me-respawn player.
- [ ] Animasi jalan berganti frame sesuai gerak; klip bisa diganti dari script.
- [ ] SFX dan musik loop terdengar di Play editor dan di package.
- [ ] `ember package` menyertakan aset audio; game paket bersuara tanpa source.
- [ ] Drag entity di viewport, undo/redo, multi-select bekerja tanpa merusak scene.
- [ ] Coin Dash selesai dimainkan dari awal sampai menang, dari editor dan dari package.
- [ ] Tidak ada fitur bahasa di luar kebutuhan game contoh; larangan bahasa tetap utuh.
