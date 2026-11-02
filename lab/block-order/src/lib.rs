//! # `u64se-block-order`
//!
//! Reusable Solana validator block-ordering probe, telemetry bit-packer, and
//! intra-block rank correlation library built on `u64se-seu-solana::StateExecutionUnit`.
//!
//! ## 64-Bit Probe Layout (`StateExecutionUnit` Register)
//! - `bits 0..7`   (`role`):         Validator cohort ID
//! - `bits 8..15`  (`status`):       Experimental axis (`1` = Axis A CU-Density, `2` = Axis B Priority-Fee)
//! - `bits 16..23` (`current_node`): Burst ID (`0..255`)
//! - `bits 24..31` (`target_node`):  Inverted send rank (`0..4`)
//! - `bits 32..47` (`epoch_slot`):   16-bit target slot anchor (`slot & 0xFFFF`)
//! - `bits 48..63` (`flags`+parity): Topological flags & crystal parity digest

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Keypair, Signer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub use u64se_seu_solana::StateExecutionUnit;

pub const COMPUTE_BUDGET_ID: &str = "ComputeBudget111111111111111111111111111111";
pub const MEMO_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeLogRecord {
    pub timestamp: String,
    pub cluster_slot: u64,
    pub burst_id: u8,
    pub axis: String,
    pub send_rank: u8,
    pub expected_priority_rank: u8,
    pub cu_limit: u32,
    pub price_micro_lamports: u64,
    pub probe_word_hex: String,
    pub signature: String,
    pub simulated_cu: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifiedProbeRow {
    pub signature: String,
    pub slot: u64,
    pub burst_id: u8,
    pub axis: String,
    pub send_rank: u8,
    pub cu_limit: u32,
    pub price_micro_lamports: u64,
    pub fee_lamports: u64,
    pub block_tx_index: i64,
    pub total_txs_in_block: usize,
    pub relative_position: f64,
    pub leader: String,
    pub client_version: String,
    pub seu_register_hex: String,
}

/// Pack experimental telemetry into the canonical 64-bit `StateExecutionUnit` register.
pub fn pack_probe_seu(
    cohort: u8,
    axis_code: u8,
    burst_id: u8,
    send_rank: u8,
    slot: u64,
) -> StateExecutionUnit {
    let seu = StateExecutionUnit::new(
        cohort,
        axis_code,
        burst_id,
        send_rank,
        (slot & 0xFFFF) as u16,
        0x0001,
    );
    let digest = (seu.0.count_ones() as u8) ^ burst_id ^ send_rank;
    seu.with_parity_digest(digest)
}

/// Format the 64-bit `StateExecutionUnit` register as an on-chain Memo payload.
pub fn seu_to_memo(seu: StateExecutionUnit) -> String {
    format!("u64se:probe:0x{:016X}", seu.0)
}

/// Parse a 64-bit `StateExecutionUnit` register from an on-chain Memo string.
pub fn seu_from_memo(s: &str) -> Option<StateExecutionUnit> {
    let prefix = "u64se:probe:0x";
    let pos = s.find(prefix)?;
    let hex_part: String = s[pos + prefix.len()..].chars().take(16).collect();
    if hex_part.len() == 16 {
        u64::from_str_radix(&hex_part, 16)
            .ok()
            .map(StateExecutionUnit)
    } else {
        None
    }
}

/// Build and sign a deterministic Solana Legacy Transaction containing:
/// 1. `ComputeBudget::SetComputeUnitLimit(cu_limit)`
/// 2. Optional `ComputeBudget::SetComputeUnitPrice(price_micro_lamports)`
/// 3. `SPL-Memo` signed by `payer` carrying the 64-bit `StateExecutionUnit` probe word.
pub fn build_signed_probe_tx(
    keypair: &Keypair,
    recent_blockhash_b58: &str,
    cu_limit: u32,
    price_micro_lamports: u64,
    memo_str: &str,
) -> Result<(String, String), String> {
    let payer_bytes = keypair.public.to_bytes();
    let compute_budget_bytes = bs58::decode(COMPUTE_BUDGET_ID)
        .into_vec()
        .map_err(|e| e.to_string())?;
    let memo_prog_bytes = bs58::decode(MEMO_PROGRAM_ID)
        .into_vec()
        .map_err(|e| e.to_string())?;
    let blockhash_bytes = bs58::decode(recent_blockhash_b58)
        .into_vec()
        .map_err(|e| e.to_string())?;

    if blockhash_bytes.len() != 32 {
        return Err("Invalid recent blockhash length".to_string());
    }

    let mut msg = Vec::with_capacity(256);
    // Message Header: 1 required signature, 0 readonly signed, 2 readonly unsigned
    msg.push(1u8);
    msg.push(0u8);
    msg.push(2u8);

    // Compact-u16 length of account_keys = 3
    msg.push(3u8);
    msg.extend_from_slice(&payer_bytes);
    msg.extend_from_slice(&compute_budget_bytes);
    msg.extend_from_slice(&memo_prog_bytes);

    // Recent blockhash (32 bytes)
    msg.extend_from_slice(&blockhash_bytes);

    // Instructions
    let ix_count = if price_micro_lamports > 0 { 3u8 } else { 2u8 };
    msg.push(ix_count);

    // Instruction 0: ComputeBudget::SetComputeUnitLimit (discriminant = 2, u32 LE)
    msg.push(1u8); // program_id_index = 1 (ComputeBudget)
    msg.push(0u8); // accounts len = 0
    msg.push(5u8); // data len = 5
    msg.push(2u8); // SetComputeUnitLimit tag
    msg.extend_from_slice(&cu_limit.to_le_bytes());

    // Instruction 1 (optional): ComputeBudget::SetComputeUnitPrice (discriminant = 3, u64 LE)
    if price_micro_lamports > 0 {
        msg.push(1u8); // program_id_index = 1 (ComputeBudget)
        msg.push(0u8); // accounts len = 0
        msg.push(9u8); // data len = 9
        msg.push(3u8); // SetComputeUnitPrice tag
        msg.extend_from_slice(&price_micro_lamports.to_le_bytes());
    }

    // Final Instruction: SPL-Memo with payer as signer (account index 0)
    let memo_bytes = memo_str.as_bytes();
    msg.push(2u8); // program_id_index = 2 (Memo)
    msg.push(1u8); // accounts len = 1
    msg.push(0u8); // account index 0 (payer)
    msg.push(memo_bytes.len() as u8);
    msg.extend_from_slice(memo_bytes);

    // Ed25519 signature over message
    let sig = keypair.sign(&msg);
    let sig_b58 = bs58::encode(sig.to_bytes()).into_string();

    // Wire transaction: [compact-u16(1), 64-byte signature, message...]
    let mut wire = Vec::with_capacity(1 + 64 + msg.len());
    wire.push(1u8);
    wire.extend_from_slice(&sig.to_bytes());
    wire.extend_from_slice(&msg);

    Ok((sig_b58, BASE64.encode(&wire)))
}

pub fn load_solana_keypair(path: &str) -> Result<Keypair, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read keypair {}: {}", path, e))?;
    let bytes: Vec<u8> =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid keypair JSON: {}", e))?;
    if bytes.len() < 64 {
        return Err("Keypair file must contain 64 bytes".to_string());
    }
    Keypair::from_bytes(&bytes[..64]).map_err(|e| format!("Ed25519 keypair error: {}", e))
}

pub fn ephemeral_simulation_keypair() -> Keypair {
    let secret = ed25519_dalek::SecretKey::from_bytes(&[7u8; 32]).unwrap();
    let public: ed25519_dalek::PublicKey = (&secret).into();
    Keypair { secret, public }
}

pub fn rpc_call(rpc_url: &str, method: &str, params: Value) -> Result<Value, String> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    for attempt in 0..5 {
        let resp = ureq::post(rpc_url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "u64se-block-order/0.2")
            .timeout(Duration::from_secs(15))
            .send_json(payload.clone());

        match resp {
            Ok(r) => {
                let body: Value = r.into_json().map_err(|e| e.to_string())?;
                if let Some(err) = body.get("error") {
                    return Err(format!("RPC {} error: {}", method, err));
                }
                return Ok(body["result"].clone());
            }
            Err(ureq::Error::Status(429, _)) | Err(ureq::Error::Transport(_)) => {
                thread::sleep(Duration::from_millis(600 * (attempt + 1)));
            }
            Err(e) => return Err(format!("HTTP error on {}: {}", method, e)),
        }
    }
    Err(format!("RPC {} exhausted retries", method))
}

pub fn now_unix_secs() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

/// Compute Kendall's rank correlation coefficient ($\tau$) between two rank vectors.
pub fn kendall_tau(x: &[i64], y: &[i64]) -> f64 {
    let n = x.len();
    if n < 2 {
        return 0.0;
    }
    let mut concordant = 0i64;
    let mut discordant = 0i64;
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = x[i] - x[j];
            let dy = y[i] - y[j];
            let prod = dx * dy;
            if prod > 0 {
                concordant += 1;
            } else if prod < 0 {
                discordant += 1;
            }
        }
    }
    let total = concordant + discordant;
    if total == 0 {
        0.0
    } else {
        (concordant - discordant) as f64 / (total as f64)
    }
}

/// Classify a validator gossip version string into its client family.
pub fn classify_client_family(version: &str) -> &'static str {
    if version.starts_with("26.") || version.starts_with("0.8") || version.starts_with("0.9") {
        "Frankendancer / Firedancer"
    } else if version.starts_with("4.") || version.starts_with("3.") || version.starts_with("2.") {
        "Agave / Jito-Solana"
    } else {
        "Custom / Legacy SVM"
    }
}
