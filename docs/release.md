# Cutting a release

Publishing a GitHub Release builds a binary for every platform, attaches them to
the release, and pushes a Docker image. The tag is the version.

## Requirements

- Push access to the repository
- The `gh` CLI, authenticated
- A green `main`. CI runs the same gates on every push there

## Cutting one

```bash
gh release create v0.1.0 --generate-notes
```

`--generate-notes` drafts the body from the pull requests merged since the last
tag. Publishing the release fires `.github/workflows/release.yml`, which does
four things:

1. Builds each asset in the matrix, stamping the tag into `Cargo.toml` and
   `Cargo.lock` first, so the binary reports the release version
2. Uploads every asset to the release
3. Generates `SHA256SUMS` from the uploaded assets and attaches it
4. Builds a multi-architecture Docker image and pushes it to GHCR

## The assets

| Asset | Target | Notes |
|---|---|---|
| `simple-confidence-monitor-x86_64-linux` | `x86_64-unknown-linux-musl` | Static, runs anywhere |
| `simple-confidence-monitor-aarch64-linux` | `aarch64-unknown-linux-musl` | Static. Pi, ARM NAS, ARM cloud |
| `simple-confidence-monitor-x86_64-macos` | `x86_64-apple-darwin` | Intel Macs |
| `simple-confidence-monitor-aarch64-macos` | `aarch64-apple-darwin` | Apple Silicon |
| `simple-confidence-monitor-x86_64-windows.exe` | `x86_64-pc-windows-msvc` | |
| `simple-confidence-monitor-aarch64-windows.exe` | `aarch64-pc-windows-msvc` | ARM Windows |
| `SHA256SUMS` | | Checksums for every asset above |

The two Linux assets link statically against musl, cross-linked with Zig, so they
carry no libc dependency. macOS and Windows build natively or cross-compile on
their own runners.

This table, the matrix in `release.yml` and the matrix in `ci.yml` all describe
the same set. CI fails if the two matrices drift apart, so change all three
together.

A macOS asset is unsigned. Gatekeeper needs one clearance per download:

```bash
xattr -d com.apple.quarantine simple-confidence-monitor-aarch64-macos
```

## Docker

```bash
docker run --rm -p 8080:8080 -v scm-state:/data \
  ghcr.io/kylejameswalker/simpleconfidencemonitor:latest --token s3cret
```

The image is Alpine with the static Linux binary, running as a non-root user. It
serves on 8080 and keeps snapshots in `/data`, so mount a volume there to keep
rooms across a restart.

The port and the state directory sit in the image `ENTRYPOINT` rather than its
`CMD`. Arguments after the image name are added to them rather than replacing
them, so passing `--token` cannot silently turn persistence off.

`deploy/portainer-stack.yml` is the same thing as a compose stack, for Portainer
or plain `docker compose`. It needs `SCM_TOKEN` set, and takes `SCM_TAG`,
`SCM_PORT` and `TZ`.

| Tag | Points at |
|---|---|
| `latest` | The most recent release |
| `vX.Y.Z` | That release |
| `edge` | The current `main` |
| `pr-<number>` | An open pull request |
| `sha-<commit>` | One commit |

A closing pull request takes its `pr-` and `sha-` tags with it.

Discovery over mDNS needs host networking, so the image leaves it off. Pass
`--mdns` with `--network host` if a container has to advertise itself.

## What CI checks

`.github/workflows/ci.yml` runs on every pull request and every push to `main`:

- `cargo fmt --check`, `clippy` with warnings denied, and the Rust test suite
- The JavaScript test suites under `node --test`
- A syntax check over every frontend script
- A release build of all six targets, so a cross-compile break shows up on the
  pull request rather than at release time
- The matrix sync check
- A Docker build, tagged for the pull request or for `main`

## If a release fails

The workflow uploads with `--clobber`, so re-running a failed job replaces a
partial asset rather than duplicating it. Re-run from the Actions tab.

A release with missing assets is worth deleting and cutting again on the same
tag. The workflow generates `SHA256SUMS` from the uploaded assets, so a partial
set leaves the checksums describing something incomplete.
