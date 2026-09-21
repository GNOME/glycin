project(
  'glycin',
  'rust',
  'c',
  # This is a global version that's used for libglycin, glycin-loaders, and glycin-thumbnailers.
  # It is however not used for Rust crates released on crates.io.
  version: '2.2.1',
  meson_version: '>=1.2',
)

gio_req = '>=2.60'
gtk4_req = '>= 4.16.0'
seccomp_req = '>= 2.5.0'
fontconfig_req = '>= 2.13.0'

is_windows_target = host_machine.system() == 'windows'

cc = meson.get_compiler('c')
windows_msvc = is_windows_target and cc.get_argument_syntax() == 'msvc'
windows_gnu = is_windows_target and cc.get_argument_syntax() == 'gcc'

compat_version = '2+'

datadir = get_option('prefix') / get_option('datadir')
bindir = get_option('prefix') / get_option('bindir')
libdir = get_option('prefix') / get_option('libdir')
includedir = get_option('prefix') / get_option('includedir')

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_manifest = meson.project_source_root() / 'Cargo.toml'

cargo_env = environment()
cargo_env.set('CARGO_HOME', cargo_home)

cargo_bin = find_program('cargo')
is_rustc_cross_build = false

# Use `cargo version -v` to find the (default) target arch and toolchain, which equates to the
# `host: ...` line
# This gives `host: <arch>-<platform>-<compiler_toolchain>`
cargo_version_details = run_command(cargo_bin, 'version', '-v', check: true).stdout().strip().split('\n')
foreach part : cargo_version_details
  p = part.split(':')
  if p[0] != part and p[0] == 'host'
    cargo_default_toolchain_parts = p[1].strip().split('-')
    cargo_host_arch = cargo_default_toolchain_parts[0]
  endif
endforeach

# Check whether the RustC cross-compilation mode (--target=xxx) is to be used.
# If this is a cross build, we proceed as we did before; otherwise on Windows builds
# we check the default host given by `cargo version -v` and see whether the info
# matches the compiler and target arch we are using.
if meson.is_cross_build()
  is_rustc_cross_build = true
  cargo_build_target = meson.get_external_property('rust_target')
else
  rustc_target_platform = []
  rustc_target_toolchain = []
  rustc_target_arch = []

  # Currently, this checks for non-cross Windows builds only
  if is_windows_target
    if windows_msvc and cargo_default_toolchain_parts[3] != 'msvc'
      is_rustc_cross_build = true
    endif
    if windows_gnu and cargo_default_toolchain_parts[3] != 'gnu'
      is_rustc_cross_build = true
    endif
    target_arch = host_machine.cpu_family()
    if target_arch != cargo_host_arch
      if cargo_host_arch != 'i686' or target_arch != 'x86'
        is_rustc_cross_build = true
      endif
    endif
    if is_rustc_cross_build
      if windows_msvc
        rustc_target_toolchain = 'msvc'
      elif windows_gnu
        rustc_target_toolchain = 'gnu'
      endif
      rustc_target_platform = 'pc-windows'
      rustc_target_arch = target_arch == 'x86' ? 'i686' : target_arch
    endif
  endif
  if is_rustc_cross_build
    cargo_build_target = f'@rustc_target_arch@-@rustc_target_platform@-@rustc_target_toolchain@'
  endif
endif

message('RustC cross build carried out: @0@'.format(is_rustc_cross_build ? 'YES' : 'NO'))

if is_rustc_cross_build
  cargo_target_args = ['--target', cargo_build_target]
  cargo_artifact_dir = cargo_target_dir / cargo_build_target
  cargo_env.set('PKG_CONFIG', find_program('pkg-config', required: true).full_path())
else
  cargo_target_args = []
  cargo_artifact_dir = cargo_target_dir
endif

# Prepend the paths in -Dpkg_config_path=... for Cargo
pkg_config_path_opt = get_option('pkg_config_path')

if pkg_config_path_opt != []
  path_sep = is_windows_target ? ';' : ':'
  paths = pkg_config_path_opt[0].split(path_sep)
  n_paths = paths.length()
  foreach i : range(0, n_paths)
    cargo_env.prepend(
      'PKG_CONFIG_PATH',
      paths[n_paths - i - 1],
    )
  endforeach
endif

if get_option('libglycin') and (get_option('libglycin-gtk4') or get_option('glycin-thumbnailer'))
  # Let the libglycin-gtk4 or glycin-thumbnailer build find the previously
  # built libglycin
  cargo_env.prepend(
    'PKG_CONFIG_PATH',
    meson.project_build_root() / 'meson-uninstalled',
  )
endif

if get_option('profile') == 'release'
  rust_target = 'release'
  cargo_profile = 'release'

  if get_option('lto') == 'auto'
    # Disable LTO for architectures other than x86_64 and aarch64
    # There have been frequent built issues with memory limits on 32-bit architectures
    if cargo_host_arch != 'x86_64' and cargo_host_arch != 'aarch64'
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

test_env = {}

if get_option('test_disable_sandbox')
  test_env += {'GLYCIN_DISABLE_SANDBOX': 'i-know-the-risks'}
endif

introspection = (get_option('introspection') or get_option('vapi') or get_option('capi_docs'))
glycin_loaders = (get_option('glycin-loaders') and host_machine.system() == 'linux')

if get_option('introspection') != introspection
  summary('introspection', introspection, section: 'Overwritten options')
endif

if glycin_loaders != get_option('glycin-loaders')
  summary('glycin-loaders', glycin_loaders, section: 'Overwritten options')
endif

# Windows targets might not have 'cp' available, so fallback
# to 'build-aux/cp-fallback.py' in these cases.
copy_cmd = find_program('cp', required: not is_windows_target)
if not copy_cmd.found()
  copy_cmd = find_program('build-aux/cp-fallback.py')
endif

subdir('glycin-loaders')
subdir('libglycin')
subdir('glycin-thumbnailer')
