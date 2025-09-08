if not get_option('libglycin') and not get_option('libglycin-gtk4')
    subdir_done()
endif

libglycin_deps = [
    dependency('fontconfig', version: fontconfig_req),
    dependency('gio-2.0', version: gio_req),
    dependency('libseccomp', version: seccomp_req),
    dependency('lcms2', version: lcms2_req),
]

pkg = import('pkgconfig')
gnome = import('gnome')
manifest = meson.project_source_root() / 'Cargo.toml'

cargo_options = [
    '--target-dir', cargo_target_dir,
    '--profile', cargo_profile,
    '--manifest-path', manifest,
]

cargo_env_module = cargo_env

packages = []

if get_option('libglycin')
    packages = [
        {
            'suffix': '',
            'namespace_suffix': '',
            'symbol_prefix_suffix': '',
            'extra_deps': [],
        },
    ]
endif

if get_option('libglycin-gtk4')
    extra_deps = [dependency('gtk4', version: gtk4_req)]

    # Not building libglycin, need to link already installed version
    if not get_option('libglycin')
        extra_deps += dependency('glycin-2')
        global_libglycin = []
    endif

    packages += {
        'suffix': '-gtk4',
        'namespace_suffix': 'Gtk4',
        'symbol_prefix_suffix': '_gtk',
        'extra_deps': extra_deps,
    }
endif

foreach package : packages
    suffix = package['suffix']
    namespace_suffix = package['namespace_suffix']
    symbol_prefix_suffix = package['symbol_prefix_suffix']

    deps = libglycin_deps + package['extra_deps']
    link_with = []
    extra_girs = []
    extra_gir_deps = []
    if suffix != ''
        link_with += global_libglycin
        if introspection
            if get_option('libglycin')
                extra_gir_deps += declare_dependency(sources: global_libglycin_gir)
            endif
            extra_girs += ['Gly-2', 'Gdk-4.0']
        endif
    endif

    cargo_output = cargo_target_dir / rust_target / f'libglycin@suffix@.so'.replace('-', '_')

    # Build library with Rust
    libglycin = custom_target(
        f'libglycin@suffix@-cargo',
        build_always_stale: true,
        build_by_default: true,
        depends: link_with,
        output: f'Meson-does-not-support-sub-dir-output@suffix@',
        console: true,
        env: cargo_env_module,
        command: [cargo_bin, 'build', cargo_options, '-p', f'libglycin@suffix@'],
    )

    # Make the build result available to meson. We cannot use the "output"
    # key in the previous target for this since meson prohibits output in subdirs
    libglycin_copy_library_for_deps = custom_target(
        f'libglycin@suffix@-copy-library2',
        depends: libglycin,
        build_by_default: true,
        build_always_stale: true,
        output: f'tmp@suffix@.so',
        command: [
            'cp',
            '-a', cargo_output,
            meson.current_build_dir() / f'tmp@suffix@.so',
        ],
    )

    # We need a shim "lib" object to pass to "generate_gir" later
    libglycin_shim = library(
        f'glycin@suffix@-2',
        f'include/glycin@suffix@.h',
        soversion: '0',
        link_with: link_with,
        include_directories: 'include/',
        dependencies: deps,
        # Mark dirty for "generate_gir" if the header or the rust generated .so changes
        link_depends: [
            # For unknown reasons the header triggers repeated gir regenerates until something else changes
            f'include/glycin@suffix@.h',
            libglycin_copy_library_for_deps,
        ],
        install: true,
    )

    if suffix == ''
        global_libglycin = libglycin_shim
    endif

    x = libglycin_shim

    # Copy the .so file to its correct location such that "generate_gir" can use it.
    # We have to do this after the shim library target since it generates
    # a useless/empty library.
    libglycin_copy_library = custom_target(
        f'libglycin@suffix@-copy-library',
        depends: libglycin_shim,
        build_always_stale: true,
        build_by_default: true,
        output: f'would-overlap-with-library()@suffix@',
        command: [
            'cp',
            '-a', cargo_output,
            libglycin_shim.full_path(),
        ],
    )

    install_headers(f'include/glycin@suffix@.h', subdir: f'glycin@suffix@-2')

    # Generate a dependency such that the following targets only happen
    # after the .so file has been copied to the correct place
    libglycin_copy_library_as_dep = declare_dependency(
        sources: libglycin_copy_library,
    )

    if suffix == ''
        version = run_command(crate_version_bin, f'libglycin@suffix@', 'gnome', check: true).stdout()
    else
        version = run_command(
            crate_version_bin,
            f'libglycin/libglycin@suffix@',
            'gnome',
            check: true,
        ).stdout()
    endif

    # Generate .pc file
    pkg.generate(
        name: f'glycin@suffix@-2',
        description: 'Glycin: Sandboxed and extendable image decoding',
        libraries: deps
        + libglycin_shim
        + libglycin_copy_library_as_dep
        + global_libglycin,
        subdirs: f'glycin@suffix@-2',
        version: version,
    )

    if introspection
        # Generate .gir and .typelib
        libglycin_gir = gnome.generate_gir(
            libglycin_shim,
            dependencies: extra_gir_deps + libglycin_copy_library_as_dep,
            sources: [f'include/glycin@suffix@.h', libglycin_copy_library],
            nsversion: '2',
            namespace: f'Gly@namespace_suffix@',
            symbol_prefix: f'gly@symbol_prefix_suffix@',
            includes: ['GLib-2.0', 'GObject-2.0', 'Gio-2.0'] + extra_girs,
            include_directories: meson.current_build_dir(),
            link_with: link_with,
            # Listed as "C headers" in docs
            header: f'glycin@suffix@.h',
            # Listed as "pkg-config files" in docs
            export_packages: f'glycin@suffix@-2',
            install: true,
        )
        if suffix == ''
            global_libglycin_gir = libglycin_gir[0]
        endif
    endif

    if get_option('vapi')
        # Generate .vapi (Vala)
        gnome.generate_vapi(
            f'glycin@suffix@-2',
            sources: libglycin_gir[0],
            packages: ['glib-2.0', 'gobject-2.0', 'gio-2.0', 'gtk4'],
            gir_dirs: meson.current_build_dir(),
            install: true,
        )
    endif

    if get_option('capi_docs')
        gidocgen = find_program('gi-docgen')

        # Generate HTML documentation
        custom_target(
            f'libglycin@suffix@-doc',
            # Generate from .gir file
            depends: libglycin_gir,
            input: [f'doc/libglycin@suffix@.toml'] + libglycin_gir[0],
            output: f'libglycin@suffix@-2',
            command: [
                gidocgen,
                'generate',
                '--config=@INPUT0@',
                '--output-dir=@OUTPUT@',
                '--quiet',
                '--no-namespace-dir',
                '--add-include-path', meson.current_build_dir(),
                '--content-dir', meson.current_source_dir() / 'doc',
                '@INPUT1@',
            ],
            build_by_default: true,
            install: true,
            install_dir: datadir / 'doc',
        )
    endif

endforeach

if get_option('python_tests')
    test('libglycin-python3', find_program('../tests/libglycin.py'))
endif
