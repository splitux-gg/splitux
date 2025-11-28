# Maintainer: gabrielgad
pkgname=splitux
pkgver=0.8.5
pkgrel=1
pkgdesc="A split-screen game launcher for Linux/SteamOS"
arch=('x86_64')
url="https://github.com/gabrielgad/splitux"
license=('GPL-3.0-or-later')
depends=(
    'fuse-overlayfs'
    'bubblewrap'
    'gamescope'
    'sdl2'
)
optdepends=(
    'umu-launcher: Windows game support via Proton'
    'plasma-desktop: KWin window management support'
)
makedepends=(
    'rust'
    'cargo'
    'meson'
    'ninja'
)
source=("$pkgname-$pkgver.tar.gz::https://github.com/gabrielgad/$pkgname/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$srcdir/$pkgname-$pkgver"
    cargo build --release

    # Build keyboard/mouse gamescope fork
    cd deps/gamescope
    git submodule update --init
    meson setup build/
    ninja -C build/
}

package() {
    cd "$srcdir/$pkgname-$pkgver"

    # Install binary
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

    # Install keyboard/mouse gamescope
    install -Dm755 "deps/gamescope/build/src/gamescope" "$pkgdir/usr/bin/gamescope-kbm"

    # Install resources
    install -Dm644 "res/splitscreen_kwin.js" "$pkgdir/usr/share/$pkgname/splitscreen_kwin.js"
    install -Dm644 "res/splitscreen_kwin_vertical.js" "$pkgdir/usr/share/$pkgname/splitscreen_kwin_vertical.js"

    # Install license
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 "COPYING.md" "$pkgdir/usr/share/licenses/$pkgname/COPYING.md"
}
