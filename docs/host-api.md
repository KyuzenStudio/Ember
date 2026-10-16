# Host API v0.1

Satu-satunya jalan script menyentuh engine. Signature terdaftar di `ember-core/src/host.rs`
dan dipakai checker, compiler, dan VM. Daftar hanya bertambah karena game contoh.

| Path | Parameter | Return | Efek |
| --- | --- | --- | --- |
| `log` | `string` | `void` | tulis ke Console |
| `input.down` | `i32` | `bool` | tombol sedang ditekan |
| `transform.move` | `f32, f32` | `void` | geser entity pemilik script |

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
- Global `var` dievaluasi urut deklarasi sebelum `on_create`.
- Override nilai dari editor diterapkan setelah `$init`, sebelum `on_create`.
