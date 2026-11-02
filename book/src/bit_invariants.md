# Mathematical Bit-Vector Invariants

The formal proofs in `u64se-seu-solana` depend on five core bit-algebraic invariants verified in Z3 SMT (`QF_BV`).

---

## 1. Single-Hot Bitmask Invariant (Theorem 3)

An authorization bitmask is single-hot if and only if exactly one bit is set to $1$. In traditional code, this is calculated via `popcount(x) == 1`. 

In `u64se-seu-solana`, single-hot checking is performed in a single ALU cycle using the Brian Kernighan bit-twiddling identity:

$$(r \ne 0) \land ((r \mathbin{\&} (r - 1)) = 0)$$

If an attacker passes a multi-role bitmask (e.g. `ROLE_SWAP | ROLE_FLASHLOAN` $\to 0x21$), $(0x21 \mathbin{\&} 0x20) \ne 0$, instantly setting `FaultMask::ROLE_UNAUTHORIZED (0x02)`.

---

## 2. Modular Slot Window (Wraparound Invariant, Theorem 6 & 7)

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

---

## 3. Incremental Adjoint XOR Parity Preservation (Theorems 9 & 10)

For multi-stage or crystal state verification, `u64se` maintains an adjoint XOR digest $P$:

$$P' = P \oplus \text{old\_cube} \oplus \text{new\_cube}$$

Because XOR is commutative, associative, and its own inverse ($x \oplus x = 0$), incremental single-word updates preserve full crystal parity in $O(1)$ without re-hashing parent nodes. Any single-bit corruption in any cell alters $P'$, providing guaranteed single-cube tamper detection (Theorem 10).

---

## 4. Probe Telemetry Injective Packing Bijection (Theorem 13)

Mainnet telemetry probes encode experimental metadata directly into the 64-bit word:

$$\text{word} = \text{pack\_probe\_seu}(\text{cohort}, \text{axis}, \text{burst\_id}, \text{send\_rank}, \text{slot16}, \text{parity})$$

Theorem 13 proves that bit-fields do not overlap or bleed, guaranteeing that distinct `(burst_id, send_rank)` pairs yield strictly distinct 64-bit words ($f(a) = f(b) \implies a = b$).

---

## 5. Priority Density Monotonicity & Fee Overflow Freedom (Theorems 14 & 15)

In unsigned 64-bit integer arithmetic (`bvudiv`), Theorem 14 proves that at flat base fee $B = 5,000$ lamports:

$$\forall c_1 < c_2 : \lfloor B / c_1 \rfloor \ge \lfloor B / c_2 \rfloor$$

Furthermore, Theorem 15 proves that the total fee calculation $5000 + (c \cdot p) / 10^6$ never overflows a 64-bit unsigned integer across all valid Solana compute budgets ($c \le 1.4 \times 10^6\text{ CU}$) and priority rates ($p \le 10^{12}\ \mu\text{L/CU}$).
