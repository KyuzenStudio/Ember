# Ember

Game engine 2D Rust + bahasa scripting EmberScript (compiler, VM) + visual editor.
Status: v0.1 dalam pengembangan per milestone. Klaim fitur hanya sah dengan verification report.

## Mulai cepat

Butuh Rust stable (`rustup`), target `x86_64-pc-windows-msvc`.

```powershell
cargo build --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
# atau semuanya:
powershell -ExecutionPolicy Bypass -File scripts\check.ps1
```

```powershell
cargo run -p ember-cli -- new MyGame
cargo run -p ember-cli -- check MyGame
cargo run -p ember-cli -- build MyGame
cargo run -p ember-cli -- package MyGame
cargo run -p ember-editor -- MyGame
.\target\debug\ember-player.exe --version
```

## Isi workspace

| Crate | Isi |
| --- | --- |
| `ember-cli` | binary `ember` (`new`, `check`, `build`, `package`) |
| `ember-core` | `Vec2`, `Ty`, registry host API |
| `ember-scene` | scene/entity/komponen, save/load RON |
| `ember-project` | format project, template, validasi path, build, package |
| `ember-lang` | lexer, parser, AST, semantic, compiler bytecode |
| `ember-vm` | format bytecode, verifier, VM |
| `ember-runtime` | input, lifecycle script, tick |
| `ember-renderer` | sprite wgpu, kamera, picking |
| `ember-player` | binary `ember-player` (main dari bytecode) |
| `ember-editor` | binary `ember-editor` (egui) |

## Dokumen

- `PRODUCT.md`, `DESIGN.md`, `AGENT.md`: kontrak proyek dan cara kerja.
- `docs/setup.md`: prasyarat dan perintah.
- `docs/emberscript.md`: tur sintaks. `docs/emberscript-grammar.md`: EBNF formal.
- `docs/host-api.md`: fungsi host, konstanta `Key`, lifecycle.
- `docs/project-format.md`: format project, scene, build, package.
- `docs/opcodes.md`, `docs/bytecode.md`: instruksi dan format biner.
- `docs/limits.md`: batas dan hal yang belum diverifikasi.
- `docs/test-report.md`, `docs/acceptance.md`: hasil tes dan status acceptance.
