#! /bin/bash

# Build the project
cargo build
cargo install --path . --features=mstsc-rs

# Create a symlink to the mstsc-rs binary
ln -s target/debug/mstsc-rs mstsc-rs
