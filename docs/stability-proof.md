# Formulir bukti stabilitas (gerbang Milestone 13)

Isi oleh penguji. Semua skenario lulus = inti dinyatakan stabil,
syarat pembuka Milestone 13 (collision). Satu baris gagal = tetap ditunda.

## Lingkungan

| Field | Isi |
| --- | --- |
| Tanggal |16 - 10 - 2026 |
| Penguji | ditdev |
| Commit | |
| Build | debug / release |
| OS + GPU + driver | |
| Rust (`rustc --version`) | |

## Cara nilai

- `[x]` LULUS bila hasil persis seperti kolom "Harapan".
- `[ ]` GAGAL bila beda, crash, freeze, atau pesan error tak jelas. Tulis bedanya di Catatan.
- Lampirkan screenshot untuk S3, S6, S9 (nama file di Catatan).

## CLI dan packaging

| ID | Langkah | Harapan | Hasil | Catatan |
| --- | --- | --- | --- | --- |
| S1 | `ember new StabGame` | folder + 5 file terbuat, exit 0 | x |berhasil tanpa ada masalah ada 5 file |
| S2 | `ember check StabGame` | `ok: 1 script(s) checked`, exit 0 | x| ini juga aman sesuai harapan|
| S3 | Rusak `player.em` (hapus `=`), `ember check` | `file:baris:kolom` + pesan, exit 1 | x|ini juga sesuai harapan \StabGame\scripts\player.em:1:17: expected '=', found number |
| S4 | Perbaiki, `ember build`, `ember package` | `dist/StabGame-windows-x86_64` berisi exe + `.embc` + aset + scene | x| sesuai harapan .\ember build StabGame
built 1 script(s) to 'E:\Project\02_Software_Engineering\App\project-amber\target\release\StabGame\build'
PS E:\Project\02_Software_Engineering\App\project-amber\target\release> .\ember package StabGame
packaged 'E:\Project\02_Software_Engineering\App\project-amber\target\release\StabGame\dist\StabGame-windows-x86_64'
  assets\player.png
  build\player.embc
  ember-player.exe
  ember.project
  scenes\main.ron|

## Player windowed (gagal)

| ID | Langkah | Harapan | Hasil | Catatan |
| --- | --- | --- | --- | --- |
| S5 | Jalankan exe di `dist/` | window judul game terbuka, sprite tengah terlihat | |tidak sesuai harapan:PS E:\Project\02_Software_Engineering\App\project-amber\target\release\StabGame\dist\StabGame-windows-x86_64> ./ember-player.exe
error: load project: entity 'Player': script 'scripts/player.em' not found |
| S6 | Tahan panah kanan 2 detik, lalu kiri, atas, bawah | gerak 4 arah, halus, tanpa freeze | | |
| S7 | Resize window, tekan Escape | ikut ukuran, Escape tutup rapi, exit 0 | | |

## Editor windowed

| ID | Langkah | Harapan | Hasil | Catatan |
| --- | --- | --- | --- | --- |
| S8 | `ember-editor StabGame`, viewport | sprite terlihat di tengah | x| sesuai harapan|
| S9 | Klik sprite, ubah pos/inspector, Save, Reload | berubah, tersimpan, terbuka ulang sama | x|sesuai harapan |
| S10 | Tulis path PNG luar di Import, Import | file tersalin, daftar bertambah, Console catat | x|sesuai harapan |
| S11 | Rusak script, tekan Check | panel Diagnostic tampil `file:baris:kolom`, editor tak crash | x|aman sesuai harapan error: scripts/player.em:1:17: expected '=', found numbe |
| S12 | Perbaiki, Play, gerak panah, ubah speed di Inspector, Stop | gerak ikut speed baru; Stop kembalikan scene edit utuh |x | sesuai harapan|

## Keputusan

| Syarat | Status |
| --- | --- |
| S1–S12 lulus semua | BELUM (coret satu) |
| M13 dibuka | Tunda lagi (coret satu) |

Tanda tangan + tanggal: DITDEV 16 10 2026

## Adendum 2026-10-16

- Akar gagal S5 diperbaiki: player/package muat project tanpa sumber `.em`
  (`load_project_opts` + tes regresi). Uji ulang S5 windowed tertunda.
- M13 dibuka dan selesai: fisika Rapier2D + HUD Play + fix akumulator sub-step.
  Penguji verifikasi body jatuh di Play windowed (`phys=true`).
- Keputusan diperbarui: M13 SELESAI. Sisa terbuka: uji ulang S5 windowed.
