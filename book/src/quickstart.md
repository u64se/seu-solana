# Developer Quickstart & CLI

Get started with `u64se-seu-solana` in Rust or TypeScript.

---

## 1. Rust Native Dependency

Add `u64se-seu-solana` to your `Cargo.toml`:

```toml
[dependencies]
u64se-seu-solana = { git = "https://github.com/u64se/seu-solana", branch = "main" }
```

### Local Branchless Evaluation

```rust
use u64se_seu_solana::{StateExecutionUnit, FaultMask};

fn main() {
    // 1. Initialize an active FSM: S0 -> S1, allowed: ROLE_SWAP
    let state = StateExecutionUnit::new(
        StateExecutionUnit::ROLE_SWAP as u8,
        StateExecutionUnit::STATUS_ACTIVE as u8,
        0,   // current: S0
        1,   // target: S1
        100, // anchor slot
        (StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) as u16,
    );

    // 2. Evaluate forward step to S1
    let (new_state, fault) = state.evaluate(
        StateExecutionUnit::ROLE_SWAP as u8, // requested role
        1,   // target: S1
        2,   // future projection: S2
        105, // current slot
    );

    assert!(fault.is_pass());
    println!("Transition passed! New state word: 0x{:016X}", new_state.0);
}
```

---

## 2. Solana Web3 / TypeScript Integration

Construct a 5-byte instruction wire:

```typescript
import { PublicKey, TransactionInstruction, Transaction } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H");

// Hop: S0 -> S1
const bump = 0;
const tag = 1;      // TAG_TRANSITION
const role = 1;     // ROLE_SWAP (0x01)
const target = 1;   // S1
const future = 2;   // S2

const ix = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [
    { pubkey: payer.publicKey, isSigner: true, isWritable: false }
  ],
  data: Buffer.from([bump, tag, role, target, future])
});

const tx = new Transaction().add(ix);
// Broadcast to Solana Devnet (CU consumed: ~341 CU)
```

---

## 3. Formal Invariant Verification

Run the formal verification suite locally:

```bash
git clone https://github.com/u64se/seu-solana
cd seu-solana
z3 verify_invariants.smt2
```

All 8 QF_BV mathematical theorems will verify and return `unsat`.
