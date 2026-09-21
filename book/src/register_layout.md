# 64-Bit State Execution Register

The `StateExecutionUnit` type is declared as a transparent wrapper over an unsigned 64-bit integer:

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateExecutionUnit(pub u64);
```

---

## Detailed Bit-Range Allocation

| Bit Range | Field Name | Type | Canonical Encoding | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `0..7` | `ROLE_MASK` | `u8` | Bitfield (`1 << bit`) | Single-hot authorized role mask |
| `8..15` | `STATUS_FLAGS` | `u8` | Bitfield | Operational state of the state machine |
| `16..23` | `CURRENT_NODE` | `u8` | Scalar $[0, 255]$ | Current state coordinate ($S_i$) |
| `24..31` | `TARGET_NODE` | `u8` | Scalar $[0, 255]$ | Committed next state coordinate ($S_{i+1}$) |
| `32..47` | `EPOCH_SLOT` | `u16` | Scalar $[0, 65535]$ | 16-bit temporal anchor (`slot & 0xFFFF`) |
| `48..63` | `TOPOLOGY_FLAGS` | `u16` | Bitfield | Structural graph constraints |

---

## Field Specifications

### 1. Role Authorization (`bits 0..7`)
Defines the authorized caller roles for state mutation:
- `ROLE_SWAP` (`1 << 0` = `0x01`): Token and asset swaps
- `ROLE_BUY_NFT` (`1 << 1` = `0x02`): NFT acquisition & minting
- `ROLE_ORACLE_UPDATE` (`1 << 2` = `0x04`): Price and state attestations
- `ROLE_GOVERNANCE` (`1 << 3` = `0x08`): Parameter reconfiguration
- `ROLE_DRAIN` (`1 << 4` = `0x10`): Emergency recovery drainage
- `ROLE_FLASHLOAN` (`1 << 5` = `0x20`): Uncollateralized single-slot liquidity

> **Single-Hot Enforcement**: A requested role is valid if and only if exactly one bit is set (`popcount(role) == 1`). Multi-bit role injection is rejected in constant time via `(r & (r - 1)) != 0`.

### 2. Operational Status (`bits 8..15`)
- `STATUS_ACTIVE` (`1 << 8` = `0x0100`): State machine is operational.
- `STATUS_PAUSED` (`1 << 9` = `0x0200`): Invariant circuit breaker triggered; all mutations blocked.
- `STATUS_RATE_LIMITED` (`1 << 10` = `0x0400`): Temporal throttle active.
- `FLAG_RING_TOPOLOGY` (`1 << 15` = `0x8000`): Permitted torus/ring wrap-around ($S_n \to S_0$).

### 3. FSM Coordinates (`bits 16..31`)
Encodes the current position $S_i$ and the valid target position $S_{i+1}$. Under default DAG rules:
$$S_{i+1} > S_i$$

### 4. Slot Window Anchor (`bits 32..47`)
Tracks the lower 16 bits of the Solana slot counter (`slot & 0xFFFF`). Protects against transaction replay, slot drift, and same-slot re-entrancy attacks.

### 5. Topology Flags (`bits 48..63`)
- `FLAG_ACYCLIC` (`1 << 48`): Enforces strict monotonically increasing topological ordering.
- `FLAG_NO_SELF_LOOP` (`1 << 49`): Strictly forbids $S_{i+1} = S_i$.
