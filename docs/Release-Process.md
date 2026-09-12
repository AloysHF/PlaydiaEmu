# Release Process

This document describes how to publish a new release of PlaydiaEmu.

## Prerequisites

- Push access to the `master` branch
- Permission to create tags and releases on GitHub

## Steps

### 1. Update version numbers

Version numbers must be updated in **two files**:

| File | Field | Current |
|------|-------|---------|
| `Cargo.toml` (workspace root) | `[workspace.package] version` | `"0.1.0"` |
| `crates/playdia-libretro/src/lib.rs` | `library_version` in `retro_get_system_info` | `"0.1.0"` |

Both values must match. If a `playdia_libretro.info` file is added later, keep
its `display_version` in sync as well — RetroArch reads `display_version` to
display the core version to users.

```bash
# Example: bumping to 0.2.0
# 1. Edit Cargo.toml
sed -i 's/^version = "0.1.0"/version = "0.2.0"/' Cargo.toml

# 2. Edit libretro library_version
# crates/playdia-libretro/src/lib.rs → library_version: c"0.2.0\0"
```

### 2. Commit the version bump

```bash
git add Cargo.toml Cargo.lock crates/playdia-libretro/src/lib.rs
git commit -m "chore: bump version to 0.2.0"
git push origin master
```

### 3. Create and push a tag

The release workflow triggers on tags matching `v*` (e.g. `v0.2.0`).

```bash
git tag v0.2.0
git push origin v0.2.0
```

### 4. CI builds and creates a draft release

Pushing the tag triggers `.github/workflows/release.yml`, which:

1. **Builds standalone binaries** for Linux, macOS (x86_64 + aarch64), and Windows
2. **Builds libretro cores** for the same platforms as `playdia_libretro.<ext>`
3. **Creates a draft GitHub Release** with:
   - Auto-generated release notes (PRs and commits since the previous tag)
   - All build artifacts attached

### 5. Review and publish the release

1. Go to [Releases](https://github.com/AloysHF/PlaydiaEmu/releases)
2. Find the draft release created by CI
3. Review the auto-generated changelog — edit if needed
4. Verify all expected artifacts are attached:
   - `playdia-emu-linux-x86_64.tar.gz`
   - `playdia-emu-macos-x86_64.tar.gz`
   - `playdia-emu-macos-aarch64.tar.gz`
   - `playdia-emu-windows-x86_64.zip`
   - `*-libretro.*` (one per platform)
5. Click **Publish release**

### 6. Sync `.info` file to upstream libretro-super (when present)

RetroArch's **Online Updater > Core Downloader** reads the `.info` file from the
upstream [libretro-super](https://github.com/libretro/libretro-super) repository,
not from this repo. Once `crates/playdia-libretro/playdia_libretro.info` exists
and changes in a release, submit a PR to sync it:

1. Fork [libretro/libretro-super](https://github.com/libretro/libretro-super)
2. Copy `crates/playdia-libretro/playdia_libretro.info` from this repo
   to `dist/info/playdia_libretro.info` in the fork
3. Submit a PR to `libretro/libretro-super` — reference the PlaydiaEmu release
   tag and list the changed fields in the PR description

## Troubleshooting

### CI build fails

- Check the [Actions](https://github.com/AloysHF/PlaydiaEmu/actions) tab
  for the failed run
- The most common failure is a missing Linux build dependency — the CI installs
  `libasound2-dev`, `libx11-dev`, and `libxkbcommon-dev` automatically

### Re-triggering a release

The release workflow only runs on tag pushes. To re-trigger:

```bash
# 1. Delete the tag locally and remotely
git tag -d v0.2.0
git push origin --delete v0.2.0

# 2. Re-push the tag (CI will re-run)
git push origin v0.2.0
```

If a draft release was already created by the failed run, delete it from the
Releases page before re-pushing the tag, otherwise the new run may conflict
with the existing draft.

### Release artifacts missing

- Verify the tag name starts with `v` (e.g. `v0.2.0`, not `0.2.0`)
- The release workflow only triggers on tag pushes (`v*`); manually dispatching
  the workflow from the Actions tab will not create a release

### Version mismatch in RetroArch

- Ensure `Cargo.toml` and `library_version` in
  `crates/playdia-libretro/src/lib.rs` have the same version string
- If an `.info` file is added later, it is bundled as-is into the release artifacts
