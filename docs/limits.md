# Keterbatasan yang diketahui (v0.1)

## Bahasa dan VM

- Parser lapor error pertama saja (tanpa recovery).
- Fungsi selalu void. Tanpa generics, closures, macros, async, reflection, GC, dynamic typing.
- `return <expr>` ditolak. Statement ekspresi selain call ditolak.
- Integer wrapping. `div`/`mod` nol = runtime error. Float ikut IEEE.
- Budget instruksi default 10 juta. Kedalaman call 256, stack 1024.

## Workflow

- Tanpa file picker (path diketik). Open eksternal Windows saja.
- Tanpa undo/redo. Docking/layout editor bawaan eframe.
- `ember run` belum ada (jalankan via editor Play atau `ember-player`).
- Audio tidak ada. Fisika/collision ditunda (tercatat, bukan target v0.1).
- Sprite 1 unit dunia tinggi × aspek. Maksimal 1024 sprite.
- Tick berhenti di error script pertama lalu Stop.

## Fisika (Milestone 13)

- Rapier2D 0.36: body statis/dinamis + collider kotak saja. Tanpa sendi, tanpa raycast.
- Rotasi terkunci. Gravitasi global (0, -9.81), skala per-body.
- Fixed timestep 1/60, maksimal 3 step per tick (kelebihan dibuang).
- Tak ada API host fisika. Skrip amati lewat transform.
- Collider tak valid (ukuran ≤ 0) = load gagal dengan pesan jelas.

## Belum diverifikasi langsung

- Tampilan windowed player/editor di layar (kode lengkap + tes GPU parsial).
- Performa VM dan renderer (belum diukur).
- Fuzz input malformed.
- CI GitHub (belum ada run tercatat; gate lokal hijau).

## Non-goals (tetap dilarang)

Native compiler, JIT, LLVM, Cranelift, transpiler C/C++, renderer multi-backend,
custom UI framework, custom physics engine, visual scripting.
