# Scheduler Reward Density Mechanics

A persistent misconception among Solana developers is that reducing compute budget below the default 200,000 CU ceiling provides no economic benefit when base transaction fees are flat at `5,000` lamports. 

Empirical measurements across both Agave and Frankendancer validators demonstrate the opposite.

---

## The Priority Score Formula

In modern validator scheduling architectures, incoming transactions are prioritized by **Reward Density** per requested compute unit:

$$\text{Priority Score} \propto \frac{\text{Base Fee (5,000 L)} + \text{Priority Fee}}{\text{Requested Compute Unit Limit}}$$

Because the requested Compute Unit limit sits directly in the denominator, decreasing the budget increases scheduler priority score at zero incremental financial expenditure.

---

## Density Scaling Table (Flat 5,000 Lamport Base Fee)

| Program Architecture | Requested CU Budget | Total Base Fee | Priority Tip | Reward Density | Relative Priority Multiplier |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Standard Anchor Default** | `200,000 CU` | `5,000 L` | `0 L` | **0.0250 L/CU** | **1.0&times; (Baseline)** |
| **Optimized Borsh Contract** | `50,000 CU` | `5,000 L` | `0 L` | **0.1000 L/CU** | **4.0&times;** |
| **Constrained Micro-Program** | `30,000 CU` | `5,000 L` | `0 L` | **0.1667 L/CU** | **6.67&times;** |
| **u64se-seu-solana Wire** | `2,000 CU` | `5,000 L` | `0 L` | **2.5000 L/CU** | **100.0&times; (Dominant)** |

To achieve the same $2.50\text{ L/CU}$ reward density with a bloated 200,000 CU transaction, a sender would be forced to pay an additional **495,000 lamports (~0.0005 SOL)** priority tip on every transaction.

---

## Experimental Isolation: Write-Lock Contention Model

To isolate scheduling priority from arrival order and eliminate concurrency noise, `u64se-block-order` enforces two experimental constraints:

1. **Single-Payer Write-Lock Contention:** All probe transactions write-lock the identical fee-payer authority (`6MpbQcu...`). Because Solana's Sealevel runtime serializes transactions that write-lock the same account, the validator is forced to establish a strict total ordering among our probes when they enter the same Banking Stage window.
2. **Inverted Dispatch Order:** Probes are constructed and dispatched from `Rank 0` (lowest priority density) to `Rank 4` (highest priority density). When `Rank 4` lands ahead of `Rank 3` inside the same block, pure arrival FIFO is falsified in favor of priority-density sorting.
