# Releasing

Each crate in socle is versioned and released independently using
prefixed git tags: `<crate>/v<semver>`.

Only crates with an FFI layer (and therefore pre-built binaries) need a
release workflow. Pure Rust crates are published to crates.io directly.

## orkester

### What the CI builds

The `.github/workflows/release-orkester.yml` workflow triggers on tags
matching `orkester/v*`. It builds `orkester-ffi` as a static library for 4 targets:

| Target                        | Runner         | Notes              |
|-------------------------------|----------------|--------------------|
| `x86_64-unknown-linux-gnu`    | ubuntu-latest  |                    |
| `x86_64-apple-darwin`         | ubuntu-latest  | cross-compile      |
| `aarch64-apple-darwin`        | ubuntu-latest  | cross-compile      |
| `x86_64-pc-windows-msvc`      | windows-latest |                    |

macOS targets are cross-compiled from Linux. Since `orkester-ffi` produces a
`staticlib` (no linking step), no macOS SDK is required.

Each tarball contains:

```
lib/orkester_ffi.lib (or .a)
include/orkester.h
lib/cmake/orkester/orkesterConfig.cmake
lib/cmake/orkester/orkesterConfigVersion.cmake
```

### Creating a release

1. Make sure all changes are committed and pushed.

2. Tag and push:

   ```sh
   git tag orkester/v0.1.0
   git push origin orkester/v0.1.0
   ```

3. CI creates a GitHub Release at
   `https://github.com/calebbuffa/socle/releases/tag/orkester/v0.1.0`
   with the 4 platform tarballs attached.

4. Update the SHA512 hashes in the cesium-native vcpkg overlay port
   (`cesium-native/extern/vcpkg/ports/orkester/portfile.cmake`).

   Download each tarball and compute its hash:

   ```sh
   sha512sum orkester-x86_64-unknown-linux-gnu.tar.gz
   sha512sum orkester-x86_64-apple-darwin.tar.gz
   sha512sum orkester-aarch64-apple-darwin.tar.gz
   sha512sum orkester-x86_64-pc-windows-msvc.tar.gz
   ```

   Replace the `ORKESTER_SHA512` values in `portfile.cmake` with the new
   hashes. This step is required for every release because the tarball
   contents change, producing new hashes. vcpkg will reject downloads
   whose hash doesn't match.

5. Update `ORKESTER_VERSION` in `portfile.cmake` and `version-semver` in
   `vcpkg.json` to the new version.

### Bumping the version

1. Update `version` in `crates/orkester/Cargo.toml` and
   `crates/orkester-ffi/Cargo.toml`.
2. Update `VERSION` in the root `CMakeLists.txt`.
3. Tag and push (see above).
4. After CI finishes, update the portfile hashes and version (steps 4-5
   above).

## Adding a new crate release

To release another crate (e.g., selekt):

1. Create `.github/workflows/release-selekt.yml` triggered on `selekt/v*`.
2. Create a vcpkg overlay port under
   `cesium-native/extern/vcpkg/ports/selekt/`.
3. Add a section to this document.

## Local development (no release needed)

```sh
# Build orkester-ffi
cd socle
cargo build -p orkester-ffi

# Install to a local prefix
cmake -B build -DCMAKE_INSTALL_PREFIX=$PWD/install
cmake --build build
cmake --install build

# Point cesium-native at the local install
cd ../cesium-native
cmake -B build -DCMAKE_PREFIX_PATH=$PWD/../socle/install
```
