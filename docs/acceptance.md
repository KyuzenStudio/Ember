# Acceptance v0.1 — status 2026-10-16

Legenda: `[x]` terbukti dengan tes/demo tercatat, `[~]` kode lengkap bukti visual kurang,
`[ ]` belum.

- [x] Project baru dapat dibuat lewat CLI. Tes `dispatch_new_creates_project` + demo `ember new`.
- [~] Editor dapat membuka, mengedit, menyimpan, dan membuka ulang scene.
  Tes round-trip + Save/Reload ada. Sesi editor windowed jalan (dilaporkan user), screenshot
  verifikasi visual belum diarsipkan.
- [~] Sprite dapat ditampilkan dan properti dasarnya diubah. Tes readback piksel GPU lulus.
  Tampilan di layar belum diarsipkan.
- [x] EmberScript statis dapat dikompilasi ke bytecode. Tes `starter_compiles_and_verifies`.
- [x] Verifier menolak bytecode malformed. 15 kasus tes.
- [x] VM menjalankan control flow dan function call yang didukung. Tes while/call nested.
- [x] Script dapat membaca input dan mengubah transform entity. Tes runtime + demo DemoGame.
- [x] Syntax/type error menunjukkan lokasi sumber. Tes `file:baris:kolom` di parser, checker, `ember check`.
- [x] Play/Stop menggunakan scene runtime terpisah. Tes RON identik sebelum/sesudah Stop.
- [ ] Sample game dapat dijalankan dari editor. Kode Play lengkap, demo Play windowed belum diarsipkan.
- [x] `ember check` bekerja tanpa membuka window. Demo `check` valid dan rusak.
- [x] `ember package` menghasilkan game Windows standalone. Demo `dist/` + `.embc` valid.
  Player windowed dari package belum dijalankan sesi interaktif.
- [x] Automated tests dan clean build memiliki hasil yang tercatat. `test-report.md` ini.
- [x] Dokumentasi menjelaskan setup, syntax, workflow, dan keterbatasan. `README.md` + `docs/`.
- [x] Tidak ada native compiler yang diselipkan ke v0.1. `cargo tree` + review: tanpa LLVM/Cranelift/JIT.

Ditunda tertulis: collision/Rapier2D (Milestone 13). Alasan: inti belum dinyatakan stabil
(run windowed dan Play windowed belum diarsipkan), dan tidak ada butir acceptance yang
mewajibkan fisika. Syarat masuk: arsip demo visual M9–M12.
