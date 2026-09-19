# Contributing

## How to Contribute

1. **Fork** this repository
2. **Create** a feature branch (`git checkout -b feature/your-feature`)
3. **Commit** your changes (`git commit -m 'Add your feature'`)
4. **Push** to the branch (`git push origin feature/your-feature`)
5. **Open** a Pull Request

## Code Style

- Use English for all comments and documentation
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Prefer `anyhow::Result` for error handling
- Use `log` crate for logging (not `println!`)
- Keep `playdiaemu-core` free of host I/O; frontends own files and windows

## Areas That Need Help

- **Game compatibility testing** — test more Redump titles and report issues with notes
- **Video codec** — AK8000 VLC / AC tables still approximate; fixture-proven diffs welcome
- **HLE playback** — improve scene navigation, timing, and disc compatibility
- **Platform ports** — macOS, Linux, Android, and iOS testing and packaging
- **Documentation** — improve docs and code comments
- **Bug reports** — if a disc doesn't play correctly, please open an issue
- **RetroArch integration** — dual-track CUE loading, compatibility testing across frontends

## Getting Started

Prefer synthetic fixtures under `crates/playdiaemu-core/tests/` for regressions.
Do not commit disc images or extracted copyrighted assets.

To understand the Playdia disc format (CDS-XA, F1/F2/F3 markers, XA ADPCM),
see [Game File Formats](Game-File-Formats.md).
