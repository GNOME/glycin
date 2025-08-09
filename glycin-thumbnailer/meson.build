if not get_option('glycin-thumbnailer')
    subdir_done()
endif

cargo_build = custom_target(
    'glycin-thumbnailer',
    build_by_default: true,
    build_always_stale: true,
    output: 'glycin-thumbnailer-bin',
    console: true,
    env: cargo_env,
    command: [
        cargo_bin,
        'build',
        '--target-dir', cargo_target_dir,
        '--profile', cargo_profile,
        '--manifest-path', cargo_manifest,
        '--package=glycin-thumbnailer',
    ],
)

custom_target(
    'glycin-thumbnailer-cp-binary',
    depends: cargo_build,
    build_by_default: true,
    build_always_stale: true,
    install: true,
    install_dir: bindir,
    output: 'glycin-thumbnailer',
    command: [
        'cp',
        cargo_target_dir / rust_target / 'glycin-thumbnailer',
        '@OUTPUT@',
    ],
)
