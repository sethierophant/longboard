**Longboard** is an [imageboard][1] engine written in Rust.

![Screenshot of a page](/../screenshots/screenshot.png?raw=True)

## Features

- Image and video uploads (with thumbnails)
- Markdown-style post formatting
- Web interface for moderation
- ... and much more!

### Goals

**Configurable, well-documented, and easy to install**. The default install
comes with manual pages, a well-commented config file, and a .service file for
starting with systemd.

**Spam-resistant**. Longboard comes with a web interface for moderating posts
and support for using DNSBLs.

**Performant**. Because most usages of longboard will probably be on low-cost
or donation-run servers, performance is a major goal.

[siege][3] metrics for a 1CPU/1GB VPS:

    siege -f urls.txt -b -t 10m

    {       "transactions":                       27582,
            "availability":                       99.89,
            "elapsed_time":                      599.77,
            "data_transferred":                  608.85,
            "response_time":                       0.44,
            "transaction_rate":                   45.99,
            "throughput":                          1.02,
            "concurrency":                        20.36,
            "successful_transactions":            27582,
            "failed_transactions":                   30,
            "longest_transaction":                59.03,
            "shortest_transaction":                0.10
    }

## Installation

Build Dependencies:

- Nightly Rust (rustc and cargo)
- GNU Make
- GNU M4
- PostgresQL (libpq)

You should also install whatever your distribution's `build-essential` or
`base-devel` package is if you're building from source.

Run Dependencies:

- PostgreSQL
- FFmpeg (for video thumbnails)

### From Source

Clone the repository:

    git clone https://github.com/sethierophant/longboard
    cd longboard

And run make:

    make
    sudo make install

Some of the Makefile options:

- **DESTDIR** Location to install to, set to `/` by default.
- **prefix** Prefix to install to, set to `/usr/local` by default.
- **servicedir** If this option is set, the directory to install systemd
  service files to. Set to `/usr/local/lib/systemd/system` by default.
- **mandir** If this option is set, the directory to install man pages to. Set
  to `/usr/local/share/man/` by default.

*Note that you should set the same make options while running `make` and `make
install`.*

For more installation options, see the [Makefile](/Makefile).

### From a Package

You can use the provided [PKGBUILD](/contrib/PKGBUILD) file to build on Arch
Linux.

Pull requests that add packages and/or build scripts for building packages are
welcome! In particular, I think it would be nice if we could include .deb files
with our releases, since so many people use Debian.

## Setup

Set up the database:

    sudo -u postgres -- psql -c 'CREATE ROLE longboard LOGIN'
    sudo -u postgres -- psql -c 'CREATE DATABASE longboard WITH OWNER longboard'

Longboard will automatically create the tables that it needs.

Add an administrator, so you can use the web moderator interface:

    longctl add-staff -r administrator -u NAME -p PASSWORD

Start the system service:

    systemctl start longboard

And you should have a longboard instance running on localhost:80!

## Configuring

See longboard(8) for usage instructions and longboard(5) for configuration
instructions.

Longboard is also distributed with **longctl**, a command-line tool for
configuring and moderating your imageboard. See longctl(1) for details.

## Contributing

All contributions are welcome! If there's any features you want to see, please
open an issue for a feature request. Also, bug reports are very much
appreciated.

## License

[GNU Affero General Public License v3.0][2]

The fonts in [res/fonts](/res/fonts) are distributed under their own licenses,
the Apache License Version 2.0 for the Roboto fonts and the Open Font License
for Oswald. These fonts were obtained from [Google Fonts][4].

[1]: https://en.wikipedia.org/wiki/Imageboard
[2]: https://www.gnu.org/licenses/agpl-3.0.en.html
[3]: https://www.joedog.org/siege-home/
[4]: https://fonts.google.com
