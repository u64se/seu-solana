# Zero-Spend Reproducibility & Docker Suite

The `u64se-block-order` measurement suite is packaged with Docker Compose for zero-setup, zero-spend keyless verification directly from `lab/block-order`.

---

## 1. Zero-Spend Dry-Run Burst (Works Without Keypair)

Run directly against live Solana Mainnet (costs 0 SOL, executes preflight simulations):

```bash
cd lab/block-order
docker compose run --rm block-order-lab burst --axis A --burst-id 0
```

---

## 2. On-Chain Verification of Any Recorded Slot (`getBlock`)

Verify intra-block transaction ordering, compute unit limits, and leader identities directly from Mainnet RPC:

```bash
# Verify Frankendancer Slot 463935349 (Kendall tau = -1.000 inversion)
docker compose run --rm block-order-lab verify --slot 463935349

# Verify Agave Slot 464374627 (Sub-2k CU ladder outpacing Jito bundles)
docker compose run --rm block-order-lab verify --slot 464374627
```

---

## 3. Recompute Full Telemetry Matrix & Statistical Analysis

Re-evaluate Kendall's $\tau$ rank correlation across all 20 probes and 10 validators:

```bash
docker compose run --rm block-order-lab analyze
```

---

## 4. Scan Live Mainnet Validator Radar

Audit client versions across current active gossip nodes:

```bash
docker compose run --rm block-order-lab radar
```

---

## 5. Live Mainnet Injection with SpendGuard

Live injection requires an explicit `--live` flag and is strictly guarded by an atomic spend limit:

```bash
SOLANA_SECRETS_DIR=/path/to/secrets \
SOLANA_KEYPAIR_PATH=/secrets/id.json \
docker compose run --rm block-order-lab burst --preset seu-checkpoint --burst-id 202 --live --max-spend-lamports 50000
```
