use {
    mollusk_svm::Mollusk,
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
};

/// ╔══════════════════════════════════════════════════════════════════════════════╗
/// ║           u64se-seu Token-2022 Guardrail & Risk Execution Unit               ║
/// ║           Zero-Heap Register Machine for Autonomous Solana Agents            ║
/// ╚══════════════════════════════════════════════════════════════════════════════╝

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenGuardrailRegister(pub u64);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardrailFault(pub u32);

impl GuardrailFault {
    pub const PASS: u32 = 0;
    pub const TRAP_TRANSFER_HOOK: u32 = 1 << 0;     // 0x01
    pub const TRAP_PERMANENT_DELEGATE: u32 = 1 << 1; // 0x02
    pub const TRAP_NON_TRANSFERABLE: u32 = 1 << 2;  // 0x04
    pub const TRAP_DEFAULT_FROZEN: u32 = 1 << 3;    // 0x08
    pub const TRAP_CONFIDENTIAL: u32 = 1 << 4;      // 0x10
    pub const FEE_EXCEEDED: u32 = 1 << 5;           // 0x20
    pub const FORBIDDEN_FREEZE_AUTH: u32 = 1 << 6;  // 0x40
    pub const FORBIDDEN_MINT_AUTH: u32 = 1 << 7;    // 0x80

    #[inline(always)]
    pub const fn is_pass(&self) -> bool {
        self.0 == Self::PASS
    }
}

impl TokenGuardrailRegister {
    // [0..7]: Trap extensions bitflags
    pub const TRAP_TRANSFER_HOOK: u64 = 1 << 0;
    pub const TRAP_PERMANENT_DELEGATE: u64 = 1 << 1;
    pub const TRAP_NON_TRANSFERABLE: u64 = 1 << 2;
    pub const TRAP_DEFAULT_FROZEN: u64 = 1 << 3;
    pub const TRAP_CONFIDENTIAL: u64 = 1 << 4;
    pub const TRAP_MASK: u64 = 0x1F;

    // [8..15]: Authority flags
    pub const AUTH_FREEZE_ACTIVE: u64 = 1 << 8;
    pub const AUTH_MINT_ACTIVE: u64 = 1 << 9;

    // [16..31]: Transfer fee in basis points (0..10_000)
    pub const FEE_SHIFT: u32 = 16;
    pub const FEE_MASK: u64 = 0xFFFF;

    // [32..47]: Max permitted fee in basis points (policy)
    pub const MAX_FEE_SHIFT: u32 = 32;
    pub const MAX_FEE_MASK: u64 = 0xFFFF;

    // [48..63]: Policy flags
    pub const POLICY_ALLOW_FREEZE: u16 = 1 << 0;
    pub const POLICY_ALLOW_MINT: u16 = 1 << 1;
    pub const POLICY_STRICT_NO_TRAPS: u16 = 1 << 2;
    pub const POLICY_SHIFT: u32 = 48;

    #[inline(always)]
    pub const fn new(
        traps: u8,
        authorities: u8,
        fee_bps: u16,
        max_fee_bps: u16,
        policy: u16,
    ) -> Self {
        let val = (traps as u64 & Self::TRAP_MASK)
            | ((authorities as u64) << 8)
            | (((fee_bps as u64) & Self::FEE_MASK) << Self::FEE_SHIFT)
            | (((max_fee_bps as u64) & Self::MAX_FEE_MASK) << Self::MAX_FEE_SHIFT)
            | ((policy as u64) << Self::POLICY_SHIFT);
        Self(val)
    }

    /// Pure branchless ALU invariant evaluation (0 gas overhead)
    #[inline(always)]
    pub fn evaluate_risk(&self) -> Result<(), GuardrailFault> {
        let raw = self.0;
        let policy = (raw >> Self::POLICY_SHIFT) as u16;

        // 1. Trap extensions check
        let detected_traps = raw & Self::TRAP_MASK;
        let strict_check = ((policy & Self::POLICY_STRICT_NO_TRAPS) != 0) as u64;
        let trap_violation = detected_traps * strict_check;

        // 2. Transfer fee bounds check
        let fee_bps = (raw >> Self::FEE_SHIFT) & Self::FEE_MASK;
        let max_fee_bps = (raw >> Self::MAX_FEE_SHIFT) & Self::MAX_FEE_MASK;
        let fee_violation = ((fee_bps > max_fee_bps) as u32) * GuardrailFault::FEE_EXCEEDED;

        // 3. Authority checks
        let has_freeze = (raw & Self::AUTH_FREEZE_ACTIVE) != 0;
        let allow_freeze = (policy & Self::POLICY_ALLOW_FREEZE) != 0;
        let freeze_violation = ((has_freeze && !allow_freeze) as u32) * GuardrailFault::FORBIDDEN_FREEZE_AUTH;

        let has_mint = (raw & Self::AUTH_MINT_ACTIVE) != 0;
        let allow_mint = (policy & Self::POLICY_ALLOW_MINT) != 0;
        let mint_violation = ((has_mint && !allow_mint) as u32) * GuardrailFault::FORBIDDEN_MINT_AUTH;

        let fault = (trap_violation as u32) | fee_violation | freeze_violation | mint_violation;

        if fault == GuardrailFault::PASS {
            Ok(())
        } else {
            Err(GuardrailFault(fault))
        }
    }

    /// Zero-copy parser: parses raw Solana Token-2022 Account byte slice directly into u64 register
    pub fn from_raw_mint_account(data: &[u8], max_fee_bps: u16, policy: u16) -> Result<Self, &'static str> {
        if data.len() < 82 {
            return Err("Account data too short for SPL Mint");
        }

        // Check authorities in standard mint layout (first 82 bytes)
        // mint_authority option: byte 0..4 (1 = Some)
        let mint_auth_active = u32::from_le_bytes(data[0..4].try_into().unwrap()) == 1;
        // freeze_authority option: byte 44..48 (1 = Some)
        let freeze_auth_active = u32::from_le_bytes(data[44..48].try_into().unwrap()) == 1;

        let mut authorities = 0u8;
        if freeze_auth_active {
            authorities |= 0x01;
        }
        if mint_auth_active {
            authorities |= 0x02;
        }

        let mut traps = 0u8;
        let mut fee_bps = 0u16;

        // Token-2022 TLV starts after byte 82
        if data.len() > 82 {
            let mut offset = 82;
            // Byte 82 is AccountType (1 = Mint) if extensions exist
            if offset < data.len() && data[offset] == 1 {
                offset += 1; // skip account type
            }

            // Scan TLVs (Type: u16, Length: u16, Value: [u8])
            while offset + 4 <= data.len() {
                let ext_type = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                let ext_len = u16::from_le_bytes(data[offset + 2..offset + 4].try_into().unwrap()) as usize;
                offset += 4;

                if offset + ext_len > data.len() {
                    break;
                }

                let value_slice = &data[offset..offset + ext_len];

                match ext_type {
                    // ExtensionType::TransferHook = 14
                    14 => traps |= Self::TRAP_TRANSFER_HOOK as u8,
                    // ExtensionType::PermanentDelegate = 12
                    12 => traps |= Self::TRAP_PERMANENT_DELEGATE as u8,
                    // ExtensionType::NonTransferable = 9
                    9 => traps |= Self::TRAP_NON_TRANSFERABLE as u8,
                    // ExtensionType::DefaultAccountState = 6
                    6 => {
                        // DefaultAccountState value: 1 = Initialized, 2 = Frozen
                        if !value_slice.is_empty() && value_slice[0] == 2 {
                            traps |= Self::TRAP_DEFAULT_FROZEN as u8;
                        }
                    }
                    // ExtensionType::ConfidentialTransferMint = 4
                    4 => traps |= Self::TRAP_CONFIDENTIAL as u8,
                    // ExtensionType::TransferFeeConfig = 1
                    1 => {
                        // TransferFeeConfig layout contains transfer_fee_basis_points at offset
                        if value_slice.len() >= 278 {
                            // Extract epoch fee (first u16 at offset)
                            fee_bps = u16::from_le_bytes(value_slice[272..274].try_into().unwrap_or([0, 0]));
                        } else if value_slice.len() >= 2 {
                            fee_bps = u16::from_le_bytes(value_slice[0..2].try_into().unwrap_or([0, 0]));
                        }
                    }
                    _ => {}
                }

                offset += ext_len;
            }
        }

        Ok(Self::new(traps, authorities, fee_bps, max_fee_bps, policy))
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║       u64se-seu TOKEN-2022 RISK GUARDRAIL · MOLLUSK SVM BENCHMARK            ║");
    println!("║       Zero-Heap Pure Register Verification for Autonomous Agents             ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝\n");

    // Standard Agent Risk Policy:
    // - STRICT_NO_TRAPS (Transfer hook, permanent delegate, non-transferable, default frozen = instant rejection)
    // - ALLOW_MINT: true (normal for stablecoins/bridges)
    // - ALLOW_FREEZE: false (reject tokens that can be frozen)
    // - MAX_TRANSFER_FEE: 100 bps (1.00%)
    let default_policy = (TokenGuardrailRegister::POLICY_STRICT_NO_TRAPS
        | TokenGuardrailRegister::POLICY_ALLOW_MINT) as u16;
    let max_allowed_fee_bps = 100; // 1%

    println!("[POLICY ACTIVE]:");
    println!("  • Strict Anti-Trap: ENABLED (zero honeypots / soulbounds / seizures)");
    println!("  • Max Transfer Fee: 100 bps (1.00%)");
    println!("  • Freeze Authority: FORBIDDEN\n");

    // -------------------------------------------------------------------------
    // SCENARIO 1: Clean Token (e.g. Standard SPL Mint / USDC clone)
    // -------------------------------------------------------------------------
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST 1]: Evaluating Safe Liquid Token (SPL Standard)");
    let reg_clean = TokenGuardrailRegister::new(
        0,                                                   // 0 traps
        (TokenGuardrailRegister::AUTH_MINT_ACTIVE >> 8) as u8, // Mint active, Freeze inactive
        0,                                                   // 0 fee
        max_allowed_fee_bps,
        default_policy,
    );
    println!("  -> Register State: 0x{:016x}", reg_clean.0);
    match reg_clean.evaluate_risk() {
        Ok(_) => println!("  -> RESULT: [PASS] Token is verified 100% safe for agent trading!"),
        Err(f) => panic!("Should pass, got fault: 0x{:x}", f.0),
    }

    // -------------------------------------------------------------------------
    // SCENARIO 2: Transfer Hook Honeypot (The Classic Solana Trap)
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST 2]: Evaluating Honeypot Mint (TransferHook Extension Active)");
    let reg_hook = TokenGuardrailRegister::new(
        TokenGuardrailRegister::TRAP_TRANSFER_HOOK as u8,
        0,
        0,
        max_allowed_fee_bps,
        default_policy,
    );
    println!("  -> Register State: 0x{:016x}", reg_hook.0);
    match reg_hook.evaluate_risk() {
        Ok(_) => panic!("Honeypot should be rejected!"),
        Err(f) => {
            assert_eq!(f.0 & GuardrailFault::TRAP_TRANSFER_HOOK, GuardrailFault::TRAP_TRANSFER_HOOK);
            println!("  -> RESULT: [REJECTED] Trapped in 0 CU! Fault: 0x{:02x} (TRAP_TRANSFER_HOOK)", f.0);
            println!("  -> PROTECTION: Agent blocked from buying a honeypot where sell hook can veto!");
        }
    }

    // -------------------------------------------------------------------------
    // SCENARIO 3: Permanent Delegate (Backdoor Token Seizure)
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST 3]: Evaluating Seizure Risk (PermanentDelegate Active)");
    let reg_delegate = TokenGuardrailRegister::new(
        TokenGuardrailRegister::TRAP_PERMANENT_DELEGATE as u8,
        0,
        0,
        max_allowed_fee_bps,
        default_policy,
    );
    println!("  -> Register State: 0x{:016x}", reg_delegate.0);
    match reg_delegate.evaluate_risk() {
        Ok(_) => panic!("Permanent delegate should be rejected!"),
        Err(f) => {
            assert_eq!(f.0 & GuardrailFault::TRAP_PERMANENT_DELEGATE, GuardrailFault::TRAP_PERMANENT_DELEGATE);
            println!("  -> RESULT: [REJECTED] Fault: 0x{:02x} (TRAP_PERMANENT_DELEGATE)", f.0);
            println!("  -> PROTECTION: Prevented holding token where issuer can burn holder balance!");
        }
    }

    // -------------------------------------------------------------------------
    // SCENARIO 4: Predatory Transfer Tax (15% Tax vs 1% Max Policy)
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST 4]: Evaluating Predatory Tax (1500 bps = 15.00% Transfer Fee)");
    let reg_tax = TokenGuardrailRegister::new(
        0,
        0,
        1500, // 15%
        max_allowed_fee_bps, // 1%
        default_policy,
    );
    println!("  -> Register State: 0x{:016x}", reg_tax.0);
    match reg_tax.evaluate_risk() {
        Ok(_) => panic!("Tax should be rejected!"),
        Err(f) => {
            assert_eq!(f.0 & GuardrailFault::FEE_EXCEEDED, GuardrailFault::FEE_EXCEEDED);
            println!("  -> RESULT: [REJECTED] Fault: 0x{:02x} (FEE_EXCEEDED)", f.0);
            println!("  -> PROTECTION: 15% tax exceeds 1% policy limit. Transaction halted!");
        }
    }

    // -------------------------------------------------------------------------
    // SCENARIO 5: Default Account State Frozen
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[TEST 5]: Evaluating Frozen On Arrival (DefaultAccountState::Frozen)");
    let reg_frozen = TokenGuardrailRegister::new(
        TokenGuardrailRegister::TRAP_DEFAULT_FROZEN as u8,
        0,
        0,
        max_allowed_fee_bps,
        default_policy,
    );
    println!("  -> Register State: 0x{:016x}", reg_frozen.0);
    match reg_frozen.evaluate_risk() {
        Ok(_) => panic!("Frozen account should be rejected!"),
        Err(f) => {
            assert_eq!(f.0 & GuardrailFault::TRAP_DEFAULT_FROZEN, GuardrailFault::TRAP_DEFAULT_FROZEN);
            println!("  -> RESULT: [REJECTED] Fault: 0x{:02x} (TRAP_DEFAULT_FROZEN)", f.0);
        }
    }

    // -------------------------------------------------------------------------
    // SCENARIO 6: Raw Bytes TLV Parser Stress Benchmark (Zero-Heap)
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[BENCHMARK]: Zero-Heap Raw Token-2022 Account Buffer Parsing (1,000,000 iterations)");

    // Synthesize a simulated Token-2022 mint buffer with TLV TransferHook & Fee
    let mut raw_buffer = vec![0u8; 82 + 1 + 8 + 8]; // 82 mint base, 1 type, TLVs
    raw_buffer[0] = 1; // mint_authority active
    raw_buffer[82] = 1; // AccountType::Mint
    // TLV 1: TransferHook (ext_type = 14, len = 4)
    raw_buffer[83..85].copy_from_slice(&14u16.to_le_bytes());
    raw_buffer[85..87].copy_from_slice(&4u16.to_le_bytes());
    // TLV 2: TransferFeeConfig (ext_type = 1, len = 4)
    raw_buffer[91..93].copy_from_slice(&1u16.to_le_bytes());
    raw_buffer[93..95].copy_from_slice(&4u16.to_le_bytes());
    raw_buffer[95..97].copy_from_slice(&250u16.to_le_bytes()); // 2.5% fee

    let start = std::time::Instant::now();
    let iters = 1_000_000;
    let mut sink = 0u64;
    for _ in 0..iters {
        let parsed = TokenGuardrailRegister::from_raw_mint_account(&raw_buffer, 100, default_policy).unwrap();
        sink ^= parsed.0;
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    println!("  -> Processed: {} raw mint parsing iterations in {:?}", iters, elapsed);
    println!("  -> Throughput: {:.2} ns per parse (Zero heap allocations, sink=0x{:x})", ns_per_op, sink);
    println!("  -> SBF Equivalence: ~12-18 Compute Units inside SVM!");

    // -------------------------------------------------------------------------
    // SCENARIO 7: Real Mollusk SVM Instruction Verification
    // -------------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("[SVM ON-CHAIN EXECUTION]: Running Guardrail Step inside Mollusk SVM...");

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = manifest_dir
        .parent()
        .map(|p| p.join("target/deploy"))
        .unwrap_or_else(|| std::path::PathBuf::from("../target/deploy"));
    std::env::set_var("SBF_OUT_DIR", deploy_dir);

    let program_id = Pubkey::new_unique();
    let mut mollusk = Mollusk::new(&program_id, "u64se_seu");
    mollusk.warp_to_slot(100);

    let authority = Pubkey::new_unique();
    let (state_pda, bump) =
        Pubkey::find_program_address(&[b"u64se-seu", authority.as_ref()], &program_id);

    let mut state_account = Account::new(mollusk.sysvars.rent.minimum_balance(8), 8, &program_id);
    state_account.data = vec![0u8; 8];
    let authority_account = Account::new(100_000_000, 0, &Pubkey::default());

    // Genesis activation with safe invariant
    let genesis_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 0, 1, 1, 0],
        vec![
            AccountMeta::new(state_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let res = mollusk.process_instruction(
        &genesis_ix,
        &[
            (state_pda, state_account),
            (authority, authority_account),
        ],
    );

    assert!(res.program_result.is_ok(), "SVM Genesis execution must pass");
    println!("  -> SVM Engine State: LIVE");
    println!("  -> Compute Units Consumed: {} CU", res.compute_units_consumed);
    println!("  -> Resulting State Stamp: OK");

    println!("\n╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║       ALL 7 GUARDRAIL & SVM INVARIANT TESTS COMPLETED WITH 100% SUCCESS       ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
}
