//! CLI entrypoint for `block-order-lab` (`u64se-block-order`).

use bs58;
use serde_json::json;
use std::{
    collections::HashMap,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    thread,
};
use u64se_block_order::{
    build_signed_probe_tx, classify_client_family, ephemeral_simulation_keypair, kendall_tau,
    load_solana_keypair, now_unix_secs, pack_probe_seu, rpc_call, seu_from_memo, seu_to_memo,
    ProbeLogRecord, VerifiedProbeRow,
};

fn print_help() {
    println!(
        "u64se Block-Order-Lab v0.2.0 (Native StateExecutionUnit Telemetry Suite)

USAGE:
  block-order-lab [OPTIONS] <SUBCOMMAND>

OPTIONS:
  --rpc <URL>                 Solana RPC endpoint [default: SOLANA_RPC_URL or Mainnet]
  --keypair <PATH>            Keypair JSON path [default: SOLANA_KEYPAIR_PATH]
  --telemetry-dir <DIR>       Telemetry directory [default: TELEMETRY_DIR or ./telemetry]

SUBCOMMANDS:
  radar                       Discover Mainnet validator client families & leader windows (0 SOL)
  burst                       Fire calibrated 5-tx inverted probe burst (Dry-Run by default)
      --axis <A|B>            Axis A (CU limit density @ 5000L) or Axis B (Priority Fee tiers)
      --burst-id <N>          8-bit burst identifier [default: 0]
      --live                  Broadcast live transactions (protected by SpendGuard)
      --max-spend-lamports    Hard atomic spend cap [default: 1500000]
  verify --slot <SLOT>        Inspect a block via getBlock and decode u64se StateExecutionUnit probes
  analyze                     Verify all recorded live probes, compute Kendall tau & write verified_matrix.json"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2
        || args
            .iter()
            .any(|a| a == "-h" || a == "--help" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let mut rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let mut keypair_path = env::var("SOLANA_KEYPAIR_PATH").ok();
    let mut telemetry_dir = env::var("TELEMETRY_DIR").unwrap_or_else(|_| "./telemetry".to_string());

    let mut subcmd = String::new();
    let mut axis = "A".to_string();
    let mut burst_id: u8 = 0;
    let mut live = false;
    let mut max_spend_lamports: u64 = 1_500_000;
    let mut verify_slot: u64 = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rpc" | "-r" => {
                i += 1;
                if i < args.len() {
                    rpc_url = args[i].clone();
                }
            }
            "--keypair" | "-k" => {
                i += 1;
                if i < args.len() {
                    keypair_path = Some(args[i].clone());
                }
            }
            "--telemetry-dir" => {
                i += 1;
                if i < args.len() {
                    telemetry_dir = args[i].clone();
                }
            }
            "--axis" => {
                i += 1;
                if i < args.len() {
                    axis = args[i].clone();
                }
            }
            "--burst-id" => {
                i += 1;
                if i < args.len() {
                    burst_id = args[i].parse().unwrap_or(0);
                }
            }
            "--live" => {
                live = true;
            }
            "--dry-run" => {
                live = false;
            }
            "--max-spend-lamports" => {
                i += 1;
                if i < args.len() {
                    max_spend_lamports = args[i].parse().unwrap_or(1_500_000);
                }
            }
            "--slot" | "-s" => {
                i += 1;
                if i < args.len() {
                    verify_slot = args[i].parse().unwrap_or(0);
                }
            }
            cmd if subcmd.is_empty() && !cmd.starts_with('-') => {
                subcmd = cmd.to_string();
            }
            _ => {}
        }
        i += 1;
    }

    match subcmd.as_str() {
        "radar" => {
            println!("--- [u64se MAINNET VALIDATOR RADAR] ---");
            let epoch_info = rpc_call(&rpc_url, "getEpochInfo", json!([]))?;
            let current_slot = epoch_info["absoluteSlot"].as_u64().unwrap_or(0);
            let epoch = epoch_info["epoch"].as_u64().unwrap_or(0);
            println!(
                "Cluster RPC: {} | Epoch: {} | Current Slot: {}",
                rpc_url, epoch, current_slot
            );

            let nodes = rpc_call(&rpc_url, "getClusterNodes", json!([]))?;
            let mut counts: HashMap<String, usize> = HashMap::new();
            if let Some(arr) = nodes.as_array() {
                for n in arr {
                    let v = n["version"].as_str().unwrap_or("unknown").to_string();
                    *counts.entry(v).or_insert(0) += 1;
                }
                println!("Total Gossip Nodes: {}", arr.len());
            }
            let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            println!("Top Validator Client Software Versions:");
            for (ver, c) in sorted.into_iter().take(8) {
                println!(
                    "  {:14} -> {:4} nodes ({})",
                    ver,
                    c,
                    classify_client_family(&ver)
                );
            }
        }

        "burst" => {
            let dry_run = !live;
            let payer = match keypair_path.as_deref().map(load_solana_keypair) {
                Some(Ok(kp)) => kp,
                Some(Err(e)) if dry_run => {
                    println!(
                        "[DRY-RUN KEYLESS MODE] ({}) -> using ephemeral simulation signer",
                        e
                    );
                    ephemeral_simulation_keypair()
                }
                None if dry_run => {
                    println!("[DRY-RUN KEYLESS MODE] No keypair specified -> using ephemeral simulation signer");
                    ephemeral_simulation_keypair()
                }
                Some(Err(e)) => return Err(e.into()),
                None => {
                    return Err(
                        "Keypair path required in --live mode via --keypair or SOLANA_KEYPAIR_PATH"
                            .into(),
                    )
                }
            };
            let payer_pubkey = bs58::encode(payer.public.to_bytes()).into_string();

            println!("--- [PARALLEL u64se BURST INJECTION] ---");
            println!("Wallet: {}", payer_pubkey);
            println!("Cluster RPC: {}", rpc_url);
            println!("Axis: {} | Burst ID: {} | Dry-Run: {}", axis, burst_id, dry_run);
            println!("Spend Guard limit: {} lamports", max_spend_lamports);

            let initial_slot = rpc_call(&rpc_url, "getSlot", json!([]))?
                .as_u64()
                .unwrap_or(0);
            println!("Current slot: {}", initial_slot);

            let configs: Vec<(u32, u64, u8)> = if axis == "A" {
                vec![
                    (300_000, 0, 0),
                    (150_000, 0, 1),
                    (80_000, 0, 2),
                    (50_000, 0, 3),
                    (30_000, 0, 4),
                ]
            } else {
                vec![
                    (35_000, 0, 0),
                    (35_000, 200_000, 1),
                    (35_000, 1_000_000, 2),
                    (35_000, 5_000_000, 3),
                    (35_000, 20_000_000, 4),
                ]
            };

            let estimated_spend: u64 = configs
                .iter()
                .map(|(cu, price, _)| 5_000 + ((*cu as u64) * price) / 1_000_000)
                .sum();
            println!("Estimated burst fee spend: {} lamports", estimated_spend);
            if estimated_spend > max_spend_lamports {
                return Err(format!(
                    "SpendGuard triggered: estimated {} lamports exceeds cap {} lamports",
                    estimated_spend, max_spend_lamports
                )
                .into());
            }

            let bh_res = rpc_call(
                &rpc_url,
                "getLatestBlockhash",
                json!([{"commitment": "confirmed"}]),
            )?;
            let blockhash = bh_res["value"]["blockhash"]
                .as_str()
                .ok_or("Missing blockhash")?
                .to_string();

            let axis_code = if axis == "A" { 1u8 } else { 2u8 };
            let mut built = Vec::new();
            for (cu_limit, price, rank) in configs {
                let seu = pack_probe_seu(1, axis_code, burst_id, rank, initial_slot);
                let memo_str = seu_to_memo(seu);
                let (sig_b58, wire_b64) =
                    build_signed_probe_tx(&payer, &blockhash, cu_limit, price, &memo_str)?;
                built.push((rank, cu_limit, price, seu, memo_str, sig_b58, wire_b64));
            }

            fs::create_dir_all(&telemetry_dir)?;
            let log_file_path = format!("{}/probes.jsonl", telemetry_dir);

            if dry_run {
                for (rank, cu_limit, price, _seu, memo_str, _sig, wire_b64) in built {
                    let sim = rpc_call(
                        &rpc_url,
                        "simulateTransaction",
                        json!([wire_b64, {"encoding": "base64", "commitment": "confirmed", "sigVerify": false, "replaceRecentBlockhash": true}]),
                    )?;
                    let units = sim["value"]["unitsConsumed"].as_u64().unwrap_or(0);
                    println!(
                        "[DRY-RUN] Rank: {} | CU Limit: {} | Price: {} | Sim CU: {} | Memo: {}",
                        rank, cu_limit, price, units, memo_str
                    );
                }
                println!("Dry-run simulation completed (0 SOL spent).");
            } else {
                let mut log_file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file_path)?;
                println!(">>> FIRING ALL 5 TRANSACTIONS CONCURRENTLY <<<");
                let results = thread::scope(|s| {
                    let mut handles = Vec::new();
                    for (rank, cu_limit, price, seu, _memo_str, _sig_b58, wire_b64) in built {
                        let rpc_ref = &rpc_url;
                        handles.push(s.spawn(move || {
                            let send_res = rpc_call(
                                rpc_ref,
                                "sendTransaction",
                                json!([wire_b64, {"encoding": "base64", "skipPreflight": true}]),
                            );
                            (rank, cu_limit, price, seu, send_res)
                        }));
                    }
                    handles
                        .into_iter()
                        .map(|h| h.join().unwrap())
                        .collect::<Vec<_>>()
                });

                for (rank, cu_limit, price, seu, send_res) in results {
                    match send_res {
                        Ok(val) => {
                            let sig = val.as_str().unwrap_or_default().to_string();
                            println!(
                                "[BROADCAST OK] Rank: {} | CU: {} | Price: {} | SEU: 0x{:016X} | Sig: {}",
                                rank, cu_limit, price, seu.0, sig
                            );
                            let rec = ProbeLogRecord {
                                timestamp: now_unix_secs(),
                                cluster_slot: initial_slot,
                                burst_id,
                                axis: axis.clone(),
                                send_rank: rank,
                                expected_priority_rank: rank,
                                cu_limit,
                                price_micro_lamports: price,
                                probe_word_hex: format!("0x{:016X}", seu.0),
                                signature: sig,
                                simulated_cu: None,
                            };
                            writeln!(log_file, "{}", serde_json::to_string(&rec)?)?;
                        }
                        Err(e) => {
                            eprintln!("[BROADCAST ERR] Rank: {} | Error: {}", rank, e);
                        }
                    }
                }
                println!("Live probe batch logged to {}", log_file_path);
            }
        }

        "verify" => {
            println!("--- [BLOCK VERIFICATION] Slot: {} ---", verify_slot);
            let block = rpc_call(
                &rpc_url,
                "getBlock",
                json!([
                    verify_slot,
                    {
                        "encoding": "jsonParsed",
                        "transactionDetails": "full",
                        "rewards": false,
                        "maxSupportedTransactionVersion": 1
                    }
                ]),
            )?;
            let txs = block["transactions"]
                .as_array()
                .ok_or("Missing transactions array")?;
            println!("Total transactions in block {}: {}", verify_slot, txs.len());

            for (idx, tx) in txs.iter().enumerate() {
                let tx_str = tx.to_string();
                if let Some(seu) = seu_from_memo(&tx_str) {
                    let status = ((seu.0 >> 8) & 0xFF) as u8;
                    println!(
                        "  [Block Index: {:04}/{}] Burst: {} | Rank: {} | Axis: {} | SlotLow16: {} | Parity: 0x{:02X} | SEU: 0x{:016X}",
                        idx,
                        txs.len(),
                        seu.current_node(),
                        seu.target_node(),
                        status,
                        seu.last_slot(),
                        seu.parity_digest(),
                        seu.0
                    );
                }
            }
        }

        "analyze" => {
            println!("--- [u64se AUTOMATED BLOCK ORDER ANALYSIS] ---");
            fs::create_dir_all(&telemetry_dir)?;
            let log_path = format!("{}/probes.jsonl", telemetry_dir);
            let mut live_records = Vec::new();

            if let Ok(file) = File::open(&log_path) {
                for line in BufReader::new(file).lines().flatten() {
                    if let Ok(rec) = serde_json::from_str::<ProbeLogRecord>(&line) {
                        if !rec.signature.is_empty() && rec.signature != "DRY_RUN_SIMULATION" {
                            live_records.push(rec);
                        }
                    }
                }
            }
            println!(
                "Loaded {} live probe records from {}",
                live_records.len(),
                log_path
            );

            let mut node_versions: HashMap<String, String> = HashMap::new();
            if let Ok(nodes) = rpc_call(&rpc_url, "getClusterNodes", json!([])) {
                if let Some(arr) = nodes.as_array() {
                    for n in arr {
                        if let Some(pk) = n["pubkey"].as_str() {
                            let ver = n["version"].as_str().unwrap_or("unknown").to_string();
                            node_versions.insert(pk.to_string(), ver);
                        }
                    }
                }
            }

            let mut verified_rows = Vec::new();
            let mut slot_cache: HashMap<u64, Vec<String>> = HashMap::new();
            let mut leader_cache: HashMap<u64, String> = HashMap::new();

            for r in &live_records {
                let tx_info = match rpc_call(
                    &rpc_url,
                    "getTransaction",
                    json!([r.signature, {"encoding": "json", "maxSupportedTransactionVersion": 1}]),
                ) {
                    Ok(v) if !v.is_null() => v,
                    _ => continue,
                };

                let slot = tx_info["slot"].as_u64().unwrap_or(0);
                let fee = tx_info["meta"]["fee"].as_u64().unwrap_or(5000);

                let block_sigs = if let Some(cached) = slot_cache.get(&slot) {
                    cached.clone()
                } else {
                    let b = rpc_call(
                        &rpc_url,
                        "getBlock",
                        json!([
                            slot,
                            {
                                "encoding": "json",
                                "transactionDetails": "signatures",
                                "rewards": false,
                                "maxSupportedTransactionVersion": 1
                            }
                        ]),
                    )?;
                    let sigs: Vec<String> = b["signatures"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    slot_cache.insert(slot, sigs.clone());
                    sigs
                };

                let tx_index = block_sigs
                    .iter()
                    .position(|s| s == &r.signature)
                    .map(|p| p as i64)
                    .unwrap_or(-1);
                let total_txs = block_sigs.len();
                let rel_pos = if total_txs > 0 && tx_index >= 0 {
                    ((tx_index as f64) / (total_txs as f64) * 10000.0).round() / 10000.0
                } else {
                    0.0
                };

                let leader = if let Some(l) = leader_cache.get(&slot) {
                    l.clone()
                } else {
                    let mut found_leader = "unknown".to_string();
                    if let Ok(sched) = rpc_call(&rpc_url, "getLeaderSchedule", json!([slot])) {
                        let rel_slot = slot % 432_000;
                        if let Some(obj) = sched.as_object() {
                            for (lpk, slist) in obj {
                                if let Some(arr) = slist.as_array() {
                                    if arr.iter().any(|x| x.as_u64() == Some(rel_slot)) {
                                        found_leader = lpk.clone();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    leader_cache.insert(slot, found_leader.clone());
                    found_leader
                };

                let client_version = node_versions
                    .get(&leader)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                println!(
                    "Rank {} | Slot {} | Pos: {:03}/{} ({:5.1}%) | Fee: {:6} L | Leader: {:.10}... | Client: {}",
                    r.send_rank,
                    slot,
                    tx_index,
                    total_txs,
                    rel_pos * 100.0,
                    fee,
                    leader,
                    client_version
                );

                verified_rows.push(VerifiedProbeRow {
                    signature: r.signature.clone(),
                    slot,
                    burst_id: r.burst_id,
                    axis: r.axis.clone(),
                    send_rank: r.send_rank,
                    cu_limit: r.cu_limit,
                    price_micro_lamports: r.price_micro_lamports,
                    fee_lamports: fee,
                    block_tx_index: tx_index,
                    total_txs_in_block: total_txs,
                    relative_position: rel_pos,
                    leader,
                    client_version,
                    seu_register_hex: r.probe_word_hex.clone(),
                });
            }

            let mut by_slot: HashMap<u64, Vec<&VerifiedProbeRow>> = HashMap::new();
            for row in &verified_rows {
                by_slot.entry(row.slot).or_default().push(row);
            }
            for (sl, group) in &by_slot {
                if group.len() >= 2 {
                    let send_ranks: Vec<i64> = group.iter().map(|g| g.send_rank as i64).collect();
                    let block_indices: Vec<i64> = group.iter().map(|g| g.block_tx_index).collect();
                    let tau = kendall_tau(&send_ranks, &block_indices);
                    println!(
                        "[INTRA-BLOCK CORRELATION] Slot {} ({} probes, client {}): Kendall tau(SendRank vs BlockIndex) = {:.3} (negative = priority inversion beat FIFO!)",
                        sl,
                        group.len(),
                        group[0].client_version,
                        tau
                    );
                }
            }

            let out_json_path = format!("{}/verified_matrix.json", telemetry_dir);
            fs::write(&out_json_path, serde_json::to_string_pretty(&verified_rows)?)?;
            println!(
                "Saved {} verified Mainnet records to {}",
                verified_rows.len(),
                out_json_path
            );
        }

        _ => {
            print_help();
        }
    }

    Ok(())
}
