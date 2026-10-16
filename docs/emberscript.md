# EmberScript v0.1

Statis, sintaks mirip Lua (bukan kompatibel Lua). Satu file `.em` = satu `ScriptComponent`.
Grammar formal: `emberscript-grammar.md`.

## Contoh lengkap

```
var speed: f32 = 6.0

fn on_create() {
    log("Player created")
}

fn on_update(dt: f32) {
    if input.down(Key.Right) {
        transform.move(speed * dt, 0.0)
    }
    if input.down(Key.Left) {
        transform.move(-speed * dt, 0.0)
    }
}
```

## Aturan

- Tipe: `i32`, `f32`, `bool`, `string`, `void`. Tanpa konversi implisit.
- `var` wajib bertipe dan berinisialisasi, di level file maupun blok.
- `var` level file = state instance script.
- Fungsi bebas boleh ada; yang dipanggil runtime hanya `on_create()` dan `on_update(dt: f32)`.
- Semua fungsi return void. `return` tanpa nilai boleh, `return <expr>` ditolak.
- Statement ekspresi selain call ditolak (`1 + 2` sendirian error).
- Nama bertitik (`input.down`, `Key.Right`) = path host API, bukan akses objek.
- Komentar `//` satu baris. Tanpa titik koma.
- Blok `entity` belum ada (ditunda).

## Error

Setiap error membawa `file:baris:kolom` dan pesan. Contoh:

```
player.em:2:11: expected ':', found '='
player.em:1:18: operator not defined for i32 and bool
```

## Batas

Parser lapor error pertama saja. Shadowing blok dalam boleh. Duplikat se-scope ditolak.
`void` tidak bisa jadi tipe variabel/parameter.
