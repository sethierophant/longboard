**Longboard** is an [imageboard][1] engine written in Rust.

![Screenshot of a page](/../screenshots/screenshot.png?raw=True)

## Goals

Longboard aims to be **configurable**, **performant**, and **spam-resistant**.

## Installation

### From a Package

TODO

### From Source

Clone the repository:

    git clone https://github.com/sethierophant/longboard
    cd longboard

Create the system user for longboard:

    sudo useradd -r longboard

And run make:

    make
    sudo make install

For more installation options, see the Makefile.

## Usage

See longboard(1) for usage instructions and longboard(5) for configuration
instructions. Also, see longctl(1) for some setup and configuration actions.

## License

[GNU Affero General Public License v3.0][2]

[1]: https://en.wikipedia.org/wiki/Imageboard
[2]: https://www.gnu.org/licenses/agpl-3.0.en.html
