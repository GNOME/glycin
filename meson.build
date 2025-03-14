project(
  'glycin',
  'rust',
  'c',
  # This is a global version that's neither related to the glycin-loaders version
  # nor the libglycin version
  version: '1.2.0',
  meson_version: '>=1.2',
)

lcms2_req = '>= 2.14'
gio_req = '>=2.60'
gtk4_req = '>= 4.16.0'
seccomp_req = '>= 2.5.0'

compat_version = '1+'

datadir = get_option('prefix') / get_option('datadir')
libdir = get_option('prefix') / get_option('libdir')
includedir = get_option('prefix') / get_option('includedir')

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_bin = find_program('cargo')

if get_option('profile') == 'release'
  rust_target = 'release'
  cargo_profile = 'release'
else
  rust_target = 'debug'
  cargo_profile = 'dev'
endif

introspection = (get_option('introspection') or get_option('vapi') or get_option('capi_docs'))

if get_option('introspection') != introspection
  summary('introspection', introspection, section: 'Overwritten options')
endif

subdir('loaders')
subdir('libglycin')