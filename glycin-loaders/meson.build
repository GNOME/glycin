if not get_option('glycin-loaders')
  subdir_done()
endif

dependency('libseccomp', version: seccomp_req)

if 'glycin-heif' in get_option('loaders')
  dependency('libheif', version: '>= 1.17.0')
endif

if 'glycin-jxl' in get_option('loaders')
  dependency('libjxl', version: ['>= 0.11.0'])
endif

if 'glycin-svg' in get_option('loaders')
  dependency('librsvg-2.0', version: '>= 2.52.0')
  dependency('cairo', version: '>= 1.17.0')
endif

if get_option('tests')
  dependency('gio-2.0', version: gio_req)
  dependency('gtk4', version: gtk4_req)
  dependency('lcms2', version: lcms2_req)
endif

glycin_datadir = datadir / 'glycin-loaders' / compat_version

meson.add_devenv({'XDG_DATA_HOME': datadir})

meson.add_dist_script('../build-aux/dist-vendor.sh', meson.project_version())

i18n = import('i18n')

loader_libexecdir = (
  get_option('prefix') / get_option('libexecdir') / 'glycin-loaders' / compat_version
)

test_skip_ext = ''

foreach ext : get_option('test_skip_ext')
  if test_skip_ext != ''
    test_skip_ext += ','
  endif
  test_skip_ext += ext
endforeach

manifest = meson.project_source_root() / 'Cargo.toml'

cargo_options = [
  '--profile', cargo_profile,
  '--manifest-path', manifest,
]

git_version = ''

if cargo_profile != 'release'
  git = find_program('git', required: false)

  if git.found()
    git_version = run_command(
      'git',
      '-C', meson.project_source_root(),
      'rev-parse',
      '--short', 'HEAD',
      check: false,
    ).stdout().strip()
  endif
endif

cargo_env_module = cargo_env
# Gettext translation
cargo_env_module.set('LOCALEDIR', get_option('prefix') / get_option('localedir'))
# Git commit for debugging
cargo_env_module.set('GIT_DESCRIBE', git_version)

test_args = []
foreach loader : get_option('loaders')
  test_args += ['-p', loader]

  cargo_build = custom_target(
    loader,
    build_by_default: true,
    build_always_stale: true,
    output: loader + '-bin',
    console: true,
    env: cargo_env_module,
    command: [
      cargo_bin,
      'build',
      '--target-dir', cargo_target_dir,
      cargo_options,
      '--package', loader,
    ],
  )

  custom_target(
    loader + '-cp-binary',
    depends: cargo_build,
    build_by_default: true,
    build_always_stale: true,
    install: true,
    install_dir: loader_libexecdir,
    output: loader,
    command: [
      'cp',
      cargo_target_dir / rust_target / loader,
      '@OUTPUT@',
    ],
  )

  # Install .conf file
  i18n.merge_file(
    type: 'desktop',
    input: configure_file(
      input: loader / loader + '.conf',
      output: 'tmp-' + loader,
      configuration: {'EXEC': loader_libexecdir / loader},
    ),
    output: loader + '.conf',
    po_dir: '..' / 'po',
    install: true,
    install_dir: glycin_datadir / 'conf.d',
  )

  # Install .thumbnailer file
  conf = configuration_data()
  conf.set('BINDIR', bindir)

  configure_file(
    input: loader / loader + '.thumbnailer.in',
    output: loader + '.thumbnailer',
    configuration: conf,
    install: true,
    install_dir: datadir / 'thumbnailers',
  )
endforeach

if get_option('tests')
  destdir = meson.project_build_root() / '_test-install'

  if not get_option('test_skip_install')
    test(
      'install-integration-test',
      find_program('meson'),
      args: [
        'install',
        '-C', meson.project_build_root(),
        '--destdir', destdir,
      ],
      is_parallel: false,
      timeout: 1800,
    )

    # Rewrite config to point into destdir
    setup_integration_test = find_program(
      meson.project_source_root() / 'build-aux/setup-integration-test.py',
    )
    foreach loader : get_option('loaders')
      test(
        'setup-integration-test-' + loader,
        setup_integration_test,
        env: {
          'DESTDIR': destdir,
          'CONFIG_FILE': glycin_datadir / 'conf.d' / loader + '.conf',
        },
        is_parallel: false,
      )
    endforeach
  endif

  env = {
    'GLYCIN_TEST_SKIP_EXT': test_skip_ext,
    'RUST_LOG': get_option('test_log_level'),
  }

  if not get_option('test_skip_install')
    # Use string concatenation to combine two absolute paths
    env += {'GLYCIN_DATA_DIR': destdir + datadir}
  endif

  test(
    'cargo-test',
    cargo_bin,
    args: [
      'test',
      '-p', 'tests',
      test_args,
      # Not using a separate target dir causes rebuilds for the loaders
      '--target-dir', cargo_target_dir / 'tests',
      cargo_options,
      '--',
      '--test-threads=1',
      '--nocapture',
    ],
    env: env,
    is_parallel: false,
    timeout: 1800,
  )

  if get_option('test_tokio')
    test(
      'cargo-test-tokio',
      cargo_bin,
      args: [
        'test',
        '-p', 'tests',
        '--no-default-features',
        '--features', 'tokio',
        # Not using a separate target dir causes rebuilds for the loaders
        '--target-dir', cargo_target_dir / 'tests',
        cargo_options,
        '--',
        '--test-threads=1',
        '--nocapture',
      ],
      env: env,
      is_parallel: false,
      timeout: 1800,
    )
  endif
endif
