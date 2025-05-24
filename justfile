build EXAMPLE:
    cargo build --release --bin {{EXAMPLE}} && \
    cargo objcopy --release --bin {{EXAMPLE}} -- -S -O binary {{EXAMPLE}}.vb
assembly EXAMPLE:
    cargo rustc --release --manifest-path examples/{{EXAMPLE}}/Cargo.toml -- --emit asm --emit llvm-ir
    cargo objdump --release --bin {{EXAMPLE}} -- --disassemble >{{EXAMPLE}}.s