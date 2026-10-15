# AGENT.md — Ember

> Panduan kerja untuk agent/developer yang mengimplementasikan Ember. Baca `PRODUCT.md` (apa dan mengapa) dan `DESIGN.md` (bagaimana) lebih dulu. Dokumen ini mengatur *cara bekerja*.
> Ketiga dokumen adalah spesifikasi awal. Jika ada konflik, hentikan pekerjaan terkait, catat konfliknya, dan minta keputusan. Jangan memilih diam-diam.

## 1. Aturan utama

1. **Audit sebelum mengubah kode.** Jangan menulis atau mengubah kode sebelum audit repository selesai dan dilaporkan.
2. **Jangan mengarang status.** Jangan menyatakan fitur selesai, tes lulus, atau build berhasil tanpa menjalankannya dan mencatat hasilnya.
3. **Milestone kecil.** Satu milestone = satu vertical slice yang dapat dijalankan dan diuji.
4. **Sederhana dulu.** Jika ada keputusan yang belum jelas, tulis asumsi eksplisit dan pilih opsi paling sederhana yang mendukung workflow v0.1.
5. **Native compilation dilarang** sampai stability gate (bagian 6) terpenuhi dan keputusan terpisah dicatat.
6. **Patuhi non-goals** di `PRODUCT.md`. Jangan menyelipkan fitur di luar scope.
7. **Jujur soal platform.** Jangan mengklaim tes Windows berhasil jika tidak benar-benar dijalankan di Windows. Laporkan platform tempat tes dijalankan.

## 2. Prosedur audit (wajib sebelum perubahan kode)

Lakukan dan laporkan:

1. **Inventaris repository:** struktur direktori, `Cargo.toml` workspace, daftar crate, file konfigurasi, CI jika ada.
2. **Status git:** branch, commit terakhir, perubahan yang belum di-commit. Jangan menimpa pekerjaan yang ada.
3. **Toolchain:** versi `rustc`/`cargo`, target yang terpasang, sistem operasi tempat audit dijalankan.
4. **Build dan tes dasar:** `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check` bila relevan. Catat hasil apa adanya, termasuk kegagalan.
5. **Dependency:** daftar dependency langsung, versi, lisensi, status pemeliharaan, kompatibilitas Windows. Tandai yang belum diverifikasi.
6. **Kesenjangan terhadap dokumen:** bandingkan kondisi repository dengan `PRODUCT.md` dan `DESIGN.md`. Pisahkan: sudah sesuai, sebagian, belum ada, bertentangan.
7. **Risiko dan pertanyaan terbuka.**
8. **Rencana milestone berikutnya** beserta asumsi.

Jika repository kosong, tulis "repository kosong" dan lanjutkan ke Milestone 0. Jangan mengisi laporan dengan hal yang tidak diperiksa.

## 3. Urutan implementasi

Setiap milestone memiliki: tujuan, hasil yang dapat didemonstrasikan, tes, dan verification report (bagian 5). Jangan mulai milestone berikutnya sebelum yang sebelumnya terverifikasi, kecuali ada alasan tertulis.

| # | Milestone | Hasil yang dapat didemonstrasikan |
| --- | --- | --- |
| 0 | Fondasi repo | Cargo workspace, CI/skrip tes lokal, `ember --version`, aturan format dan lint |
| 1 | Window dan sprite | Window winit + wgpu menampilkan satu sprite dari file PNG; resize ditangani |
| 2 | Scene model | Entity + `Transform2D` + `SpriteRenderer` + `Camera2D`; save/load RON; entity ID stabil; tes round-trip |
| 3 | Project format dan CLI dasar | `ember new` menghasilkan project valid; validasi path aset; versi format |
| 4 | Lexer dan parser | Token, AST, source span, syntax error dengan lokasi; tes valid dan invalid |
| 5 | Semantic analysis | Name resolution, type checking, diagnostic tipe; `ember check` bekerja headless |
| 6 | Bytecode dan verifier | Format bytecode, compiler dari AST, verifier yang menolak bytecode malformed |
| 7 | VM | Aritmetika, branch, call, return, host call, instruction budget, error runtime dengan source mapping |
| 8 | Script di runtime | Host API minimal; `on_create`/`on_update`; script menggerakkan entity dari input |
| 9 | Player dan packaging awal | `ember-player` menjalankan sample dari bytecode terverifikasi; `ember package` menghasilkan folder Windows |
| 10 | Editor: kerangka | Viewport, Hierarchy, Inspector, Toolbar; membuka/menyimpan project |
| 11 | Editor: aset dan diagnostic | Asset Browser, impor gambar, Console, panel diagnostic, penautan script |
| 12 | Play/Stop | Scene runtime terpisah; edit-time tidak berubah setelah Stop |
| 13 | Collision dasar (opsional) | Hanya jika milestone 0–12 stabil; bila tidak, catat sebagai ditunda |
| 14 | Hardening | Bug fixing, clean build, dokumentasi, test report, cek acceptance criteria |

Catatan urutan: pembuktian packaging (milestone 9) sengaja dilakukan sebelum editor agar risiko deployment muncul awal. Jika agent menemukan alasan kuat mengubah urutan, catat alasannya.

## 4. Aturan coding

### Umum

- Ikuti `rustfmt` dan perbaiki peringatan `clippy` kecuali ada alasan tertulis.
- Tidak ada `unwrap()`/`expect()` pada jalur yang dipengaruhi input pengguna (script, bytecode, scene, aset, path). Gunakan error bertipe dengan konteks.
- Tidak ada panic akibat script, bytecode, atau file project yang rusak.
- Gunakan `unsafe` hanya jika terbukti perlu; beri komentar `// SAFETY:` yang menjelaskan invarian.
- Hindari dependency baru tanpa alasan. Setiap dependency baru dicatat di laporan dengan lisensi dan alasan.
- Jangan membuat abstraksi, trait, atau generik "untuk nanti". Tambahkan setelah ada dua pemakaian nyata atau kebutuhan batas modul yang jelas.
- Patuhi aturan dependensi crate di `DESIGN.md` bagian 2. Periksa dengan `cargo tree` bila ragu.

### Komentar

- Komentar singkat dan menjelaskan *mengapa*, bukan mengulang apa yang kode lakukan.
- Tidak ada banner comment atau pemisah dekoratif.
- Dokumentasi publik (`///`) hanya untuk API yang memang publik dan non-trivial.

### Error dan diagnostic

- Error compiler membawa file, baris, kolom, dan pesan jelas.
- Error runtime VM membawa lokasi sumber jika source mapping tersedia.
- Error load aset/scene menyebut path dan penyebab.

### Keamanan input

- Validasi path aset terhadap root project (tolak path absolut, `..`, dan keluar-root lewat symlink).
- Verifier dijalankan sebelum eksekusi bytecode dari sumber mana pun, termasuk yang dihasilkan compiler sendiri.
- Batasi sumber daya VM (instruction budget, kedalaman stack/call).

### Git

- Commit kecil dengan pesan jelas.
- Jangan commit artefak build atau aset besar yang tidak perlu.
- Jangan menulis ulang riwayat yang sudah ada tanpa persetujuan.

## 5. Format verification report

Setiap milestone diakhiri dengan laporan ini. Isi hanya hal yang benar-benar dijalankan.

```
# Verification Report — Milestone <nomor>: <nama>

Tanggal: <YYYY-MM-DD>
Commit: <hash>
Platform: <OS, arsitektur, versi rustc>

## Ringkasan
<2–4 kalimat: apa yang diselesaikan>

## Perubahan
- <modul/file utama dan tujuan perubahan>

## Perintah yang dijalankan dan hasilnya
| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build | lulus/gagal | |
| cargo test | X lulus, Y gagal, Z diabaikan | |
| cargo clippy | bersih/N peringatan | |
| cargo fmt --check | lulus/gagal | |

## Demonstrasi
<langkah yang dijalankan dan hasil yang diamati>

## Acceptance checklist yang terpengaruh
- [x] <butir terbukti, sertakan bukti>
- [ ] <butir belum terpenuhi>

## Belum diverifikasi
<hal yang tidak dijalankan, mis. tes di Windows, GPU tertentu>

## Masalah dan risiko yang diketahui
<daftar>

## Asumsi yang dibuat
<daftar>

## Rekomendasi langkah berikutnya
<daftar>
```

Aturan laporan: sertakan keluaran kegagalan apa adanya, jangan menyembunyikan tes yang gagal atau dinonaktifkan, dan tandai jelas jika sesuatu hanya diuji di platform selain Windows.

## 6. Stability gate untuk native compilation

Evaluasi native backend (misalnya Cranelift) boleh dibuka hanya jika **semua** terpenuhi:

1. Semua acceptance criteria v0.1 terbukti.
2. Sintaks dan semantik EmberScript v0.1 stabil dan terdokumentasi.
3. Host API stabil: tidak ada perubahan breaking selama periode yang ditetapkan pemilik proyek, dan dibuktikan oleh minimal satu game contoh non-trivial.
4. Format bytecode dan perilaku verifier terdokumentasi dan teruji, termasuk tes bytecode malformed.
5. Packaged game berjalan lewat VM di Windows x86-64, dibuktikan dengan laporan.
6. Ada suite tes konformansi (script → hasil yang diharapkan) yang dapat dipakai membandingkan VM dan backend baru.
7. Pemilik proyek membuat keputusan tertulis terpisah yang menetapkan scope, target, dan kriteria berhenti.

Sampai saat itu: tidak ada LLVM, Cranelift, JIT, atau transpiler C/C++ di repository, termasuk sebagai prototipe di branch utama.

## 7. Acceptance checklist v0.1

Centang hanya dengan bukti (perintah, keluaran, atau demonstrasi di verification report).

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

## 8. Larangan ringkas

- Mengubah kode sebelum audit.
- Mengklaim hasil yang tidak dijalankan.
- Menambah fitur di luar scope v0.1 atau fitur bahasa terlarang (generics, closures, macros, async, reflection, module kompleks, GC, dynamic typing).
- Native compilation, JIT, LLVM, Cranelift, atau transpiler C/C++ sebelum stability gate dan keputusan terpisah.
- Membuat renderer abstraction multi-backend, custom UI framework, atau custom physics engine.
- Menyebut output `ember package` sebagai executable native-compiled dari EmberScript.
- Mengeksekusi bytecode yang belum diverifikasi.
- Membiarkan script, bytecode, scene, atau aset yang rusak menyebabkan crash.

## 9. Saat ragu

1. Periksa `PRODUCT.md` dan `DESIGN.md`.
2. Jika masih belum jelas: tulis asumsi eksplisit, pilih opsi paling sederhana yang mendukung workflow v0.1, dan catat di verification report.
3. Jika asumsi itu mengubah scope, kontrak publik (grammar, bytecode, host API, format file), atau melibatkan native compilation: berhenti dan minta keputusan.
