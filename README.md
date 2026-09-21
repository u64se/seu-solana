# u64se-seu-solana

[![CI](https://img.shields.io/github/actions/workflow/status/u64se/seu-solana/ci.yml?branch=main&style=flat-square&logo=github-actions&logoColor=white)](https://github.com/u64se/seu-solana/actions)
[![Colosseum Hackathon](https://img.shields.io/badge/Colosseum-Flagship__Submission-orange?style=flat-square)](https://seu-solana.rag.engineering)
[![Docs](https://img.shields.io/badge/docs-docs.u64se.rag.engineering-blue?style=flat-square)](https://docs.u64se.rag.engineering)
[![State Size](https://img.shields.io/badge/state__size-8__bytes__(u64)-007acc?style=flat-square)](#specification)
[![ALU Cost](https://img.shields.io/badge/ALU__cost-~8__CU-brightgreen?style=flat-square)](#specification)
[![Binary Size](https://img.shields.io/badge/SBF__size-4%2C720__bytes-blueviolet?style=flat-square)](#specification)
[![Devnet](https://img.shields.io/badge/Solana-Devnet__Live-14F195?style=flat-square&logo=solana&logoColor=black)](https://solana.fm/address/FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H?cluster=devnet-solana)

Branchless 64-bit State Execution Unit (SEU) for Solana SVM.  
**Superteam Ukraine · Colosseum Global Hackathon Flagship Submission** (Infrastructure & Developer Tooling).

- **Live Showcase**: [seu-solana.rag.engineering](https://seu-solana.rag.engineering)
- **SVM Workbench**: [u64se.rag.engineering](https://u64se.rag.engineering)
- **Documentation**: [docs.u64se.rag.engineering](https://docs.u64se.rag.engineering)
- **Live Devnet Program**: [`FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H`](https://solana.fm/address/FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H?cluster=devnet-solana)

```
+-------------------+-------------------+-------------------+-------------------+
| bits 63..48 (16b) | bits 47..32 (16b) | bits 31..16 (16b) | bits 15..0 (16b)  |
| topology flags    | slot window delta | fsm current/target| auth & status     |
+-------------------+-------------------+-------------------+-------------------+
```

## Table of Contents

- [Overview](#overview)
- [Status & Security Notice](#status--security-notice)
- [Quick Start](#quick-start)
- [Specification](#specification)
- [Register Layout](#register-layout)
- [Instruction ABI](#instruction-abi)
- [Formal Verification (Z3 / SMT-LIB2)](#formal-verification-z3--smt-lib2)
- [Verification & Testing](#verification--testing)
- [CU & Performance Profile](#cu--performance-profile)
- [Toolchain & SBF Build](#toolchain--sbf-build)
- [License](#license)

---

## Overview

`u64se-seu-solana` packs an entire deterministic finite-state machine (FSM) protocol into a single 64-bit CPU register word (`u64`). 

Engineered for high-assurance Solana SVM programs, it executes with **zero heap allocation** (`no_allocator!`) and **zero conditional branches** ($O(1)$ constant time) in the core ALU pipeline. This eliminates branch-predictor timing side-channels, removes memory deserialization overhead, and enables rigorous formal bit-vector verification.

---

## Status & Security Notice

> **v0.1.0 — Experimental Research Primitive**: This codebase is an experimental prototype exploring constant-time branchless FSM execution and formal bit-vector verification on Solana SVM. While mathematical invariants are formally proven using the Z3 SMT solver and validated via Mollusk SVM, this software has not yet undergone an independent third-party commercial security audit. It is provided for evaluation, benchmarking, and architectural research.

---

## Quick Start

Verify formal mathematical proofs, run unit and fuzzing tests, and benchmark onchain execution in under a minute:

```bash
# 1. Verify 8 formal mathematical invariants with Z3 (QF_BV)
z3 verify_invariants.smt2

# 2. Run host ALU unit tests and Proptest fuzzing
cargo test --release

# 3. Run Mollusk SVM onchain attack suite
cd svm-tests && cargo run --release --bin security_tests

# 4. Measure exact SBF Compute Units (CU) in Mollusk SVM
cd svm-tests && cargo run --release --bin measure_sbf_cu

# 5. Run host ALU throughput benchmark
cargo run --release --example benchmark
```

---

## Specification

- **Runtime**: **Pinocchio** (zero-copy, zero-dependency, `no_std`, `no_allocator!`)
- **State Size**: exactly 8 bytes (`u64` register word)
- **Branching**: 0 conditional branches in ALU evaluation ($O(1)$ constant time)
- **Heap Allocation**: 0 bytes (enforced at compile-time via `no_allocator!`)
- **Binary Size**: **4,720 bytes** (stripped ELF SBF)
- **Compute Units (CU)**:
  - **ALU Core**: **~8 CU** (pure register transition)
  - **End-to-End Onchain**: **< 1,900 CU** (includes signer auth, PDA derivation, Clock sysvar, and return data)
    - Genesis Activation: **1,759 CU**
    - State Transition: **1,864 CU**

---

## Register Layout

The entire contract state is bit-packed into a single 64-bit register word:

| Range | Field | Encoding | Description |
| :--- | :--- | :--- | :--- |
| `0..7` | Role Auth | `allowed_roles_mask` | 8-bit authorization bitfield (single-hot enforced) |
| `8..15` | Engine Status | `status_flags` | Active (`0x01`), Paused (`0x02`), Rate-limited (`0x04`), Ring (`0x80`) |
| `16..23` | Current Node | $S_i \in [0, 255]$ | Current FSM topological coordinate |
| `24..31` | Target Node | $S_{i+1} \in [0, 255]$ | Next FSM topological coordinate |
| `32..47` | Slot Anchor | `slot & 0xFFFF` | 16-bit bounded temporal slot anchor |
| `48..63` | Topology Flags | `topology_flags` | Acyclic DAG (`0x01`), No Self-Loop (`0x02`) |

---

## Instruction ABI

Compact 5-byte instruction wire:

```
[0]: bump (u8)          - O(1) canonical PDA bump seed
[1]: tag (u8)           - 0: TAG_GENESIS, 1: TAG_TRANSITION
[2]: role (u8)          - GENESIS: allowed_roles_mask | TRANSITION: requested_role (single-hot)
[3]: target (u8)        - expected next FSM coordinate
[4]: future_target (u8) - GENESIS: config_flags (Ring, DAG, Self-loop) | TRANSITION: projected next coordinate
```

### Genesis Configuration Flags (`byte [4]` in `TAG_GENESIS`)

- `0x00`: Canonical DAG default (`FLAG_ACYCLIC | FLAG_NO_SELF_LOOP` $\to$ `0x0003`)
- `bit 0` (`0x01`): `FLAG_ACYCLIC` (enforces strict forward progression)
- `bit 1` (`0x02`): `FLAG_NO_SELF_LOOP` (prohibits transitions where $S_i = S_{i+1}$)
- `bit 7` (`0x80`): `FLAG_RING_TOPOLOGY` (permits ring boundary wrap-around)

### Account Model

1. `[writable]` **State Account**: PDA derived via `["u64se-seu", authority, bump]` (exact size: 8 bytes).
2. `[signer]` **Authority Account**: Transaction signer authorized to trigger state transitions.

---

## Formal Verification (Z3 / SMT-LIB2)

All state transitions and algebraic topological constraints are formally verified using the **Z3 SMT solver** under bit-vector theory (`QF_BV`). Proofs are located in [`verify_invariants.smt2`](verify_invariants.smt2).

| Theorem | Formal Invariant | SMT Theory | Result |
| :--- | :--- | :--- | :--- |
| **Theorem 1** | **Prohibition of Self-Loops**: $S_{i+1} \ne S_i$ under DAG topology | `QF_BV` | `unsat` (proven) |
| **Theorem 2** | **Strict DAG Acyclicity**: Backward transitions ($S_{i+1} \le S_i$) rejected | `QF_BV` | `unsat` (proven) |
| **Theorem 3** | **Single-Hot Role Authorization**: Multi-hot escalation (`popcount != 1`) rejected | `QF_BV` | `unsat` (proven) |
| **Theorem 4** | **Recovery Drain Safety**: Drain requires $\Delta > 100$ slots, `ROLE_DRAIN`, and $S \to 0$ | `QF_BV` | `unsat` (proven) |
| **Theorem 5** | **Absolute Pause Lock**: Inactive, paused, or rate-limited states cannot mutate | `QF_BV` | `unsat` (proven) |
| **Theorem 6** | **Temporal Monotonicity**: Slot time skew ($\Delta > \text{0x7FFF}$) strictly rejected | `QF_BV` | `unsat` (proven) |
| **Theorem 7** | **Deterministic Ring Wrap**: Wrap-around restricted to $S_i > 0 \land S_{i+1} = 0$ | `QF_BV` | `unsat` (proven) |
| **Theorem 8** | **Forward Projection Safety**: Forward projections ($S_{i+2}$) strictly validated | `QF_BV` | `unsat` (proven) |

Run the verification suite:
```bash
z3 verify_invariants.smt2
```

---

## Verification & Testing

Verification is organized across three independent validation layers:

### 1. Host Unit & Proptest Fuzzing Suite

```bash
cargo test --release
```

- **Arbitrary Fuzzing**: Property-based testing proves evaluation never panics across arbitrary `u64` register inputs.
- **Multi-Hot Role Rejection**: Proves that multi-bit role masks are deterministically blocked.
- **Ring Wrap-Around**: Validates ring boundary transitions and rejects out-of-order jumps.
- **Temporal Arithmetic**: Validates 16-bit slot wraparound and inactivity recovery drain.

### 2. Onchain SVM Validation Suite (Mollusk SVM)

```bash
cd svm-tests && cargo run --release --bin security_tests
```

Validates SVM execution-layer invariants inside an isolated Solana runtime:
- `MissingRequiredSignature`: Non-signer authority invocations are rejected.
- `AccountAlreadyInitialized`: Replay attacks on genesis initialization are rejected.
- `InvalidSeeds`: Unauthorized PDA derivation seeds are rejected.
- `InvalidAccountData (malformed size)`: Data slices $\ne 8$ bytes are rejected.
- `InvalidAccountData (readonly)`: Mutations of non-writable accounts are rejected.

---

## CU & Performance Profile

### Mollusk SBF CU Measurement (Pinocchio Runtime)

```bash
cd svm-tests && cargo run --release --bin measure_sbf_cu
```

```text
[STEP 1]: GENESIS ACTIVATION (Clock sysvar, bump, return_data)
  Compute Units    : 1759 CU
  Status           : Success
  Active Register  : 0x0003006401000101

[STEP 2]: FSM TRANSITION S1 -> S2 (Canonical Clock, O(1) Bump)
  Compute Units    : 1864 CU
  Status           : Success
  Next Register    : 0x0003006902010101
```

### Host ALU Throughput Benchmark (x86_64)

```bash
cargo run --release --example benchmark
```

- **Evaluation Latency**: **~4.8 – 8.0 ns / op**
- **Throughput**: **~120,000,000 – 200,000,000 ops / sec**

---

## Toolchain & SBF Build

Validated environment:
- `cargo-build-sbf 4.1.0` (Platform Tools v1.54, rustc 1.89.0, Solana CLI 4.2.2)
- `Z3 4.8.12`

```bash
cargo-build-sbf
```

Produces an ultra-compact **4,720 bytes** stripped ELF SBF binary `target/deploy/u64se-seu-solana.so`.

---

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
