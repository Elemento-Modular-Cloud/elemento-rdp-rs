#! /bin/bash

# Build the project
cargo clean
cargo build --release
cargo install --path . --features=mstsc-rs

# Create a symlink to the mstsc-rs binary
echo "Creating symlink to mstsc-rs binary"
rm -f mstsc-rs
ln -s target/release/mstsc-rs mstsc-rs
