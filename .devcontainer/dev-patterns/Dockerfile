FROM mcr.microsoft.com/devcontainers/rust:2-1-trixie

# Install just
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Install nvm + Node LTS Krypton (v24)
ENV NVM_DIR=/usr/local/share/nvm
RUN mkdir -p "$NVM_DIR" \
    && curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash \
    && . "$NVM_DIR/nvm.sh" \
    && nvm install lts/krypton \
    && nvm alias default lts/krypton \
    && nvm use default

# Install pnpm and create a stable symlink for node binaries
RUN . "$NVM_DIR/nvm.sh" \
    && corepack enable pnpm \
    && ln -s "$(dirname "$(which node)")" "$NVM_DIR/current"

ENV PATH="$NVM_DIR/current:$PATH"
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0

# Add wasm target and install build tools
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli \
    && apt-get update && apt-get install -y --no-install-recommends binaryen && rm -rf /var/lib/apt/lists/*
