# Runner Linux do smoke: toolchain Rust 1.98.1 + Xvfb + xdotool + ImageMagick + cliente PostgreSQL 18.
#
# O fonte do repositório é montado em /src (somente leitura) na execução; nada do workspace é copiado
# para a imagem. O alvo do cargo fica em /tmp/target (volume), então o `target/` do Windows não é
# tocado.
FROM postgres:18.6@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates \
        libdbus-1-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
        libgl1-mesa-dev libwayland-dev libxkbcommon-dev libssl-dev \
        cmake clang libclang-dev openssl \
        xvfb xdotool imagemagick x11-utils gnome-keyring libsecret-tools dbus-x11 \
        libdbus-1-3 libgl1 libglx-mesa0 libgl1-mesa-dri libegl1 libgles2 \
        libxkbcommon-x11-0 fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain 1.98.1 --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /src
ENTRYPOINT ["/bin/bash", "/src/smoke/run.sh"]
