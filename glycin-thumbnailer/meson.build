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

thumbnailer_exeext = is_windows_target ? '.exe' : ''

custom_target(
    'glycin-thumbnailer-cp-binary',
    depends: cargo_build,
    build_by_default: true,
    build_always_stale: true,
    install: true,
    install_dir: bindir,
    output: f'glycin-thumbnailer@thumbnailer_exeext@',
    command: [copy_cmd.full_path()] + [
        cargo_artifact_dir / rust_target / f'glycin-thumbnailer@thumbnailer_exeext@',
        '@OUTPUT@',
    ],
)
