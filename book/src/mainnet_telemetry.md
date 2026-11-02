# Mainnet Empirical Telemetry (20 Probes, 10 Validators)

All measurements below were conducted on **Solana Mainnet-Beta** during **Epoch 1073 (Post-Alpenglow)**. Validator software versions were queried directly from Mainnet gossip (`getClusterNodes`) and cross-referenced with `getLeaderSchedule`.

---

## 1. Validator Client Distribution

| # | Validator Identity Pubkey | Client Family | Gossip Version | Probed Mainnet Slots |
| :--- | :--- | :--- | :--- | :--- |
| **1** | `privaEdSEmnMPGPoQACUkcDGkFBbTArVvsEGd7C5wUM` | Agave / Jito-Solana | **`4.3.2`** | `463931745` |
| **2** | `DrifTrN923QaouP89UxkQzFGbumKPCnfkNYQRwmZxatz` | Frankendancer (Firedancer) | **`26.10.5`** | `463931749` |
| **3** | `Ninja1spj6n9t5hVYgF3PdnYz2PLnkt7rvaw3firmjs` | Agave Release Candidate | **`4.4.0-rc.1`** | `463931754` |
| **4** | `3SkE34PVeGck2ArEffFKjihrQgURsvnoTAhitsNXNzXd` | Agave / Jito-Solana | **`4.3.2`** | `463931760` |
| **5** | `HEL1USMZKAL2odpNBj2oCjffnFGaYwmbGmyewGv1e2TU` | Agave Mainline | **`4.4.0`** | `463931765` |
| **6** | `G3a3iYZKNLbivothF3twqcaTCEvoPb9uJ2bFc9DNKkBQ` | Agave / Jito-Solana | **`4.3.2`** | `463935347` |
| **7** | `JUPiTERrZqgf1jUyR7dSkhMx4Kn2qJyekWsg3LT1h4b` | Frankendancer (Firedancer) | **`26.10.5`** | `463935348`, `463935349` |
| **8** | `EkvdKhULbMFqjKBKotAzGi3kwMvMpYNDKJXXQQmi6C1f` | Agave / Jito-Solana | **`4.3.2`** | `464360091` |
| **9** | `5Cchr1XGEg7dbBXByV5NY2ad8jfxAM7HA3x8D56rq9Ux` | Agave / Jito-Solana | **`4.3.2`** | `464360092` |
| **10** | `CAo1dCGYrB6NhHh5xb1cGjUiu86iyCfMTENxgHumSve4` | Agave / Jito-Solana | **`4.3.2`** | `464374627` |

---

## 2. Key Empirical Inversions

### Burst 200 · Frankendancer v26.10.5 Inversion (Slot 463935349)
In Slot `463935349` (Leader: `JUPiTERrZqgf...`), concurrent probes at flat 5,000 lamports base fee demonstrated strict Kendall's $\tau = -1.000$:
- **Rank 4** (`30,000 CU`, sent *last*): Captured block index **`39 / 189` (top 20.6%)**.
- **Rank 3** (`50,000 CU`, sent *earlier*): Pushed down to block index **`90 / 189` (47.6%)**.
- **Result**: Intra-block jump of **51 positions** against arrival order.

### Burst 201 · Agave 4.3.2 Inversion (Slot 464360092)
In Slot `464360092` (Leader: `5Cchr1XGEg...`), 3 concurrent probes landed within the same block:
- **Rank 4** (`30,000 CU`): Block index **`42 / 176` (top 23.9%)**.
- **Rank 1** (`150,000 CU`): Block index **`47 / 176` (26.7%)**.
- **Rank 0** (`300,000 CU`): Block index **`52 / 176` (29.5%)**.
- **Result**: Strict reverse arrival ranking ($\tau = -1.000$) across Agave 4.3.2.

### Burst 202 · Sub-2,000 CU Preset seu-checkpoint (Slot 464374627)
In Slot `464374627` (Leader: `CAo1dCGYrB...`, Agave 4.3.2), our `seu-checkpoint` preset (1,900..2,500 CU) captured **4 consecutive block indices**:
- **Indices `27, 28, 29, 30 / 375` (top 7.20% – 8.00% of block)**.
- **Interleaving Ahead of 13 out of 14 Jito MEV Bundles**: Infiltrated into the early 64-tick PoH window ahead of 92.8% of Jito tip transactions (which were scattered between indices 129 and 347) at zero priority tip.
