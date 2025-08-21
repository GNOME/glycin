# Contributing to Glycin

## Running tests

```sh
$ meson setup -Dbuildtype=debug builddir
$ meson test -vC builddir
```

## Finding blocked syscalls

Auditd is enabled by default on Fedora. On Debian you need to

```sh
sudo apt install auditd
systemctl start auditd.service
```

After that you have to execute the failing operation again while running the program with `GLYCIN_SECCOMP_DEFAULT_ACTION=KILL_PROCESS`.

On Fedora the logs should be accessible via

```sh
journalctl -b | grep SECCOMP
```

On Debian via

```sh
sudo grep SECCOMP /var/log/audit/audit.log
```

## Using the locally built loaders

While `meson test` will ensure to run against the locally built loaders, for other commands, this is not the case. As a first step, you need to install the loaders into a local directory:

```sh
$ meson setup -Dbuildtype=debug --prefix=$(pwd)/install builddir
$ meson install -C builddir
```

You can set `GLYCIN_DATA_DIR=$(pwd)/install/share` to force glycin to use the locally built loaders.

Usually, the better option is to use `meson devenv` which will also set other relevant variables:

```sh
$ meson devenv -C builddir -w .
# Test if it works
$ ./tests/libglycin.py
```

## Useful Commands

Glycin comes with a few tools in `glycin-dev-tools/` that can be helpful for development.

```sh
$ meson devenv -C builddir -w .
$ cargo r --bin glycin-image-info image.png
```

Use ImageMagic to get Exif information.

```sh
$ identify -format '%[EXIF:*]' <image>
```

Increase memory usage (this can crash your PC)

```sh
$ sudo swapoff -a
$ stress-ng --vm-bytes 20G --vm-keep -m 1
```

## Test D-Bus API stability

The following test will ensure that the lastest API documented in `docs/` hasn't changed.

```
$ cargo test -p tests -- dbus_api_stability --include-ignored
```

## Resources

- [xdg/shared-mime-info](https://gitlab.freedesktop.org/xdg/shared-mime-info/-/blob/master/data/freedesktop.org.xml.in)
