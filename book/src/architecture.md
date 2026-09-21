# Core Architecture

The core architecture of `u64se-seu-solana` is designed around mathematical determinism and hardware mechanical sympathy. Rather than managing complex nested structs, every semantic element of the state machine is encoded into a 64-bit integer word.

```
63        48 47          32 31        24 23        16 15         8 7          0
+-----------+--------------+------------+------------+------------+------------+
| TOPOLOGY  | EPOCH_SLOT   | TARGET     | CURRENT    | STATUS     | ROLE_MASK  |
| FLAGS     | ANCHOR       | NODE       | NODE       | FLAGS      |            |
| (16 bits) | (16 bits)    | (8 bits)   | (8 bits)   | (8 bits)   | (8 bits)   |
+-----------+--------------+------------+------------+------------+------------+
```

---

## The Three Architectural Tenets

### 1. Zero-Heap Allocation (`no_allocator!`)
In standard Solana programs, the default allocator manages a 32 KB bump heap. In `u64se-seu-solana`, the heap allocator is disabled at compile time via Pinocchio's `no_allocator!()` macro. All computations occur strictly within CPU registers (`r1`..`r5`) and on the 4 KB SBF call stack.

### 2. Branchless Combinatorial Evaluation
Conditional jumps (`JEQ`, `JNE`, `JGT`) introduce branch speculation penalties and timing variability. The transition evaluation pipeline in `u64se-seu-solana` computes all safety properties simultaneously using arithmetic and bitwise boolean masks, accumulating violations into a single 8-bit `FaultMask`.

### 3. Dual-Plane Validation (Host vs On-Chain)
The exact same Rust evaluation logic compiles to:
1. **Off-Chain / Host**: Evaluated in native x86_64 / aarch64 assembly in **~2.4 nanoseconds** (419M ops/sec), allowing autonomous agents to filter out invalid intentions locally before incurring any RPC cost or gas.
2. **On-Chain SBF**: Executed within the Solana VM in **~8 CU** of raw ALU instructions, guaranteeing identical validation semantics on both sides of the wire.
