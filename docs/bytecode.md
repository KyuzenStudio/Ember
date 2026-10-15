# Ember Bytecode Format v1

Status: format biner Milestone 6. Header bawa magic dan versi.
Semua integer little-endian. String = `len u32` + byte UTF-8.

## Layout

```
magic:        4 byte "EMBC"
version:      u16 (= 1)
source_file:  string
strings:      u32 count, lalu string[]
globals:      u32 count, lalu {ty u8, name string}[]
functions:    u32 count, lalu function[]
function:
  name:       string
  params:     u16 (jumlah parameter; slot 0..params)
  locals:     u32 count, lalu ty u8[] (parameter dulu, lalu lokal urut deklarasi)
  code:       u32 len, lalu byte[]
  spans:      u32 count, lalu {offset u32, line u32, col u32}[]
```

`ty u8`: 0=i32, 1=f32, 2=bool, 3=string, 4=void.

## Aturan

- Decode menolak magic salah, versi asing, data terpotong, dan byte sisa.
- Fungsi `$init` (indeks 0) evaluasi inisialisasi global urut deklarasi, lalu `return`.
- Global pra-isi default tipe (0, 0.0, false, "") sebelum `$init` berjalan (ditegakkan Milestone 7).
- `spans[].offset` harus tepat di awal instruksi.
- Setiap fungsi non-kosong dan diakhiri `return`.
- Verifier wajib jalan sebelum eksekusi sumber mana pun, termasuk output compiler sendiri.
