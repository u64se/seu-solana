use {
    mollusk_svm::{result::ProgramResult as MolluskResult, Mollusk},
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

pub struct FaultMask;
impl FaultMask {
    pub const SELF_LOOP_FORBIDDEN: u32 = 1 << 3; // 0x08
    pub const BACKWARD_FORBIDDEN: u32 = 1 << 4;  // 0x10
}

fn main() {
    if std::env::var("SBF_OUT_DIR").is_err() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let deploy_dir = manifest_dir
            .parent()
            .map(|p| p.join("target/deploy"))
            .unwrap_or_else(|| std::path::PathBuf::from("../target/deploy"));
        std::env::set_var("SBF_OUT_DIR", deploy_dir);
    }

    println!("u64se-seu: mollusk sbf verification against target/deploy/u64se_seu.so");

    let program_id = Pubkey::new_unique();
    let mut mollusk = Mollusk::new(&program_id, "u64se_seu");
    mollusk.warp_to_slot(100);

    let authority = Pubkey::new_unique();
    let (canonical_pda, bump) =
        Pubkey::find_program_address(&[b"u64se-seu", authority.as_ref()], &program_id);

    let authority_account = Account::new(100_000_000, 0, &Pubkey::default());
    let lamports = mollusk.sysvars.rent.minimum_balance(8);

    // -------------------------------------------------------------------------
    // TEST 1: Real Genesis Activation in SVM
    // -------------------------------------------------------------------------
    println!("[TEST 1]: Executing Genesis Activation in Mollusk SBF...");
    let mut state_account = Account::new(lamports, 8, &program_id);
    state_account.data = vec![0u8; 8];

    let genesis_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 0, 1, 1, 0], // bump, TAG_GENESIS, ROLE_SWAP, S1, default_flags
        vec![
            AccountMeta::new(canonical_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let res_genesis = mollusk.process_instruction(
        &genesis_ix,
        &[
            (canonical_pda, state_account.clone()),
            (authority, authority_account.clone()),
        ],
    );

    assert!(res_genesis.program_result.is_ok(), "Genesis must succeed");
    let cu_genesis = res_genesis.compute_units_consumed;
    let genesis_state = res_genesis.resulting_accounts[0].1.clone();
    let reg_genesis = u64::from_le_bytes(genesis_state.data[0..8].try_into().unwrap());
    println!("  -> PASS: Genesis consumed {} CU | Register: 0x{:016x}\n", cu_genesis, reg_genesis);

    // -------------------------------------------------------------------------
    // TEST 2: Real State Transition S1 -> S2 in SVM
    // -------------------------------------------------------------------------
    println!("[TEST 2]: Executing Transition S1 -> S2 in Mollusk SBF...");
    mollusk.warp_to_slot(105);

    let transition_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 1, 1, 1, 2], // bump, TAG_TRANSITION, ROLE_SWAP, target=S1, future=S2
        vec![
            AccountMeta::new(canonical_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let res_transition = mollusk.process_instruction(
        &transition_ix,
        &[
            (canonical_pda, genesis_state.clone()),
            (authority, authority_account.clone()),
        ],
    );

    assert!(res_transition.program_result.is_ok(), "Transition must succeed");
    let cu_trans = res_transition.compute_units_consumed;
    let trans_state = res_transition.resulting_accounts[0].1.clone();
    let reg_trans = u64::from_le_bytes(trans_state.data[0..8].try_into().unwrap());
    println!("  -> PASS: Transition consumed {} CU | Next Register: 0x{:016x}\n", cu_trans, reg_trans);

    // -------------------------------------------------------------------------
    // TEST 3: Real Re-entrancy Self-Loop Attack Interception (S1 -> S1)
    // -------------------------------------------------------------------------
    println!("[TEST 3]: Firing Self-Loop Attack into SBF runtime...");
    let self_loop_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 1, 1, 1, 1], // future_target = 1 (violates FLAG_NO_SELF_LOOP)
        vec![
            AccountMeta::new(canonical_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let res_loop = mollusk.process_instruction(
        &self_loop_ix,
        &[
            (canonical_pda, genesis_state.clone()),
            (authority, authority_account.clone()),
        ],
    );

    let expected_code = FaultMask::SELF_LOOP_FORBIDDEN as u32; // 0x08
    assert_eq!(
        res_loop.program_result,
        MolluskResult::Failure(ProgramError::Custom(expected_code)),
        "Self-loop must be rejected with Custom(0x08)"
    );
    println!("  -> PASS: Self-loop rejected by SBF with ProgramError::Custom(0x{:02x})\n", expected_code);

    // -------------------------------------------------------------------------
    // TEST 4: Real DAG Backward Jump Attack Interception (S2 -> S1)
    // -------------------------------------------------------------------------
    println!("[TEST 4]: Firing Backward DAG Jump into SBF runtime...");
    let backward_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 1, 1, 2, 1], // current S2 -> target S1 (violates FLAG_ACYCLIC)
        vec![
            AccountMeta::new(canonical_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let res_backward = mollusk.process_instruction(
        &backward_ix,
        &[
            (canonical_pda, trans_state.clone()),
            (authority, authority_account.clone()),
        ],
    );

    let expected_backward_code = FaultMask::BACKWARD_FORBIDDEN as u32; // 0x10
    assert_eq!(
        res_backward.program_result,
        MolluskResult::Failure(ProgramError::Custom(expected_backward_code)),
        "Backward jump must be rejected with Custom(0x10)"
    );
    println!("  -> PASS: Backward jump rejected by SBF with ProgramError::Custom(0x{:02x})\n", expected_backward_code);

    // -------------------------------------------------------------------------
    // TEST 5: Real Unauthorized Signer Security Check
    // -------------------------------------------------------------------------
    println!("[TEST 5]: Firing Unsigned Transaction into SBF runtime...");
    let unsigned_ix = Instruction::new_with_bytes(
        program_id,
        &[bump, 0, 1, 1, 0],
        vec![
            AccountMeta::new(canonical_pda, false),
            AccountMeta::new_readonly(authority, false), // ATTACK: signer = false
        ],
    );

    let res_unsigned = mollusk.process_instruction(
        &unsigned_ix,
        &[
            (canonical_pda, state_account.clone()),
            (authority, authority_account.clone()),
        ],
    );

    assert_eq!(
        res_unsigned.program_result,
        MolluskResult::Failure(ProgramError::MissingRequiredSignature)
    );
    println!("  ok: missing signature rejected\n");
    println!("ok: all onchain checks passed against u64se_seu.so");
}
