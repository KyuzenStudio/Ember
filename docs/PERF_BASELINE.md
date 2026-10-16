# Baseline performa (M15)

<!-- PERF:debug -->
## Profil debug (windows x86_64)

Uji: `cargo test -p ember-runtime --test perf_baseline` (menulis ulang bagian ini).

| Metrik | Nilai |
| --- | --- |
| VM update (2000 tick, 1 skrip, loop 100) | 326.42 us/tick |
| Fisika (600 tick, 50 body dinamis) | 843.57 us/tick |
| Kompilasi skrip kecil (200x) | 51.1 us/kompilasi |
| Draw count (200 sprite, CPU `scene_sprites`) | 200 draws, 121.1 us |
| Frame GPU | 5.070 ms/frame (200 sprites, 800x600 offscreen) |
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
