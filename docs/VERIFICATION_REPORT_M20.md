# Verification Report — Milestone 20: editor v2

Tanggal: 2026-10-16
Commit: `ae844be` + perubahan M20 belum di-commit (diserahkan untuk review)
Platform: Windows 11 (NT 10.0.26300.0), x86-64 (AMD64), rustc 1.99.0 / cargo 1.99.0

## Ringkasan

Editor v2 selesai: multi-select (klik + Ctrl+klik di hierarchy/viewport),
geser entity di viewport (klik-seret, berlaku ke semua terpilih, sementara saat
Play), undo/redo snapshot RON (tambah/hapus entity, geser viewport; tombol +
Ctrl+Z/Ctrl+Y/Ctrl+Shift+Z; nonaktif saat Play), dan authoring anim dasar
(+Anim, +/− klip, +/− frame, fps/loop, pilih current).

## Perubahan

- `crates/ember-editor/src/app.rs`: `selection: Vec<EntityId>` + helpers;
  `UndoStack` snapshot (cap 100); `delete_selection`/`move_selection`;
  drag viewport via `screen_to_world` delta; UI authoring anim; 4 tes.

## Perintah yang dijalankan dan hasilnya

| Perintah | Hasil | Catatan |
| --- | --- | --- |
| cargo build --locked | lulus | |
| cargo test --locked | 237 lulus, 0 gagal, 0 diabaikan | terverifikasi silang: 237 fn `#[test]` di tree; +4 tes editor M20 |
| cargo clippy --all-targets --locked -- -D warnings | bersih | |
| cargo fmt --check | lulus | |

## Demonstrasi

- `undo_redo_add_entity`, `move_selection_shifts_all` (+undo),
  `delete_selection_undoable`, `toggle_select_adds_and_removes`: lulus.
- Authoring anim memakai operasi struct biasa + tersimpan via scene round-trip
  (M18); undo-nya tercakup snapshot tambah/hapus.

## Acceptance checklist yang terpengaruh

Butir v0.2 terkait (tetap kosong sampai M22): drag/undo/multi tanpa merusak
scene — mekanisme terbukti headless; uji windowed interaktif tertunda.

## Belum diverifikasi

- Interaksi mouse/keyboard nyata (drag terasa, Ctrl+klik, shortcut).
- Preview animasi di Inspector (gambar frame aktif; belum ada).
- Edit Inspector rinci (nama, tekstur, collider) belum masuk undo.

## Masalah dan risiko yang diketahui

- Undo snapshot seluruh scene per aksi: boros untuk scene besar, cukup untuk v0.2.
- `selection` lama `Option` diganti `Vec`; semua call site diperbarui.
- Karena batas borrow checker, snapshot undo hanya di aksi diskrit + drag
  viewport; edit Inspector rinci disengaja di luar cakupan (dicatat di sini).

## Asumsi yang dibuat

- Ctrl sebagai modifier multi-select/undo mengikuti konvensi Windows.

## Rekomendasi langkah berikutnya

1. M21 (Coin Dash): konten + mainkan + dengar; jawab open Q multi-digit.
2. Uji windowed: drag, Ctrl+klik, Ctrl+Z/Y.
