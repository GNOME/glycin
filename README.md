# Glycin

Glycin allows to decode, edit, and create images and read metadata. The decoding happens in sandboxed modular *image loaders and editors*.

- [glycin](https://docs.rs/glycin/) – The Rust image library
    - [libglycin](https://gnome.pages.gitlab.gnome.org/glycin/libglycin/) – C-Bindings for the library
    - [libglycin-gtk4](https://gnome.pages.gitlab.gnome.org/glycin/libglycin-gtk4/) – C-Bindings to convert glycin frames to GDK Textures
- [glycin-loaders](glycin-loaders) – Glycin loaders for several formats
- [glycin-thumbnailers](glycin-thumbnailers) – Glycin thumbnailer using the installed loaders

Other rust crates:

- [glycin-utils](https://docs.rs/glycin-utils/) – Utilities to write loaders for glycin
- [glycin-common](https://docs.rs/glycin-common/) – Components shared between the glycin-utils and glycin crates.

## Usage and Packaging

The Rust client library is available as [glycin on crates.io](https://docs.rs/glycin/). For other programming languages, the libglycin C client library can be used. For the client libraries to work, **loader binaries must also be installed**. The loader binaries provided by the glycin project cover a lot of common image formats (see below). Both, the loader binaries and libglycin can be built from the released [glycin tarballs](https://download.gnome.org/sources/glycin/). By using `-Dglycin-thumbnailer=false`, `-Dglycin-loaders=false`, `-Dlibglycin=false`, or `-Dlibglycin-gtk4=false` it is possible to build only specific components. In distributions, the loaders are usually packaged as *glycin-loaders*, and libglycin as *libglycin-2*. However, each loader binary could be also packaged as its own package.

### Example

```rust
let file = gio::File::for_path("image.jpg");
let image = Loader::new(file).load().await?;

let height = image.info().height();
let texture = image.next_frame().await?.texture();
```

## Limitations

Glycin is based on technologies like memfds, unix sockets, and linux namespaces. It currently only works on Linux. An adoption to other unixoid systems could be possible without usage of the sandbox mechanism. Windows support is currently not planned and might not be feasible.

## Supported Image Formats

The following features are supported by the glycin loaders provided in the [loaders](loaders) directory.

| Format       | Decoder  | ICC | CICP | EXIF | XMP | Animation | Library                    |
|--------------|----------|-----|------|------|-----|-----------|----------------------------|
| AVIF         | heif     | ✔   | ✔    | ✔    | ✘   | ✘         | libheif-rs + libheif (C++) |
| BMP          | image-rs | ✘   | —    | —    | —   | —         | image-rs                   |
| Camera RAW † | raw      | —   | —    | ✘    | ✔ § | —         | libopenraw                 |
| DDS          | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| farbfeld     | no mime  | —   | —    | —    | —   | —         | image-rs                   |
| QOI          | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| GIF          | image-rs | ✘   | —    | —    | ✘   | ✔         | image-rs                   |
| HEIC         | heif     | ✔   | ✔    | ✔    | ✘   | ✘         | libheif-rs + libheif (C++) |
| ICO          | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| JPEG         | image-rs | ✔   | —    | ✔    | ✔   | —         | image-rs                   |
| JPEG XL      | jxl      | ✔   | ✘    | ✔    | ✘   | ✘         | jpegxl-rs + libjxl (C++)   |
| OpenEXR      | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| PNG          | image-rs | ✔   | ✔    | ✔    | ✔   | ✔         | image-rs                   |
| PNM          | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| SVG          | image-rs | ✘   | —    | —    | ✘   | —         | librsvg + gio/cairo (C)    |
| TGA          | image-rs | —   | —    | —    | —   | —         | image-rs                   |
| TIFF         | image-rs | ✔   | —    | ✔    | ✘   | —         | image-rs                   |
| WEBP         | image-rs | ✔   | —    | ✔    | ✘   | ✔         | image-rs                   |

| Symbol | Meaning                                     |
|--------|---------------------------------------------|
| ✔      | Supported                                   |
| ✘      | Supported by format but not implemented yet |
| —      | Not available for this format               |
| †      | Camera RAW format support varies            |
| §      | DNG or XMP sidecar if available             |

## Image Loader Configuration

Loader configurations are read by the client library from `XDG_DATA_DIRS` and `XDG_DATA_HOME`. The location is typically of the from

```
<data-dir>/share/glycin/<compat-version>+/conf.d/<loader-name>.conf
```

so for example

```
<data-dir>/share/glycin/0+/conf.d/glyicn-image-rs.conf
```

The configs are [glib KeyFiles](https://docs.gtk.org/glib/struct.KeyFile.html) of the the form

```ini
[loader:image/png]
Exec = /usr/libexec/glycin/2+/glycin-image-rs
```

Where the part behind `loader` is a mime-type and the value of `Exec` can be any executable path.

### Existing Compatibility Versions

Not every new major version of the library has to break compatibility with the loaders. If a glycin version X breaks compatibility, the new compativility version will be called X+. Only glycin X and newer version will be compatible with X+ until a new compatibilityv version is used. The definition of the API of each compatibility version is available in [`docs/`](docs/). The following compatibility versions currently exist

| compat-version | Compatible With                |
|----------------|--------------------------------|
| 0+             | glycin 0.x                     |
| 1+             | glycin 1.x, 2.x; libglycin 1.x |
| 2+             | glycin 3.x; libglycin 2.x      |

## Sandboxing and Inner Workings

Glycin spawns one process per image file. The communication between glycin and the loader takes place via peer-to-peer D-Bus over a Unix socket.

Glycin supports a sandbox mechanism inside and outside of Flatpaks. Outside of Flatpaks, the following mechanisms are used: The image loader binary is spawned via `bwrap`. The bubblewrap configuration only allows for minimal interaction with the host system. Only necessary parts of the filesystem are mounted and only with read access. There is no direct network access. Environment variables are not passed to the sandbox. Before forking the process the memory usage is limited via calling `setrlimit` and syscalls are limited to an allow-list via seccomp filters.

Inside of Flatpaks the `flatpak-spawn --sandbox` command is used. This restricts the access to the filesystem in a similar way as the direct `bwrap` call. The memory usage is limited by wrapping the loader call into a `prlimit` command. No additional seccomp filters are applied to the existing Flatpak seccomp rules.

The GFile content is streamed to the loader via a Unix socket. This way, loaders can load contents that require network access, without having direct network access themselves. Formats like SVG set the `ExposeBaseDir = true` option in their config. This option causes the original image file's directory to be mounted into the sandbox to include external image files from there. The `ExposeBaseDir` option has no effect for `flatpak-spawn` sandboxes since they don't support this feature.

The loaders provide the texture data via a memfd that is sealed by glycin and then given as an mmap to GDK. For animations and SVGs the sandboxed process is kept alive for new frames or tiles as long as needed.

For information on how to implement a loader, please consult the [`glycin-utils` docs](https://docs.rs/glycin-utils/).

## Building and Testing

- The `-Dloaders` option allows to only build certain loaders.
- The `-Dtest_skip_ext` option allows to skip certain image filename extensions during tests. The option `-Dtest_skip_ext=heic` might be needed if x265 is not available.
- Running integration tests requires the glycin loaders to be installed. By default, `meson test` creates an separate installation against which the tests are run. This behavior can be changed by setting `-Dtest_skip_install=true`, requiring to manually calling `meson install` before running the tests.
- The `glycin` crate has an example, `glycin-render` that will load the image passed as a parameter and render it as a PNG into `output.png` in the current directory.

## Packaging Status

[![Packaging Status](https://repology.org/badge/vertical-allrepos/glycin-loaders.svg?exclude_unsupported=1&header=)](https://repology.org/project/glycin-loaders/versions)

## Apps Using Glycin

- [Camera (Snapshot)](https://flathub.org/apps/org.gnome.Snapshot)
- [Fotema](https://flathub.org/apps/app.fotema.Fotema)
- [Fractal](https://flathub.org/apps/org.gnome.Fractal)
- [Identity](https://flathub.org/apps/org.gnome.gitlab.YaLTeR.Identity)
- [Image Viewer (Loupe)](https://flathub.org/apps/org.gnome.Loupe)
- [Shortwave](https://flathub.org/apps/de.haeckerfelix.Shortwave)

## The Name

[Glycin](https://en.wikipedia.org/wiki/Glycin) (ˈɡlaɪsiːn) is a photographic developing agent. There is no deeper meaning behind the name choice but using a somewhat unique name that is related to images. Glycin is often confused with the amino acid [glycine](https://en.wikipedia.org/wiki/Glycine), which is called glycin in other languages, like German.

## License

SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

The camera raw loader uses the crate `libopenraw` which is licensed as LGPL-3.0-or-later. The JPEG XL loader uses the `jpegxl-rs` and `jpegxl-sys` crates which are licensed as GPL-3.0-or-later. Given these are only separate executables, only the `glycin-raw` and the `glycin-jxl` binary falls under said licenses, and doesn't precludes using `glycin` under MPL-2.0 OR LGPL-2.1-or-later. This is not legal advice.
