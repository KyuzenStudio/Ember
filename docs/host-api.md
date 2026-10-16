# Host API v0.1 + v2 (M16)

Satu-satunya jalan script menyentuh engine. Signature terdaftar di `ember-core/src/host.rs`
dan dipakai checker, compiler, dan VM. `f32.of_i`, `i32.of_f`, `str.of_f` SENGAJA
tidak didaftarkan (checker menolak sebagai unknown host function).

| Path | Parameter | Return | Efek |
| --- | --- | --- | --- |
| `log` | `string` | `void` | tulis ke Console |
| `input.down` | `i32` | `bool` | tombol sedang ditekan |
| `input.pressed` | `i32` | `bool` | tombol baru ditekan sejak tick lalu (sekali per tekan) |
| `transform.move` | `f32, f32` | `void` | geser entity pemilik script |
| `transform.x` / `transform.y` | — | `f32` | baca posisi sendiri |
| `transform.set` | `f32, f32` | `void` | teleport entity pemilik script |
| `entity.x` / `entity.y` | `string` (nama) | `f32` | baca posisi entity lain; error bila tak ada |
| `entity.destroy` | `string` (nama) | `void` | hapus entity + scriptnya, diterapkan setelah script; error bila tak ada |
| `str.of_i` | `i32` | `string` | konversi eksplisit ke string (murni, tanpa host state) |
| `str.cat` | `string, string` | `string` | gabung string (murni) |
| `audio.play` / `audio.loop` | `string` (path aset) | `void` | antre perintah audio; backend nyata di M19 |
| `audio.stop` | — | `void` | antre henti audio |
| `anim.play` | `string` (klip) | `void` | antre ganti klip; komponen klip di M18 |
| `game.restart` | — | `void` | minta bangun ulang runtime (state script ter-reset) |
| `game.set_title` | `string` | `void` | judul window berikutnya (diabaikan bila tanpa window) |

## Konstanta `Key` (tipe `i32`, nilai stabil)

| Nama | Nilai |
| --- | --- |
| `Key.Up` | 0 |
| `Key.Down` | 1 |
| `Key.Left` | 2 |
| `Key.Right` | 3 |
| `Key.Space` | 4 |
| `Key.Escape` | 5 |

## Lifecycle

- `on_create()`: sekali saat spawn. Boleh tidak ada.
- `on_update(dt: f32)`: tiap frame, `dt` detik (clamp 0.1). Boleh tidak ada.
- `on_collision_begin(other: string)` / `on_collision_end(other: string)`: saat kontak
  fisika mulai/berakhir, `other` nama entity lawan. Boleh tidak ada.
- Global `var` dievaluasi urut deklarasi sebelum `on_create`.
- Override nilai dari editor diterapkan setelah `$init`, sebelum `on_create`.
