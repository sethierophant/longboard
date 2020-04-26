**Longboard** is an [imageboard][1] engine written in Rust.

![Screenshot of a page](/../screenshots/screenshot.png?raw=True)

## Goals

Longboard aims to be:
    - Configurable, well-documented, and easy to install.
    - Spam-resistant.
    - Performant.

## Installation

### From Source

Clone the repository:

    git clone https://github.com/sethierophant/longboard
    cd longboard

Create the system user for longboard:

    sudo useradd -r longboard

And run make:

    make
    sudo make install

For more installation options, see the [Makefile](/Makefile).

### From a Package

You can use the provided [PKGBUILD](/contrib/PKGBUILD) file to build on Arch
Linux.

Pull requests that add packages and/or build scripts for building packages are
welcome! In particular, I think it would be nice if we could include .deb files
with our releases, since so many people use Debian.

## Usage

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

[1]: https://en.wikipedia.org/wiki/Imageboard
[2]: https://www.gnu.org/licenses/agpl-3.0.en.html
