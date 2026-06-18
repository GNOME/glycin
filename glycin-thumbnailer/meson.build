if not get_option('glycin-thumbnailer')
    subdir_done()
endif

if not get_option('libglycin')
    # Use external libglycin if not configured to build it
    dependency('glycin-2')
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
        cargo_target_args,
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
        cargo_artifact_dir / rust_target / 'glycin-thumbnailer',
        '@OUTPUT@',
    ],
)
