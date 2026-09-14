use {
    mollusk_svm::Mollusk,
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
};

fn main() {
    if std::env::var("SBF_OUT_DIR").is_err() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let deploy_dir = manifest_dir
            .parent()
            .map(|p| p.join("target/deploy"))
            .unwrap_or_else(|| std::path::PathBuf::from("../target/deploy"));
        std::env::set_var("SBF_OUT_DIR", deploy_dir);
    }

    let program_id = Pubkey::new_unique();
    let mut mollusk = Mollusk::new(&program_id, "u64se_seu");
    mollusk.warp_to_slot(100);

    let authority = Pubkey::new_unique();
    let (state_pda, bump) =
        Pubkey::find_program_address(&[b"u64se-seu", authority.as_ref()], &program_id);

    // Initial 8-byte empty state for genesis
    let initial_data = vec![0u8; 8];
    let lamports = mollusk.sysvars.rent.minimum_balance(8);
    let mut state_account = Account::new(lamports, 8, &program_id);
    state_account.data = initial_data;

    let authority_account = Account::new(100_000_000, 0, &Pubkey::default());

    // -------------------------------------------------------------------------
    // STEP 1: GENESIS ACTIVATION (Tag = 0, role = ROLE_SWAP (1), target = S1)
    // -------------------------------------------------------------------------
    let genesis_ix_data = vec![
        bump, // bump passed by client (O(1) PDA verification)
        0,    // TAG_GENESIS = 0
        1,    // role = ROLE_SWAP (allowed_roles_mask)
        1,    // target_node = S1
        0,    // config_flags = 0 (canonical DAG: FLAG_ACYCLIC | FLAG_NO_SELF_LOOP)
    ];

    let genesis_ix = Instruction::new_with_bytes(
        program_id,
        &genesis_ix_data,
        vec![
            AccountMeta::new(state_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let accounts_genesis = [
        (state_pda, state_account.clone()),
        (authority, authority_account.clone()),
    ];

    let result_genesis = mollusk.process_instruction(&genesis_ix, &accounts_genesis);

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║   [STEP 1]: GENESIS ACTIVATION (Clock::get(), bump, return_data) ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!(
        "  Compute Units     : {} CU",
        result_genesis.compute_units_consumed
    );
    println!("  Execution Status  : {:?}", result_genesis.program_result);
    println!("  Return Data Hex   : {:02x?}", result_genesis.return_data);

    assert!(result_genesis.program_result.is_ok());

    // Update state account data with resulting genesis output
    state_account = result_genesis.resulting_accounts[0].1.clone();
    let state_after_genesis = u64::from_le_bytes(state_account.data[0..8].try_into().unwrap());
    println!("  Active Register   : 0x{:016x}\n", state_after_genesis);

    // -------------------------------------------------------------------------
    // STEP 2: TRANSITION S0 -> S1 (Tag = 1, role = ROLE_SWAP (1), target = S1)
    // -------------------------------------------------------------------------
    mollusk.warp_to_slot(105); // Advance 5 slots on-chain

    let transition_ix_data = vec![
        bump, // bump
        1,    // TAG_TRANSITION = 1
        1,    // role = ROLE_SWAP
        1,    // target_node = S1
        2,    // future_target = S2 (explicit DAG target)
    ];

    let transition_ix = Instruction::new_with_bytes(
        program_id,
        &transition_ix_data,
        vec![
            AccountMeta::new(state_pda, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let accounts_transition = [(state_pda, state_account), (authority, authority_account)];

    let result_transition = mollusk.process_instruction(&transition_ix, &accounts_transition);

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║   [STEP 2]: FSM TRANSITION S1 -> S2 (Canonical Clock, O(1) Bump) ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!(
        "  Compute Units     : {} CU",
        result_transition.compute_units_consumed
    );
    println!(
        "  Execution Status  : {:?}",
        result_transition.program_result
    );
    println!(
        "  Return Data Hex   : {:02x?}",
        result_transition.return_data
    );

    let final_state_acc = &result_transition.resulting_accounts[0].1;
    let final_register = u64::from_le_bytes(final_state_acc.data[0..8].try_into().unwrap());
    println!("  Next Register     : 0x{:016x}", final_register);
    println!("════════════════════════════════════════════════════════════════════");

    assert!(result_transition.program_result.is_ok());
}
