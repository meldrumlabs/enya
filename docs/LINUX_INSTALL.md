# Building Enya on Linux (Ubuntu/Debian)

## Install system dependencies

```bash
sudo apt update && sudo apt install -y \
  build-essential pkg-config mold \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libssl-dev \
  libvulkan-dev mesa-vulkan-drivers
```

## Install Rust and Just

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install just
```

## Build and run

```bash
git clone https://github.com/meldrumlabs/enya.git
cd enya
just install
just run
```
