# FaultMask Bitfield (0x00..0x80)

Unlike standard contracts that abort execution with arbitrary numeric error codes (e.g. `102`, `6001`), `u64se-seu-solana` records all failure modalities as a transparent 8-bit bitmask (`FaultMask`).

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FaultMask(pub u8);
```

---

## Canonical Bit Allocation

| Bit | Hex | Constant Identifier | Formal Invariant Violated |
| :---: | :---: | :--- | :--- |
| `—` | `0x00` | `FaultMask::PASS` | All invariants satisfied (0 violations) |
| `0` | `0x01` | `INACTIVE_OR_PAUSED` | State machine is halted, paused, or uninitialized |
| `1` | `0x02` | `ROLE_UNAUTHORIZED` | Caller requested a forbidden role or multi-hot bitmask |
| `2` | `0x04` | `TARGET_MISMATCH` | Expected target coordinate does not match committed target |
| `3` | `0x08` | `SELF_LOOP_FORBIDDEN` | Illegal self-loop transition ($S_{i+1} = S_i$) |
| `4` | `0x10` | `BACKWARD_FORBIDDEN` | Illegal backward/re-entrant transition ($S_{i+1} < S_i$) |
| `5` | `0x20` | `SLOT_TIME_SKEW` | Clock drift from past epoch ($\Delta > \text{0x7FFF}$) |
| `6` | `0x40` | `RATE_LIMITED` | Slot collision; duplicate transition within same slot ($\Delta = 0$) |
| `7` | `0x80` | `VALUE_OVERFLOW` | Reserved for arithmetic coordinate boundary breach |

---

## Multiple Concurrent Fault Recording

Because `FaultMask` is a bitfield, a single evaluation step captures **all simultaneous violations** rather than short-circuiting on the first error. 

For example, if an attacker attempts a backward transition with an unauthorized multi-bit role:
$$\text{fault} = \text{ROLE\_UNAUTHORIZED (0x02)} \mid \text{BACKWARD\_FORBIDDEN (0x10)} = \mathbf{0x12}$$

This gives client-side monitors, MEV searchers, and circuit breakers complete observability into why an action was fenced off.

---

## Helper Methods

```rust
impl FaultMask {
    /// Returns true if and only if zero fault bits are active.
    #[inline(always)]
    pub const fn is_pass(&self) -> bool {
        self.0 == Self::PASS
    }

    /// Tests for specific fault conditions.
    #[inline(always)]
    pub const fn contains(&self, bit: u8) -> bool {
        (self.0 & bit) != 0
    }
}
```
