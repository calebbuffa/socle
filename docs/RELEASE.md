# Releasing orkester

Each crate and its FFI are built and published independently.

## How it works

The `.github/workflows/release.yml` workflow triggers on tag pushes matching `v*`.
It builds `orkester-ffi` as a static library for 4 targets:

| Target                        | Runner         | Cost (GitHub Free) |
|-------------------------------|----------------|--------------------|
| `x86_64-unknown-linux-gnu`    | ubuntu-latest  | 1x                 |
| `x86_64-apple-darwin`         | ubuntu-latest  | 1x (cross-compile) |
| `aarch64-apple-darwin`        | ubuntu-latest  | 1x (cross-compile) |
| `x86_64-pc-windows-msvc`      | windows-latest | 2x                 |

macOS targets are cross-compiled from Linux. Since `orkester-ffi` produces a
`staticlib` (no linking step), no macOS SDK is required. This avoids the 10x
cost of macOS runners on GitHub Free.

Each tarball contains:
```
lib/orkester_ffi.lib (or .a)
include/orkester.h
lib/cmake/orkester/orkesterConfig.cmake
```

The workflow creates a GitHub Release with all 4 tarballs attached.

## Creating a release

1. Make sure all changes are committed and pushed to `main`.

2. Tag the release:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. The CI workflow runs automatically. When it completes, a GitHub Release
   appears at `https://github.com/calebbuffa/socle/releases/tag/v0.1.0`
   with the 4 platform tarballs.

4. After the release is published, update the SHA512 hashes in the
   cesium-native vcpkg overlay port:
   ```
   cesium-native/extern/vcpkg/ports/orkester/portfile.cmake
   ```

   Download each tarball and compute its hash:
   ```sh
   sha512sum orkester-x86_64-unknown-linux-gnu.tar.gz
   sha512sum orkester-x86_64-apple-darwin.tar.gz
   sha512sum orkester-aarch64-apple-darwin.tar.gz
   sha512sum orkester-x86_64-pc-windows-msvc.tar.gz
   ```

   Replace the placeholder `"0"` values in `portfile.cmake` with the real hashes.

## Bumping the version

1. Update `version` in `crates/orkester/Cargo.toml` and `crates/orkester-ffi/Cargo.toml`.
2. Update `VERSION` in `CMakeLists.txt` (`project(socle VERSION x.y.z ...)`).
3. Update `ORKESTER_VERSION` in `cesium-native/extern/vcpkg/ports/orkester/portfile.cmake`.
4. Update `version-semver` in `cesium-native/extern/vcpkg/ports/orkester/vcpkg.json`.
5. Tag and push (see above).

## Local development (no release needed)

For local development against cesium-native without publishing a release:

```sh
# Build orkester-ffi
cd socle
cargo build -p orkester-ffi

# Install to a local prefix
cmake -B build -DCMAKE_INSTALL_PREFIX=$PWD/install
cmake --build build
cmake --install build

# Configure cesium-native to use the local install
cd ../cesium-native
cmake -B build -DCMAKE_PREFIX_PATH=$PWD/../socle/install
```
