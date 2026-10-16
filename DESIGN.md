# DESIGN.md — Ember

> Status dokumen: spesifikasi desain, bukan laporan status. Bagian 1–12 arsip v0.1
> (tidak diubah). Bagian 13+ rancangan v0.2. Bentuk akhir boleh berubah berdasarkan
> bukti implementasi, selama perubahan dicatat dan tidak melanggar batas scope di `PRODUCT.md`.

## 1. Stack

| Kebutuhan | Pilihan |
| --- | --- |
| Bahasa implementasi | Rust |
| Build | Cargo workspace |
| Rendering GPU | wgpu |
| Window, event loop, input | winit |
| UI editor | egui |
| Collision/physics (kandidat) | Rapier2D |
| Format project dan scene | RON |
| CLI | binary `ember` |
| Version control | Git |
| Target awal | Windows x86-64 |

Sebelum dependency dipakai, verifikasi: versi terbaru yang kompatibel satu sama lain (terutama wgpu, winit, egui), dukungan Windows, lisensi, dan status pemeliharaan. Hasil verifikasi dicatat di laporan audit/milestone.

## 2. Struktur workspace

Batas logis yang diinginkan:

```
crates/
├── ember-core/       tipe dasar bersama (ID, math 2D, error umum)
├── ember-platform/   window, event loop, input (winit)
├── ember-renderer/   renderer sprite 2D (wgpu)
├── ember-assets/     loading, caching, validasi path aset
├── ember-scene/      scene, entity, component, save/load RON
├── ember-lang/       lexer, parser, semantic analysis, bytecode compiler
├── ember-vm/         format bytecode, verifier, VM
├── ember-runtime/    game loop, script host, host API
├── ember-editor/     editor egui
├── ember-player/     player standalone
└── ember-cli/        binary `ember`
```

Ini batas logis, bukan kewajiban membuat sebelas crate sekaligus. Mulai dengan sedikit crate dan modul internal; pisahkan saat dependency boundary sudah nyata.

### Aturan dependensi

- `ember-lang` tidak bergantung pada renderer, platform, atau editor.
- `ember-vm` tidak bergantung pada UI editor.
- `ember-player` tidak membawa dependency editor (egui dan sejenisnya).
- Runtime berbicara ke VM melalui interface host API yang jelas.
- Tidak ada circular dependency.
- Tidak ada abstraksi backend hipotetis tanpa kebutuhan nyata.

## 3. EmberScript

### Karakteristik

- Statically typed.
- Sintaks terinspirasi Lua, bukan implementasi kompatibel Lua.
- Compiler dan diagnostic buatan sendiri.
- Target kompilasi pertama: bytecode.
- Akses ke engine hanya lewat host API terbatas dan terdefinisi.
- Dikembangkan bertahap berdasarkan kebutuhan game nyata.

### Tipe

`i32`, `f32`, `bool`, `string`, `void`.

Tidak ada konversi implisit antar tipe di v0.1. Konversi eksplisit ditambah hanya jika game contoh membutuhkannya.

### Fitur v0.1

Deklarasi variabel, literal dan expression, aritmetika dan perbandingan, operator boolean, assignment, `if`/`else`, `while`, deklarasi dan pemanggilan fungsi, return value, lexical scope dan name resolution, static type checking, diagnostic dengan file/baris/kolom, dan lifecycle `on_create()` serta `on_update(dt: f32)`.

### Dilarang di v0.1

Generics, closures, macros, async, reflection, sistem module kompleks, garbage collector, dynamic typing.

### Keputusan grammar (asumsi eksplisit)

Contoh `entity Player { ... }` di brief hanya konseptual. Untuk v0.1 dipilih bentuk yang paling mudah diimplementasikan:

- Satu file `.em` = satu script untuk satu `ScriptComponent`.
- `var` di tingkat file = state instance script.
- Fungsi dengan nama dan signature tertentu = entry point lifecycle.
- Nama dengan titik seperti `input.down` atau `transform.move` adalah **path namespace host API**, diselesaikan saat name resolution ke host function. Ini bukan akses objek/member.
- `Key.Right` dan sejenisnya adalah konstanta bawaan bertipe `i32`.
- Blok `entity` dapat dipertimbangkan setelah parser dan semantic analysis stabil, sebagai perubahan terpisah.

Contoh bentuk v0.1:

```
var speed: f32 = 220.0

fn on_create() {
    log("Player created")
}

fn on_update(dt: f32) {
    if input.down(Key.Right) {
        transform.move(speed * dt, 0.0)
    }
}
```

Grammar formal (EBNF) ditulis saat milestone parser dimulai dan disimpan di dokumentasi bahasa. Grammar harus konsisten dengan implementasi dan diuji.

### Host API minimal

| Fungsi | Tujuan |
| --- | --- |
| `log(string)` | Menulis ke Console |
| `input.down(i32) -> bool` | Cek tombol sedang ditekan |
| `transform.move(f32, f32)` | Menggeser transform entity pemilik script |

Daftar dapat bertambah hanya karena kebutuhan game contoh. Setiap host function memiliki signature bertipe yang terdaftar di satu tempat dan dipakai oleh type checker, compiler, dan VM.

## 4. Pipeline compiler

```
Source (.em)
  → Lexer + Parser        token, AST, source span, syntax error
  → Semantic Analysis     name resolution, static type checking
  → Bytecode Compiler     instruksi, metadata fungsi, source mapping
  → Bytecode Verifier     opcode, index, jump, metadata, perilaku stack
  → Ember VM              eksekusi bytecode terverifikasi
```

Prinsip diagnostic:

- Setiap error membawa file, baris, kolom, dan pesan yang dapat ditindaklanjuti.
- Parser memulihkan diri dari error agar beberapa error dapat dilaporkan sekaligus bila praktis; ini tidak boleh membuat parser rumit di awal.
- Compiler tidak panic karena input pengguna.

## 5. Bytecode dan VM

### VM

- Stack-based.
- Instruksi sederhana dan terdokumentasi (satu dokumen daftar opcode: operand, efek stack, error).
- Dukungan: constants, local variables, aritmetika, perbandingan, branch, call, return, host call.
- Source mapping dari instruksi ke posisi script untuk runtime error.
- Instruction budget (atau mekanisme setara) untuk menghentikan infinite loop.
- Error runtime terstruktur (tipe error, lokasi sumber, call stack ringkas). Script yang salah tidak boleh menyebabkan crash proses.

### Verifier

Wajib, walaupun bytecode dibuat compiler sendiri. Bytecode yang rusak ditolak sebelum dieksekusi. Pemeriksaan minimal:

- Opcode valid.
- Index constant, local, function, dan host function dalam batas.
- Target jump valid dan jatuh pada batas instruksi.
- Metadata fungsi konsisten (jumlah parameter, local, return).
- Perilaku stack konsisten (tidak underflow, kedalaman terbatas, konsisten di titik gabung).
- Tipe operand sesuai jika format bytecode membawa informasi tipe; jika tidak, desain format harus menjamin lewat compiler dan verifier.

### Format bytecode

Memiliki header dengan magic number dan versi format. Detail biner ditetapkan saat milestone bytecode dan didokumentasikan sebelum VM diimplementasikan.

### Native compilation (di luar v0.1)

Tidak ada LLVM, Cranelift, JIT, atau transpiler C/C++ di v0.1. Evaluasi hanya setelah stability gate di `AGENT.md` terpenuhi dan ada keputusan terpisah yang tercatat.

## 6. Runtime

### Sistem awal

Scene management, entity/component sederhana, `Transform2D`, sprite renderer, `Camera2D`, script component, input keyboard, delta time, asset loading dan caching, logging dan runtime diagnostics, collision/physics dasar bila layak.

### Komponen

| Komponen | Kegunaan |
| --- | --- |
| `Transform2D` | Posisi, rotasi, skala |
| `SpriteRenderer` | Tekstur, tint, sorting order |
| `Camera2D` | Kamera scene |
| `ScriptComponent` | Script dan status instance |
| `Collider2D` | Bentuk collision (opsional v0.1) |
| `RigidBody2D` | Integrasi physics (opsional v0.1) |

Tidak perlu ECS generik yang kompleks. Struktur entity/component sederhana dengan data yang jelas sudah cukup selama bisa berkembang.

### Urutan frame

1. Proses event dan input.
2. Hitung delta time.
3. Update physics dengan fixed timestep (jika aktif).
4. Jalankan script update.
5. Proses perubahan scene yang diantrikan.
6. Siapkan dan render scene.
7. Present frame dan perbarui diagnostics.

Urutan ini identik di Play mode editor dan packaged player. Perubahan scene dari script (create/destroy entity dan sejenisnya) diantrikan dan diterapkan pada langkah 5, bukan di tengah iterasi.

### Play/Stop

Play membuat salinan scene sebagai scene runtime. Stop membuang scene runtime dan scene edit-time tetap tidak berubah. Ini wajib diuji.

## 7. Renderer 2D

Berbasis wgpu, khusus game 2D.

Prioritas: window dan surface management, sprite rendering, texture loading, alpha blending, camera transform, sorting layer/order, resize handling, texture caching, debug drawing (transform dan collider), statistik frame time, draw count, dan script update time.

Sprite batching dilakukan ketika implementasi membutuhkannya. Tidak ada render graph atau abstraksi multi-backend.

## 8. Visual editor

Dibangun dengan egui, bertahap di sekitar vertical slice. Referensi editor lain hanya inspirasi susunan workflow, bukan untuk disalin tampilannya.

### Panel

| Panel | Fungsi |
| --- | --- |
| Viewport | Melihat dan memilih objek di scene |
| Hierarchy | Daftar entity; create, rename, delete, select |
| Inspector | Edit transform dan komponen yang didukung |
| Asset Browser | Menelusuri dan mengimpor aset |
| Script/Diagnostics | Menautkan script, membuka editor eksternal, menampilkan error compiler |
| Console | Log dan runtime error |
| Toolbar | Save, Play, Stop |

### Interaksi minimal

Membuat scene dan entity, memilih entity, mengubah posisi/rotasi/skala, menambah sprite, mengimpor gambar, menautkan script, menyimpan dan membuka ulang project, Play/Stop, melihat diagnostic tanpa crash.

### Di luar scope

Text editor kustom, node editor, framework docking kompleks, visual scripting.

### Aturan editor

- Perubahan edit-time dan runtime harus terpisah.
- Operasi yang mengubah scene sebaiknya melalui satu jalur perintah agar undo/redo dapat ditambahkan nanti. Undo/redo bukan syarat v0.1.
- Error compile atau load ditampilkan di panel, tidak membuat editor keluar.

## 9. Format project dan aset

```
MyGame/
├── ember.project
├── scenes/
│   └── main.ron
├── scripts/
│   └── player.em
├── assets/
│   └── player.png
├── build/
└── README.md
```

`ember.project` menyimpan identitas project, versi format, startup scene, dan konfigurasi dasar.

Prinsip:

- Referensi aset memakai ID stabil atau path relatif yang tervalidasi.
- Scene dan project memiliki versi format.
- Error load aset informatif (path, penyebab).
- Path aset tidak boleh keluar dari root project (tolak `..`, path absolut, dan symlink yang keluar root).
- Output build terpisah dari source aset (`build/`).
- Save/load mempertahankan entity ID.
- Hanya RON di v0.1.

## 10. CLI dan packaging

| Perintah | Perilaku |
| --- | --- |
| `ember new MyGame` | Membuat project starter yang valid |
| `ember check` | Memeriksa syntax, nama, dan tipe tanpa membuka window |
| `ember run` | Menjalankan project lewat runtime |
| `ember build` | Memvalidasi dan menyiapkan aset serta bytecode |
| `ember package` | Menghasilkan distribusi Windows x86-64 tanpa editor |

Exit code non-nol saat ada error agar dapat dipakai automation.

Output `package`: player + runtime + aset + bytecode terverifikasi. Player adalah executable native Rust yang menjalankan VM. Dokumentasi dan pesan CLI tidak boleh menyebutnya native-compiled dari EmberScript.

Player minimal dan alur package dibuktikan lebih awal (lihat milestone di `AGENT.md`).

## 11. Testing

### Unit test

Lexer dan source span; parser, precedence, input invalid; type checker dan identifier tak dikenal; bytecode generation; verifier menolak opcode/index/jump/metadata invalid; VM (aritmetika, branch, call, return, host call, runtime error, instruction budget); scene save/load dan ID stabil; validasi path aset.

### Integration test

Script menggerakkan entity; error syntax/type menunjukkan lokasi; bytecode rusak ditolak sebelum eksekusi; `ember new` menghasilkan project yang lolos `ember check`; scene disimpan dan dibuka ulang; Play/Stop tidak mengubah scene edit-time; packaging menghasilkan player dan file yang diperlukan.

### Aturan

- Tes yang butuh GPU/window dipisahkan dari tes headless agar CI headless tetap berguna.
- Jangan mengklaim tes Windows berhasil jika belum dijalankan di Windows.
- Verifier dan parser sebaiknya diuji dengan input acak/malformed (fuzz atau property test dapat ditambahkan setelah dasar stabil).

## 12. Dokumentasi yang dibutuhkan v0.1

Setup, syntax EmberScript, daftar opcode, host API, workflow editor, format project/scene, dan keterbatasan yang diketahui.

---

# Rancangan v0.2 — "membuat game sungguhan"

## 13. Game contoh: Coin Dash (penggerak semua fitur v0.2)

Platformer satu level. Player lari/lompat; 3 koin (disentuh → skor+1, koin hilang);
1 musuh patroli (disentuh → nyawa-1, player kembali ke start); skor tampil via log;
SFX lompat/koin/kena + musik loop; animasi jalan 2 frame; kamera mengikuti player;
menang saat semua koin diambil. Setiap fitur v0.2 di bawah merujuk kebutuhan ini;
fitur tanpa rujukan tidak masuk.

## 14. Host API v2

Signature bertipe, didaftarkan di satu tempat seperti v0.1 (`ember-core/src/host.rs`,
dipakai checker, compiler, VM). Hanya penambahan berikut, tiap baris terjustifikasi
game contoh:

| Path | Parameter | Return | Kebutuhan game contoh | Status |
| --- | --- | --- | --- | --- |
| `input.pressed` | `i32` | `bool` | lompat tepat sekali per tekan (edge, `down` tak cukup) | tetap |
| `transform.x` | — | `f32` | batas patroli musuh, klem arena player | tetap |
| `transform.y` | — | `f32` | deteksi jatuh ke jurang (batas bawah arena) | tetap |
| `transform.set` | `f32, f32` | `void` | respawn player ke start | tetap |
| `entity.x` | `string` (nama) | `f32` | kamera mengikuti player | tetap |
| `entity.y` | `string` (nama) | `f32` | kamera mengikuti player (vertikal) | tetap |
| `entity.destroy` | `string` (nama) | `void` | koin hilang; diterapkan terantre (langkah 5 urutan frame) | tetap |
| `f32.of_i` | `i32` | `f32` | tidak ada use case jelas di Coin Dash | **ditunda** |
| `i32.of_f` | `f32` | `i32` | tidak ada use case jelas di Coin Dash | **ditunda** |
| `str.of_i` | `i32` | `string` | tampilkan skor via `log`/judul | tetap |
| `str.of_f` | `f32` | `string` | tidak ada use case jelas di Coin Dash | **ditunda** |
| `str.cat` | `string, string` | `string` | susun `"skor: " + angka` | tetap |
| `audio.play` | `string` (path aset) | `void` | SFX lompat/koin/kena | tetap |
| `audio.loop` | `string` (path aset) | `void` | musik latar | tetap |
| `audio.stop` | — | `void` | hentikan musik saat menang/restart | tetap |
| `anim.play` | `string` (nama klip) | `void` | ganti klip jalan/diam dari script | tetap |
| `game.restart` | — | `void` | mulai ulang scene setelah menang/kalah (kontrak §21) | **tambah (usulan)** |
| `game.set_title` | `string` | `void` | tampilkan skor/menang di judul window (opsi B §21, menunggu keputusan) | **tambah (usulan)** |

Lifecycle baru: `on_collision_begin(other: string)` dan `on_collision_end(other: string)`,
`other` = nama entity lawan. Filter (`other == "Coin"`) memakai `==` string yang
sudah ada di v0.1.

## 15. Bahasa v2 (delta dari v0.1)

Kesimpulan audit: **tidak ada perubahan grammar** yang dibutuhkan game contoh.
Array, `for`, `break`, string multi-baris, dan interpolasi string semuanya
DITOLAK dengan alasan: entity game contoh berjumlah tetap di scene (tanpa koleksi),
loop `while` + kondisi cukup untuk timer/patroli, dan penyusunan string dicover
`str.cat`. Jika M21 membuktikan satu kebutuhan tak terpenuhi tanpa fitur bahasa,
fitur itu diusulkan terpisah dengan justifikasi dan tes, bukan paket.

Aturan versi tetap: perubahan grammar atau format bytecode menaikkan versi format
dan wajib punya tes kompatibilitas/penolakan versi lama.

## 16. Event collision

Rapier2D sudah dipakai (M13). Desain: collider dinamis + sensor memakai
`ActiveEvents::CONTACT_EVENTS`; runtime menguras contact events tiap tick setelah
step, memetakan handle → nama entity, mengantrekan pasangan begin/end terurut
dan terdedup, lalu memanggil lifecycle hanya pada script yang mendefinisikannya.
Koin memakai collider sensor (tak mendorong player) → butuh flag `sensor` pada
`Collider2D` (tambahan field opsional scene; aturan versi format berlaku).

## 17. Animasi sprite

Data-driven, tanpa array di script. Komponen scene baru `SpriteAnim`: daftar klip
bernama, tiap klip = daftar path frame + fps + flag loop. Renderer memilih frame
dari waktu sim; `anim.play` mengganti klip aktif. Klip default = sprite statis
sekarang (kompatibel mundur). Preview klip di Inspector diputuskan di M20.

## 18. Audio dasar

Kebutuhan: SFX satu-tembak + satu musik loop, WAV/Vorbis, jalan di Play editor
dan di package tanpa source. Kandidat ( diverifikasi 2026-10-16 via crates.io):

| Kandidat | Versi | Lisensi | Windows | Putusan |
| --- | --- | --- | --- | --- |
| rodio | 0.23.0 (rilis 2026-10-11) | MIT OR Apache-2.0 | ya (pemutaran via cpal/WASAPI) | **dipilih**: API putar/loop paling sederhana, decoder symphonia per fitur |
| kira | 0.12.5 | MIT (tercatat; verifikasi ulang saat pin) | ya (via cpal) | cadangan: mixer game-focused, API lebih besar dari kebutuhan |
| cpal saja | 0.18.2 | Apache-2.0 (verifikasi ulang saat pin) | ya (WASAPI) | ditolak: terlalu rendah (mixing + decode tulis sendiri) |

Desain: crate baru `ember-audio` (batas dependensi nyata: rodio terisolasi agar tes
headless tak butuh device audio; target tanpa audio = sink no-op). Hanya format
WAV dan OGG (keputusan: MP3 **tidak ikut** v0.2). Aset `assets/*.wav|*.ogg`
divalidasi seperti PNG; decode saat load; fitur rodio dibatasi `wav` + `vorbis`.
`ember package` menyalin aset audio.

Verifikasi dependensi (sumber: crates.io API, 2026-10-16):

| Crate | Versi terbaru | Lisensi tercatat | Windows | Status |
| --- | --- | --- | --- | --- |
| rodio | 0.23.0 (rilis 2026-10-11) | MIT OR Apache-2.0 | ya, pemutaran via cpal/WASAPI | kandidat dipilih; pin + uji kompatibilitas di M19 |
| kira | 0.12.5 | MIT (tercatat; verifikasi ulang saat pin) | ya, via cpal | cadangan |
| cpal | 0.18.2 | Apache-2.0 (verifikasi ulang saat pin) | ya, WASAPI | ditolak langsung (terlalu rendah) |

Belum dicek: kompilasi bersama toolchain repo (rustc 1.99.0, edition 2024),
ukuran dependency tree, perilaku tanpa device audio di headless/CI. Itu syarat M19.

## 19. Editor v2

- Geser entity di viewport: seret sprite terpilih; konversi layar→dunia yang sudah
  ada dipakai; saat Play berlaku sementara seperti edit Inspector.
- Undo/redo: operasi scene lewat satu jalur perintah (sudah disiapkan aturannya di
  bagian 8); riwayat per sesi edit, bukan lintas Stop/Play.
- Multi-select: klik-tambah; geser/ubah transform berlaku ke semua terpilih;
  Inspector menampilkan nilai bersama atau campuran.

## 20. Asumsi v0.2 dan pertanyaan terbuka

Asumsi: satu OutputStream audio per proses cukup; nama entity unik per scene
(dipakai `entity.*` dan `other`); koin/musuh berjumlah tetap di scene; musik satu
trek loop; format scene boleh tambah field opsional dengan bump versi + tes kompat.

Keputusan pemilik (2026-10-16): MP3 tidak ikut v0.2; fuzz = korpus dalam-repo +
generator deterministik seed tetap di stable (bukan cargo-fuzz); performa = harness
kecil dalam-repo (`Instant`), baseline ditulis ke file di repo, metrik minimal
frame time, waktu update VM, draw count.

Pertanyaan terbuka: preview animasi di Inspector (M20); harness fuzz rinci dan
lokasi korpus (M15); format file baseline (M15).

## 21. Tampilan skor dan kontrak menang/kalah (minimum, menunggu keputusan display)

Jalur yang ada: `log` → Console editor + stdout player. HUD debug tidak dihitung.
Skor via log saja tidak terlihat pemain di game paket (console bukan tampilan game).

Opsi:

- A. Log saja. Dampak nol (tanpa API, tanpa milestone). Ditolak untuk skor/menang
  karena tak terlihat di game paket.
- B. Judul window (`game.set_title`, usulan §14). Dampak kecil: +1 host fn (M16),
  plumbing judul di player + Play editor (winit `set_title` sudah tersedia, tanpa
  dep baru). Skor, nyawa, dan "MENANG"/"KALAH" tampil di judul.
- C. Sprite digit (entity angka 0–9 diganti dari script). Butuh API ganti tekstur
  sprite + belasan entity; rapuh. Cadangan bila B ditolak.
- D. Renderer teks bitmap. Cakupan besar (font, glyph cache) → milestone sendiri.
  Ditolak untuk v0.2.

Usulan (keputusan pemilik 2026-10-16): opsi B ditetapkan sebagai jalur minimum
untuk M16–M20 (`game.set_title` masuk Host API v2). Untuk M21, HUD final
direncanakan lewat sprite digit dan sprite pesan memakai `SpriteAnim`
(frame = digit/mesan), tanpa library teks baru. Larangan library teks tetap.

Kontrak minimum menang/kalah (berlaku apa pun opsi display): state dipegang script
(global `var`: skor, nyawa, flag selesai); saat menang/kalah script memanggil
`audio.stop` + menampilkan pesan lewat jalur display terpilih; input restart
(`input.pressed(Key.Space)`) memanggil `game.restart` yang membangun ulang runtime
dari scene + chunk yang sama (tanpa baca ulang disk bila sudah di memori).
Detail visual menunggu M21.

Pertanyaan terbuka (aset): `assets/sprite/` berisi `cat/cat_walk.png`,
`knight/_Run.png`, `nature/1.png…5.png` — sumber dan lisensinya belum diketahui.
Jangan dipakai di game contoh sebelum jelas; jangan disentuh tahap ini.

Pertanyaan terbuka (HUD M21): apakah `SpriteAnim` cukup untuk angka multi-digit
(mis. skor 12 = dua entity digit + logika susun, atau satu klip per nilai)?
Diputuskan saat M21; tanpa library teks apa pun.
