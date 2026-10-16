# Format project dan scene

Hanya RON di v0.1. Versi format project: 1. Versi format scene: 3 (loader terima
v1–v3; save selalu tulis versi terbaru).

## Layout project (`ember new`)

```
MyGame/
├── ember.project
├── scenes/main.ron
├── scripts/player.em
├── assets/player.png
└── README.md
```

`build/` dan `dist/` dibuat oleh `ember build`/`ember package`, bukan `ember new`.

## `ember.project`

```
(
    format_version: 1,
    name: "MyGame",
    startup_scene: "scenes/main.ron",
)
```

## Scene (`scenes/*.ron`)

Scene berisi `entities` dengan ID stabil (`u64`, dipertahankan saat save/load).
Komponen per entity:

| Field | Isi |
| --- | --- |
| `transform` | `position`, `rotation_degrees`, `scale` |
| `sprite` | `texture` (path relatif), `tint` rgba, `sorting_order` |
| `camera` | `height` (unit dunia vertikal) |
| `script` | path `.em` relatif (opsional) |
| `script_values` | override nama→nilai (`I32`/`F32`/`Bool`/`Str`) |
| `collider` | `hx`, `hy` setengah ukuran kotak, `sensor` (opsional) |
| `rigidbody` | `kind` (`Static`/`Dynamic`), `velocity`, `gravity_scale` (opsional) |
| `anim` | `clips` (nama, `frames`, `fps`, `looped`), `current`, `t` (opsional) |

Field `script`/`script_values`/`collider`/`rigidbody`/`anim` punya default sehingga file lama tetap muat.

## Aturan path

- Relatif terhadap root project. Absolut, `..`, dan keluar-root ditolak.
- Symlink keluar root dilewati saat copy, dicek kanonis saat target ada.
- Ref script harus file `.em` yang ada. Nested `scripts/sub/*.em` ditolak v0.1.

## Build dan package

- `build/<nama>.embc`: bytecode biner (`EMBC` v1) per script top-level, sudah lolos verifier.
- `dist/<Game>-windows-x86_64/`: `ember-player` + `ember.project` + `scenes/` + `build/` + `assets/`.
- Player hanya baca `.embc`. Sumber `.em` tak ikut dieksekusi.
