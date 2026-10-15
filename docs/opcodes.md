# Ember Opcode List v1

Status: set instruksi Milestone 6. Nilai opcode stabil; perubahan butuh versi format baru.
Semua integer little-endian. Stack tumbuh ke atas; `[A, B]` = B di puncak.

## Konstanta

| Op | Byte | Operand | Efek stack | Error |
| --- | --- | --- | --- | --- |
| `const.i32` | 0x01 | i32 | `[] -> [i32]` | — |
| `const.f32` | 0x02 | f32 | `[] -> [f32]` | — |
| `const.true` | 0x03 | — | `[] -> [bool]` | — |
| `const.false` | 0x04 | — | `[] -> [bool]` | — |
| `const.str` | 0x05 | u16 string idx | `[] -> [string]` | idx di luar tabel |

## Variabel

| Op | Byte | Operand | Efek stack | Error |
| --- | --- | --- | --- | --- |
| `load.local` | 0x10 | u16 slot | `[] -> [T]` | slot di luar locals |
| `store.local` | 0x11 | u16 slot | `[T] -> []` | underflow, tipe lawan slot |
| `load.global` | 0x12 | u16 idx | `[] -> [T]` | idx di luar globals |
| `store.global` | 0x13 | u16 idx | `[T] -> []` | underflow, tipe lawan global |

## Aritmetika

Integer (`add.i` 0x20, `sub.i` 0x21, `mul.i` 0x22, `div.i` 0x23, `mod.i` 0x24):
`[i32, i32] -> [i32]`. Underflow atau operand non-i32 ditolak.
Float (`add.f` 0x28, `sub.f` 0x29, `mul.f` 0x2A, `div.f` 0x2B, `mod.f` 0x2C):
`[f32, f32] -> [f32]`.
Negasi (`neg.i` 0x25, `neg.f` 0x2D): `[T] -> [T]`.
`div.i`/`mod.i` dengan pembagi nol = runtime error (Milestone 7).

## Perbandingan (hasil bool)

- Integer: `eq.i` 0x30, `ne.i` 0x31, `lt.i` 0x32, `le.i` 0x33, `gt.i` 0x34, `ge.i` 0x35.
  `[i32, i32] -> [bool]`.
- Float: `eq.f` 0x38, `ne.f` 0x39, `lt.f` 0x3A, `le.f` 0x3B, `gt.f` 0x3C, `ge.f` 0x3D.
  `[f32, f32] -> [bool]`.
- Bool: `eq.b` 0x40, `ne.b` 0x41. String: `eq.s` 0x44, `ne.s` 0x45.
- Operand salah tipe atau underflow ditolak.

## Boolean

`and` 0x48, `or` 0x49: `[bool, bool] -> [bool]`. `not` 0x4A: `[bool] -> [bool]`.

## Lompat

| Op | Byte | Operand | Efek stack | Error |
| --- | --- | --- | --- | --- |
| `jump` | 0x50 | u32 target absolut | tidak ubah | target bukan batas instruksi |
| `jump.if_false` | 0x51 | u32 target absolut | `[bool] -> []` | underflow, bukan bool, target salah |

Target = offset byte dari awal code. Harus `< len` dan tepat di awal instruksi.

## Panggil

| Op | Byte | Operand | Efek stack | Error |
| --- | --- | --- | --- | --- |
| `call.func` | 0x60 | u16 fn idx | pop param kaliee, push tidak ada (void) | idx salah, tipe argumen lawan metadata |
| `call.host` | 0x61 | u16 host idx | pop param host, push ret non-void | idx salah, tipe argumen lawan signature |

Fungsi v0.1 selalu void, jadi tidak ada nilai kembali di stack.

## Kontrol

| Op | Byte | Operand | Efek stack | Error |
| --- | --- | --- | --- | --- |
| `return` | 0x70 | — | stack harus kosong | nilai tersisa di stack |

Fungsi harus diakhiri `return`. Kedalaman stack maksimum 1024.
