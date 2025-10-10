# Maintainer: Grenish Rai <mrcoder2033d@gmail.com>
pkgname=clipboard-manager
pkgver=0.1.0
pkgrel=1
pkgdesc="A simple clipboard manager written in Rust"
arch=('x86_64')
url="https://github.com/Grenish/clipboard-manager"
license=('MIT')
depends=('gcc-libs' 'libxcb' 'libx11')
makedepends=('cargo' 'rust' 'git')
source=("git+${url}.git")
sha256sums=('SKIP')

build() {
  cd "${srcdir}/${pkgname}"
  cargo build --release
}

package() {
  cd "${srcdir}/${pkgname}"
  install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
  install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
  install -Dm644 README.md "${pkgdir}/usr/share/doc/${pkgname}/README.md"
}
