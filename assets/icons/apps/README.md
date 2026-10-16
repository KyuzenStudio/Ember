# Ember Engine — Brand Assets

## Konsep
Simbol adalah monogram **E** geometris. Stem dan dua lengan (atas/bawah) berwarna netral, dengan ujung lengan dipotong miring (sudut sama, 8:10). Lengan tengah dipisah menjadi blok **Ember Orange** yang lebih pendek: inti panas di dalam huruf, bukan ikon api yang ditempel. Potongan miring memberi kesan percikan/bara yang bergerak ke kanan; ruang negatif di antara lengan menjaga huruf E tetap terbaca.

Wordmark `EMBER` dibuat dari path SVG kustom (grid tinggi 24, tebal garis 5, spasi 4), sehingga **tidak ada dependensi font**.

## Palet
| Nama | Hex | Penggunaan |
|---|---|---|
| Obsidian | `#111318` | Simbol/wordmark di latar terang, latar gelap |
| Ember Orange | `#FF6A2A` | Hanya untuk inti (lengan tengah) |
| Warm Ivory | `#F2F0EA` | Simbol/wordmark di latar gelap, latar terang |

Semua file flat: tanpa gradien, bayangan, blur, filter, atau mask.

## Varian
| File | Latar | Catatan |
|---|---|---|
| `ember-symbol.svg` | Terang | Simbol saja, viewBox 64×64 |
| `ember-symbol-light.svg` | Gelap | Simbol saja (ivory + orange) |
| `ember-logo-dark.svg` | Terang | Simbol + wordmark Obsidian |
| `ember-logo-light.svg` | Gelap | Simbol + wordmark Ivory |
| `ember-logo-monochrome.svg` | Bebas | Satu warna (`#111318`), bentuk terbaca tanpa warna |
| `ember-icon-preview.svg` | — | Aset presentasi, bukan master |

Nama "dark/light" mengikuti spesifikasi: *dark* = warna gelap untuk latar terang.

## Ukuran minimum
- Simbol: 16 px (masih terbaca, inti berupa ~4 px oranye), disarankan ≥ 24 px.
- Logo lengkap: lebar ≥ 96 px (tinggi ≥ 24 px). Di bawah itu gunakan simbol saja.
- Cetak: simbol ≥ 6 mm, logo lengkap ≥ 24 mm lebar.

## Recolor / adaptasi
Setiap bentuk adalah `<path>` dengan atribut `fill` eksplisit. Ganti nilai hex langsung. Untuk monokrom, ubah semua `fill` ke satu warna. Jangan menambah stroke pada simbol. Jaga ruang kosong di sekeliling logo minimal setengah tinggi huruf E.

## Keterbatasan
- Pada 16 px inti oranye hanya beberapa piksel; mode monokrom 16 px akan menampilkan E dengan celah di tengah (tetap terbaca sebagai E).
- Simbol berwarna Obsidian tidak terlihat di latar gelap; pakai `ember-symbol-light.svg`.
- Wordmark berupa outline; teks tidak dapat diedit, dan `2D GAME ENGINE` sengaja tidak disertakan.
