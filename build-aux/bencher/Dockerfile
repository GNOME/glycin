# Build with
#    podman build -f build-aux/bencher/Dockerfile .

# Fix a version to make benchmarks comparable
FROM ubuntu:resolute-20260421 AS runtime-deps

RUN apt-get update

# Libraries
RUN apt-get install -y liblcms2-dev gettext libseccomp-dev libfontconfig-dev libglib2.0-dev

# Needed to compare with gdk-pixbuf
RUN apt-get install -y libgdk-pixbuf-2.0-dev

FROM runtime-deps AS build

# Build tools
RUN apt-get install -y meson pkg-config cargo

WORKDIR /glycin

# Copy sources
COPY . .

# Build image-rs loader
RUN meson setup -Dloaders=glycin-image-rs -Dtests=false -Dlibglycin=false -Dlibglycin-gtk4=false builddir
RUN meson install -C builddir

# Build benches and download images
# The specified benchmark does not exist so no benchark is run
RUN cargo bench -p benches only-load-images

RUN ls -lh target

RUN cp target/release/deps/$(ls -1 target/release/deps/|grep ^loader-[a-z0-9]*$) loader-bench

FROM runtime-deps

WORKDIR /glycin

COPY --from=build /glycin/benches/ benches/
COPY --from=build /glycin/tests/test-images benches/test-images/
COPY --from=build /glycin/loader-bench benches/loader-bench

COPY --from=build /usr/share/glycin-loaders/ /usr/share/glycin-loaders/
COPY --from=build /usr/libexec/glycin-loaders/ /usr/libexec/glycin-loaders/

ENV RUST_BACKTRACE=1

WORKDIR /glycin/benches

CMD ["./loader-bench", "--bench", "tiny.png"]
