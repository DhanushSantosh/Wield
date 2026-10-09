FROM ubuntu:22.04@sha256:5ec03bb3441e8b0bf3b4f9cd4629a1ae763010dc3035bb8da3ae6cf026486401

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl wget file xz-utils build-essential pkg-config \
    libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev libgtk-layer-shell-dev \
    libgdk-pixbuf-2.0-dev gdk-pixbuf2.0-bin xdg-utils

ARG NODE_VERSION=24.21.0
ARG NODE_SHA256=fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6
RUN curl --fail --location --retry 5 \
      "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
      --output /tmp/node.tar.xz \
    && echo "${NODE_SHA256}  /tmp/node.tar.xz" | sha256sum --check \
    && mkdir -p /opt/node \
    && tar --extract --file /tmp/node.tar.xz --strip-components=1 --directory /opt/node

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH=/opt/node/bin:/opt/cargo/bin:${PATH}
RUN curl --proto '=https' --tlsv1.2 --fail --location --retry 5 \
      https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
    && rustup component add clippy rustfmt \
    && cargo install cargo-audit --version 0.22.2 --locked \
    && chmod -R a+rX /opt/rustup /opt/cargo

WORKDIR /workspace
