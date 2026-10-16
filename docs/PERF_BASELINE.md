# Baseline performa (M15)

<!-- PERF:debug -->
## Profil debug (windows x86_64)

Uji: `cargo test -p ember-runtime --test perf_baseline` (menulis ulang bagian ini).

| Metrik | Nilai |
| --- | --- |
| VM update (2000 tick, 1 skrip, loop 100) | 314.44 us/tick |
| Fisika (600 tick, 50 body dinamis) | 808.65 us/tick |
| Kompilasi skrip kecil (200x) | 49.2 us/kompilasi |
| Draw count (200 sprite, CPU `scene_sprites`) | 200 draws, 152.0 us |
| Frame GPU | 5.202 ms/frame (200 sprites, 800x600 offscreen) |
<!-- /PERF:debug -->











<!-- PERF:release -->
## Profil release (windows x86_64)

Uji: `cargo test -p ember-runtime --test perf_baseline` (menulis ulang bagian ini).

| Metrik | Nilai |
| --- | --- |
| VM update (2000 tick, 1 skrip, loop 100) | 67.18 us/tick |
| Fisika (600 tick, 50 body dinamis) | 25.66 us/tick |
| Kompilasi skrip kecil (200x) | 8.0 us/kompilasi |
| Draw count (200 sprite, CPU `scene_sprites`) | 200 draws, 28.9 us |
| Frame GPU | 3.872 ms/frame (200 sprites, 800x600 offscreen) |
<!-- /PERF:release -->
