# Imagem base pinada pelo digest do manifesto (multi-arch): o harness não muda sem uma decisão.
FROM ubuntu:24.04@sha256:2198e4c65076e9cb0b3d695219df667c0e3b0072da64ca3c2648aca17dcecaf5

# Inno Setup 6.7.4 x86: mesma linha do 6.7.1 que o windows-latest traz no release.
# Para testar o 7.1.1 x86, passe INNO_URL=... E INNO_SHA256=<sha do asset> (veja o README; o
# 7.1.1 free anuncia "Non-commercial use only").
ARG INNO_URL=https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe
ARG INNO_SHA256=a549929319f9120716677c685535ca4671e4b937b5fdc87e2585f32d8eb30e8c

RUN dpkg --add-architecture i386 \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        wine wine32 xvfb curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# O instalador baixado roda como usuário comum: só o `apt-get` acima precisa de root. O runtime do
# harness continua root para os volumes montados (ver run.sh/real.sh).
RUN useradd --create-home wine

ENV HOME=/home/wine \
    WINEARCH=win32 \
    WINEPREFIX=/home/wine/.wine \
    WINEDEBUG=-all

USER wine

# Prefixo Wine e instalação silenciosa do compilador no C:\InnoSetup.
RUN xvfb-run -a wineboot -i || true
RUN curl -fsSL -o /tmp/innosetup.exe "$INNO_URL" \
    && echo "$INNO_SHA256  /tmp/innosetup.exe" | sha256sum -c - \
    && ls -l /tmp/innosetup.exe \
    && xvfb-run -a wine /tmp/innosetup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR=C:\\InnoSetup

USER root

# O prefixo foi criado pelo usuário `wine` no build; o runtime do harness roda como root (os
# volumes montados são do host), então o prefixo passa para root antes de sair da imagem. Os
# temporários do Wine/Xvfb do build também saem para o runtime root não encontrar dono `wine`.
RUN chown -R root:root /home/wine/.wine \
    && rm -rf /tmp/wine-* /tmp/innosetup.exe /tmp/.X*-lock

WORKDIR /test
