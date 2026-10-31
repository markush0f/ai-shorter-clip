#!/usr/bin/env bash
# Development runtime only. No sudo, system installation, or unverified packages.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != Linux || ! -f /etc/fedora-release ]]; then
  printf 'Este instalador local es para Fedora. En Windows/macOS instala VLC 3 de 64 bits; consulta README.md.\n' >&2
  exit 1
fi
source /etc/os-release
arch="$(uname -m)"
if [[ "$arch" != x86_64 ]]; then printf 'El runtime local validado requiere Fedora x86_64.\n' >&2; exit 1; fi
for tool in dnf rpmkeys rpm2cpio cpio; do command -v "$tool" >/dev/null || { printf 'Falta %s\n' "$tool" >&2; exit 1; }; done
mkdir -p "$root/.runtime/rpmdb"
keys="$root/.runtime/rpmdb"
for key in "/etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-${VERSION_ID}-primary" "/etc/pki/rpm-gpg/RPM-GPG-KEY-rpmfusion-free-fedora-${VERSION_ID}"; do
  [[ -f "$key" ]] || { printf 'Falta la clave de un repositorio configurado: %s. Consulta README.md.\n' "$key" >&2; exit 1; }
  rpmkeys --dbpath "$keys" --import "$key"
done
names=(vlc-libs vlc-plugins-base vlc-plugins-video-out vlc-plugin-ffmpeg vlc-plugin-pulseaudio libavcodec-freeworld)
mapfile -t packages < <(dnf -q repoquery --available --latest-limit=1 --arch="$arch" --qf '%{name}-%{version}-%{release}.%{arch}\n' "${names[@]}")
for name in "${names[@]}"; do
  printf '%s\n' "${packages[@]}" | grep -q "^${name}-[0-9]" || { printf 'No se encontró %s. Necesitas los repositorios Fedora y RPM Fusion Free configurados.\n' "$name" >&2; exit 1; }
done
download="$(mktemp -d "$root/.runtime/vlc-download.XXXXXX")"
stage="$(mktemp -d "$root/.runtime/vlc-stage.XXXXXX")"
cleanup() { [[ ! -d "$download" ]] || rm -rf -- "$download"; [[ ! -d "$stage" ]] || rm -rf -- "$stage"; }
trap cleanup EXIT
dnf download --resolve --destdir "$download" "${packages[@]}"
shopt -s nullglob
rpms=("$download"/*.rpm)
(( ${#rpms[@]} > 0 )) || { printf 'No se descargó ningún paquete.\n' >&2; exit 1; }
for package in "${rpms[@]}"; do
  rpmkeys --dbpath "$keys" --checksig "$package" || exit 1
  (cd "$stage" && rpm2cpio "$package" | cpio -idmu --quiet --no-absolute-filenames)
  rpm --dbpath "$keys" -qp --qf '%{name} %{version}-%{release} %{arch} | %{license}\n' "$package" >> "$stage/PACKAGES-AND-LICENSES.txt"
done
[[ -f "$stage/usr/lib64/libvlc.so.5" && -d "$stage/usr/lib64/vlc/plugins" && -f "$stage/usr/lib64/ffmpeg/libavcodec.so.62" ]] || {
  printf 'El runtime no tiene la estructura libVLC 3/FFmpeg esperada. No se ha sustituido el runtime anterior.\n' >&2; exit 1;
}
if [[ -e "$root/.runtime/vlc" ]]; then mv "$root/.runtime/vlc" "$root/.runtime/vlc-backup-$(date +%s)"; fi
mv "$stage" "$root/.runtime/vlc"
printf '\nRuntime local preparado en .runtime/vlc. Arranca con npm run tauri dev, sin variables adicionales.\n'
printf 'No se han instalado ni sustituido paquetes del sistema. Conserva las licencias antes de distribuir binarios.\n'
