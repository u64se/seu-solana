# Mathematical Bit-Vector Invariants

The formal proofs in `u64se-seu-solana` depend on three key bit-algebraic invariants.

---

## 1. Single-Hot Bitmask Invariant

An authorization bitmask is single-hot if and only if exactly one bit is set to $1$. In traditional code, this is calculated via `popcount(x) == 1`. 

In `u64se-seu-solana`, single-hot checking is performed in a single ALU cycle using the Brian Kernighan bit-twiddling identity:

$$(r \ne 0) \land ((r \mathbin{\&} (r - 1)) = 0)$$

If an attacker passes a multi-role bitmask (e.g. `ROLE_SWAP | ROLE_FLASHLOAN` $\to 0x21$), $(0x21 \mathbin{\&} 0x20) \ne 0$, instantly setting `FaultMask::ROLE_UNAUTHORIZED (0x02)`.

---

## 2. Modular Slot Window (Wraparound Invariant)

Solana slots are represented as 64-bit integers on-chain, but `u64se` bounds temporal tracking to a compact 16-bit register (`0..65535`):

```rust
let slot_delta = current_slot.wrapping_sub(anchor_slot);
```

Under 16-bit modular arithmetic:
- A normal forward step within the epoch: $\Delta \in [1, 100]$.
- A slot wraparound across boundary ($65535 \to 5$):
  $$(5 - 65535) \pmod{65536} = 6 \le \text{0x7FFF} \implies \text{VALID}$$
- A stale transaction or backward replay ($100 \to 90$):
  $$(90 - 100) \pmod{65536} = 65526 > \text{0x7FFF} \implies \text{TRAPPED (0x20 TIME\_SKEW)}$$
- A duplicate submission within the exact same slot ($\Delta = 0$):
  $$\Delta = 0 \implies \text{TRAPPED (0x40 RATE\_LIMITED)}$$

This eliminates both replay attacks and same-slot flashloan re-entrancy without requiring external tables.
