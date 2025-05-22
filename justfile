build EXAMPLE:
    cargo build --release --bin {{EXAMPLE}} && \
    cargo objcopy --release --bin {{EXAMPLE}} -- -S -O binary {{EXAMPLE}}.vb