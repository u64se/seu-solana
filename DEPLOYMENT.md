# u64se-seu-solana · Deployment Registry

## Solana Devnet (Cluster: devnet)

| Parameter | Value |
| :--- | :--- |
| **Status** | `[M4: STABLE_METAL]` (Live On-Chain) |
| **Program ID** | `FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H` |
| **ProgramData Address** | `6is2yZs555hs5zE3JcgHQsfosaZKMRfGY96k5ReA3CeJ` |
| **Upgrade Authority** | `6MpbQcu81Sw1dXtUFzYR1sV5Ff8jFdizTjKLNtmkuVF4` (esense main) |
| **Binary Artifact** | `target/deploy/u64se-seu-solana.so` |
| **Binary Size** | 4,720 bytes (stripped ELF SBF) |
| **Rent Balance** | 0.03119628 SOL |
| **Latest Upgrade Slot** | `498148813` |
| **Upgrade Tx** | `3xCDMUTU5CMNnJHtpHRAc1BQTc33MdRv2SZcaiTF2sz1UVLKAKwFrRGEh19zVHKAgRNRKyTbkEe19FdL83SjBz6P` |
| **SolanaFM Explorer** | https://solana.fm/tx/3xCDMUTU5CMNnJHtpHRAc1BQTc33MdRv2SZcaiTF2sz1UVLKAKwFrRGEh19zVHKAgRNRKyTbkEe19FdL83SjBz6P?cluster=devnet-solana |
| **Program on SolanaFM** | https://solana.fm/address/FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H?cluster=devnet-solana |
| **Solscan Explorer** | https://solscan.io/account/FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H?cluster=devnet |

## Account Topology & Seeds

- **Program Owner**: `BPFLoaderUpgradeab1e11111111111111111111111`
- **State PDA Derivation**: `["u64se-seu", authority_pubkey, &[bump]]`
- **State Size**: exactly 8 bytes (`u64` register word)
- **Instruction Wire**: 5 bytes (`[bump, tag, role, target, future_target]`)
