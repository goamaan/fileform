#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
FILEFORM_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
FILEFORM_PDF_WORK="${FILEFORM_PDF_BUILD_ROOT:-$FILEFORM_ROOT/Artifacts/pdf-build-windows}"
FILEFORM_PDF_PACK="${FILEFORM_PDF_PACK_OUTPUT:-$FILEFORM_ROOT/Artifacts/PDFPack-Windows}"
[[ "${MSYSTEM:-}" == "UCRT64" ]] || { echo 'Run in MSYS2 UCRT64.' >&2; exit 1; }
FILEFORM_PDF_ARCH=x86_64
mkdir -p "$FILEFORM_PDF_WORK" "$FILEFORM_PDF_PACK/bin" "$FILEFORM_PDF_PACK/licenses" "$FILEFORM_PDF_PACK/sources"
cd "$FILEFORM_PDF_WORK"
fetch_source() {
    local filename="$1" source_url="$2" source_hash="$3"
    if [ ! -f "$filename" ]; then
        curl --fail --location --silent --show-error "$source_url" -o "$filename.download"
        mv "$filename.download" "$filename"
    fi
    printf '%s  %s\n' "$source_hash" "$filename" | sha256sum -c -
    if [ ! -d "${filename%.tar.gz}" ]; then tar -xf "$filename"; fi
}
fetch_source qpdf-12.4.1.tar.gz https://github.com/qpdf/qpdf/releases/download/v12.4.1/qpdf-12.4.1.tar.gz f045aa277be2356ff53a89a8622945958291177d2483afc20ede7c8a8cd3873c
fetch_source libjpeg-turbo-3.2.0.tar.gz https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/3.2.0/libjpeg-turbo-3.2.0.tar.gz 6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e
cmake -S libjpeg-turbo-3.2.0 -B jpeg-build -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_EXE_LINKER_FLAGS=-static \
    -DENABLE_SHARED=OFF -DENABLE_STATIC=ON -DWITH_TURBOJPEG=OFF -DWITH_TOOLS=OFF -DWITH_TESTS=OFF > jpeg-configure.log 2>&1 || { tail -n 60 jpeg-configure.log >&2; exit 1; }
cmake --build jpeg-build --target jpeg-static --parallel "${FILEFORM_BUILD_JOBS:-2}" > jpeg-build.log 2>&1 || { tail -n 60 jpeg-build.log >&2; exit 1; }
mkdir -p jpeg-include
cp libjpeg-turbo-3.2.0/src/jpeglib.h libjpeg-turbo-3.2.0/src/jmorecfg.h jpeg-build/jconfig.h jpeg-include/
cmake -S qpdf-12.4.1 -B qpdf-build -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_EXE_LINKER_FLAGS=-static \
    -DBUILD_SHARED_LIBS=OFF -DBUILD_STATIC_LIBS=ON -DBUILD_DOC=OFF \
    -DUSE_IMPLICIT_CRYPTO=OFF -DREQUIRE_CRYPTO_NATIVE=ON -DDEFAULT_CRYPTO=native \
    -DPKG_CONFIG_EXECUTABLE=/usr/bin/false -U 'pc_*' \
    -DZLIB_LIBRARY=/ucrt64/lib/libz.a -DZLIB_INCLUDE_DIR=/ucrt64/include \
    -DLIBJPEG_H_PATH="$FILEFORM_PDF_WORK/jpeg-include" \
    -DLIBJPEG_LIB_PATH="$FILEFORM_PDF_WORK/jpeg-build/libjpeg.a" > qpdf-configure.log 2>&1 || { tail -n 60 qpdf-configure.log >&2; exit 1; }
cmake --build qpdf-build --target qpdf --parallel "${FILEFORM_BUILD_JOBS:-2}" > qpdf-build.log 2>&1 || { tail -n 60 qpdf-build.log >&2; exit 1; }
cp qpdf-build/qpdf/qpdf.exe "$FILEFORM_PDF_PACK/bin/qpdf.exe"
cp qpdf-12.4.1/LICENSE.txt "$FILEFORM_PDF_PACK/licenses/qpdf-LICENSE.txt"
cp qpdf-12.4.1/NOTICE.md "$FILEFORM_PDF_PACK/licenses/qpdf-NOTICE.md"
cp qpdf-12.4.1/Artistic-2.0 "$FILEFORM_PDF_PACK/licenses/qtest-Artistic-2.0.txt"
cp libjpeg-turbo-3.2.0/LICENSE.md "$FILEFORM_PDF_PACK/licenses/libjpeg-turbo-LICENSE.md"
cp libjpeg-turbo-3.2.0/README.ijg "$FILEFORM_PDF_PACK/licenses/libjpeg-turbo-README.ijg"
printf '%s\n' 'This software is based in part on the work of the Independent JPEG Group.' > "$FILEFORM_PDF_PACK/licenses/IJG-attribution.txt"
cp qpdf-12.4.1.tar.gz libjpeg-turbo-3.2.0.tar.gz "$FILEFORM_PDF_PACK/sources/"
cp "$FILEFORM_ROOT/crates/fileform-engine/tools/build-pdf-windows.sh" "$FILEFORM_PDF_PACK/sources/"
cp jpeg-build/CMakeCache.txt "$FILEFORM_PDF_PACK/jpeg-build-flags.txt"
cp qpdf-build/CMakeCache.txt "$FILEFORM_PDF_PACK/qpdf-build-flags.txt"
"$FILEFORM_PDF_PACK/bin/qpdf.exe" --version > "$FILEFORM_PDF_PACK/version.txt"
objdump -p "$FILEFORM_PDF_PACK/bin/qpdf.exe" | sed -n 's/.*DLL Name: //p' > "$FILEFORM_PDF_PACK/linked-libraries.txt"
python3 - "$FILEFORM_PDF_PACK" <<'CHECK'
import sys
from pathlib import Path
p=Path(sys.argv[1])
allowed={'kernel32.dll','advapi32.dll','bcrypt.dll','user32.dll','shell32.dll','ole32.dll','oleaut32.dll','msvcrt.dll','ucrtbase.dll','ws2_32.dll','ntdll.dll'}
dlls=[x.strip().lower() for x in (p/'linked-libraries.txt').read_text().splitlines()]
assert dlls and all(x in allowed or x.startswith('api-ms-win-') for x in dlls),dlls
CHECK
pacman -Qi > "$FILEFORM_PDF_PACK/toolchain-packages.txt"
FILEFORM_NOTICE_COUNT=0
while read -r FILEFORM_PACKAGE FILEFORM_LICENSE_PATH; do
    if [[ "$FILEFORM_LICENSE_PATH" == */share/licenses/* && -f "$FILEFORM_LICENSE_PATH" ]]; then
        FILEFORM_NOTICE="$FILEFORM_PDF_PACK/licenses/msys2/$FILEFORM_PACKAGE/${FILEFORM_LICENSE_PATH#*/share/licenses/}"
        mkdir -p "$(dirname "$FILEFORM_NOTICE")"
        cp "$FILEFORM_LICENSE_PATH" "$FILEFORM_NOTICE"
        FILEFORM_NOTICE_COUNT=$((FILEFORM_NOTICE_COUNT + 1))
    fi
done < <(pacman -Ql)
[[ "$FILEFORM_NOTICE_COUNT" -gt 0 ]] || { echo 'No dependency notices collected.' >&2; exit 1; }
gcc --version > "$FILEFORM_PDF_PACK/toolchain.txt"
pacman -Q >> "$FILEFORM_PDF_PACK/toolchain.txt"
FILEFORM_PDF_PACK_PATH="$FILEFORM_PDF_PACK" FILEFORM_PDF_ARCHITECTURE="$FILEFORM_PDF_ARCH" python3 - <<'PY'
import hashlib, json, os
from pathlib import Path
p = Path(os.environ['FILEFORM_PDF_PACK_PATH'])
components = [
    {'name':'qpdf', 'version':'12.4.1', 'license':'Apache-2.0',
     'sourceURL':'https://github.com/qpdf/qpdf/releases/download/v12.4.1/qpdf-12.4.1.tar.gz',
     'sourceSHA256':'f045aa277be2356ff53a89a8622945958291177d2483afc20ede7c8a8cd3873c'},
    {'name':'libjpeg-turbo', 'version':'3.2.0', 'license':'IJG AND BSD-3-Clause AND Zlib',
     'sourceURL':'https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/3.2.0/libjpeg-turbo-3.2.0.tar.gz',
     'sourceSHA256':'6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e'},
    {'name':'zlib', 'distribution':'MSYS2 UCRT64 static library', 'license':'Zlib'}]
manifest = {'schemaVersion':1, 'id':'app.fileform.pdf', 'version':'12.4.1-fileform.1',
    'architecture':os.environ['FILEFORM_PDF_ARCHITECTURE'], 'platform':'windows',
    'upstreamSignatureVerified':False, 'components':components,
    'executables':{'qpdf':hashlib.sha256((p/'bin/qpdf.exe').read_bytes()).hexdigest()}}
(p/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
PY
echo "Verified PDF pack built at $FILEFORM_PDF_PACK"
