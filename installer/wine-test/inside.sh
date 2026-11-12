#!/bin/sh
# Roda dentro do container: compila o smoke.iss com o ISCC sob Wine e instala o setup.
set -eu

xvfb-run -a wine 'C:\InnoSetup\ISCC.exe' 'Z:\test\smoke.iss'
xvfb-run -a wine 'Z:\out\wine-test-setup.exe' /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-
test -f "$WINEPREFIX/drive_c/WineTest/dummy.txt"
echo "wine-test: ISCC + setup sob Wine OK"
