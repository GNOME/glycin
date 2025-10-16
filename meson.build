project(
  'glycin',
  'rust',
  'c',
  # This is a global version that's neither related to the glycin-loaders version
  # nor the libglycin version
  version: '2.0.4',
  meson_version: '>=1.2',
)

lcms2_req = '>= 2.14'
gio_req = '>=2.60'
gtk4_req = '>= 4.16.0'
seccomp_req = '>= 2.5.0'
fontconfig_req = '>= 2.13.0'

compat_version = '2+'

datadir = get_option('prefix') / get_option('datadir')
bindir = get_option('prefix') / get_option('bindir')
libdir = get_option('prefix') / get_option('libdir')
includedir = get_option('prefix') / get_option('includedir')

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_manifest = meson.project_source_root() / 'Cargo.toml'

cargo_bin = find_program('cargo')
crate_version_bin = find_program(meson.project_source_root() / 'build-aux/crates-version.py')

cargo_env = environment()
cargo_env.set('CARGO_HOME', cargo_home)

if get_option('libglycin') and get_option('libglycin-gtk4')
  # Let the libglycin-gtk4 build find the previously built libglycin
  cargo_env.prepend(
    'PKG_CONFIG_PATH',
    meson.project_build_root() / 'meson-uninstalled',
  )
endif

if get_option('profile') == 'release'
  rust_target = 'release'
  cargo_profile = 'release'

  if get_option('lto') == 'auto'
    cargo_version_details = run_command(cargo_bin, 'version', '-v', check: true).stdout().strip()

    # Disable LTO for architectures other than x86_64 and aarch64
    # There have been frequent built issues with memory limits on 32-bit architectures
    if not cargo_version_details.contains('host: x86_64-') and not cargo_version_details.contains('host: aarch64-')
      message('LTO automatically disabled')
      cargo_env.set('CARGO_PROFILE_RELEASE_LTO', 'false')
    endif
  elif get_option('lto') == 'false'
    cargo_env.set('CARGO_PROFILE_RELEASE_LTO', 'false')
  endif

else
  rust_target = 'debug'
  cargo_profile = 'dev'
endif

introspection = (get_option('introspection') or get_option('vapi') or get_option('capi_docs'))

if get_option('introspection') != introspection
  summary('introspection', introspection, section: 'Overwritten options')
endif

subdir('glycin-loaders')
subdir('glycin-thumbnailer')
subdir('libglycin')
