# Pinocchio Runtime (4,720 B SBF)

`u64se-seu-solana` is built using the **Pinocchio** runtime framework—a lightweight, zero-dependency, `no_std` toolchain developed specifically for ultra-lean Solana on-chain development.

---

## Why Pinocchio Over Anchor / Solana Program SDK

The official `solana-program` crate pulls in a large dependency graph including `borsh`, `bincode`, `sha2`, and dynamic allocators. As a result, even an empty "Hello World" Anchor contract compiles to a binary exceeding **150 KB to 300 KB**.

Pinocchio strips away all unnecessary abstractions:
- **No Rust Standard Library (`#![no_std]`)**: Removes OS-level shims.
- **No Heap Allocator (`no_allocator!()`)**: Completely removes dynamic memory management.
- **Direct Syscall Bindings**: Interacts directly with the SBF virtual machine via raw memory offsets.

```rust
use pinocchio::{
    account::AccountView,
    default_panic_handler,
    no_allocator,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

// Enforces zero-heap at link time
no_allocator!();
default_panic_handler!();
```

---

## Binary Size Verification

Compiling with the Solana SBF toolchain and stripping debug symbols:

```bash
cargo build-sbf --release
llvm-strip --strip-all target/deploy/u64se_seu_solana.so
```

Yields an exact ELF binary size of:
$$\mathbf{4,720\text{ bytes}}$$

This tiny footprint ensures that deployment transactions require only 12KB of write data, lowering deployment rent costs from ~2.5 SOL (for Anchor) down to **under 0.05 SOL**.
