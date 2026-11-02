# Inside the Block: Validator Ordering Lab

`u64se-block-order` (`lab/block-order`) is a dedicated empirical research and measurement module inside `u64se-seu-solana` designed to evaluate how Solana validator schedulers order transactions into blocks.

- **Network**: Solana Mainnet-Beta (Post-Alpenglow Upgrade, Epoch `1073`)
- **Measurement Date**: November 1–2, 2026 (Slots `463,931,745` to `464,374,627`)
- **Validator Diversity**: **10 distinct Mainnet validators** across **4 distinct client software versions**:
  - `Agave 4.3.2`
  - `Agave 4.4.0`
  - `Agave 4.4.0-rc.1`
  - `Frankendancer / Firedancer 26.10.5`
- **Total Mainnet Spend**: **`0.001017 SOL`** (across 20 live Mainnet probe transactions, strictly bounded by an atomic `SpendGuard` limit of `0.0015 SOL`)
- **Formal Guarantee**: Backed by **Theorems 13, 14, and 15** in [`verify_invariants.smt2`](https://github.com/u64se/seu-solana/blob/main/verify_invariants.smt2).

---

## Executive Summary & Core Findings

Rather than treating Solana block production as a naive First-In-First-Out (FIFO) queue or a one-dimensional priority fee auction, this module injects deterministic, self-identifying probe transactions carrying a canonical 64-bit `StateExecutionUnit` register word (`u64se:probe:0x...`) directly into Mainnet leader windows.

### Three Key Results:

1. **Intra-Block Inversion via Requested CU Limit (Kendall's $\tau = -1.000$):**  
   At flat `5,000` lamports base fee and zero priority tip, reducing requested compute budget strictly elevates scheduler priority score. On Frankendancer v26.10.5 (Slot `463935349`), a probe requesting `30,000 CU` jumped **51 positions ahead** of a prior probe requesting `50,000 CU`. On Agave 4.3.2 (Slot `464360092`), all concurrent probes sorted in exact inverse order of arrival.
2. **Sub-2,000 CU Infiltration Ahead of Jito MEV Bundles:**  
   In Slot `464374627` (Agave 4.3.2), our `seu-checkpoint` preset (1,900..2,500 CU) captured 4 consecutive block slots (**indices 27, 28, 29, 30 / 375**; top 7.2% of block) ahead of 13 out of 14 Jito MEV bundles that paid substantial tips but were appended to later PoH ticks.
3. **100x Scheduler Leverage for `< 1,900 CU` Programs:**  
   Setting a tight `ComputeUnitLimit(2,000)` yields a **100x higher validator reward density** ($2.50\text{ L/CU}$ vs $0.025\text{ L/CU}$) at zero priority fee compared to standard 200,000 CU defaults.
