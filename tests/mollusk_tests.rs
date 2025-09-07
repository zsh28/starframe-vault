use mollusk_svm::{result::Check, Mollusk};
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_program as system_program;

const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("GxpAtbXpkbDu5b86TidcmuF5RF9UJm821rqJ5W3S4T12");

// Instruction discriminators from IDL
const INITIALIZE_DISCRIMINATOR: [u8; 8] = [0xaf, 0xaf, 0x6d, 0x1f, 0x0d, 0x98, 0x9b, 0xed];
const DEPOSIT_DISCRIMINATOR: [u8; 8] = [0xf2, 0x23, 0xc6, 0x89, 0x52, 0xe1, 0xf2, 0xb6];
const WITHDRAW_DISCRIMINATOR: [u8; 8] = [0xb7, 0x12, 0x46, 0x9c, 0x94, 0x6d, 0xa1, 0x22];
const CLOSE_DISCRIMINATOR: [u8; 8] = [0x62, 0xa5, 0xc9, 0xb1, 0x6c, 0x41, 0xce, 0x60];

// VaultState account discriminator
const VAULT_STATE_DISCRIMINATOR: [u8; 8] = [0xe4, 0xc4, 0x52, 0xa5, 0x62, 0xd2, 0xeb, 0x98];

// PDA Seeds
const STATE_SEED: &[u8] = b"STATE";
const VAULT_SEED: &[u8] = b"VAULT";

fn create_mollusk() -> Mollusk {
    let mut mollusk = Mollusk::default();
    mollusk.add_program(&PROGRAM_ID, "target/deploy/starframe_vault", &mollusk_svm::program::loader_keys::LOADER_V3);
    mollusk
}

fn find_vault_state_pda(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[STATE_SEED, owner.as_ref()], &PROGRAM_ID)
}

fn find_vault_pda(state: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED, state.as_ref()], &PROGRAM_ID)
}

fn create_vault_state_data(owner: &Pubkey, state_bump: u8, vault_bump: u8) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&VAULT_STATE_DISCRIMINATOR);
    data.extend_from_slice(owner.as_ref());
    data.push(state_bump);
    data.push(vault_bump);
    data
}

fn create_initialize_instruction(
    owner: &Pubkey,
    state: &Pubkey,
    vault: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &INITIALIZE_DISCRIMINATOR,
        vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(*state, false),
            AccountMeta::new(*vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}

fn create_deposit_instruction(
    user: &Pubkey,
    vault: &Pubkey,
    vault_state: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut instruction_data = DEPOSIT_DISCRIMINATOR.to_vec();
    instruction_data.extend_from_slice(&amount.to_le_bytes());
    
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(*user, true),
            AccountMeta::new(*vault, false),
            AccountMeta::new(*vault_state, false), // Made writable for ValidatedAccount
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}

fn create_withdraw_instruction(
    user: &Pubkey,
    vault: &Pubkey,
    vault_state: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut instruction_data = WITHDRAW_DISCRIMINATOR.to_vec();
    instruction_data.extend_from_slice(&amount.to_le_bytes());
    
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(*user, true),
            AccountMeta::new(*vault, false),
            AccountMeta::new(*vault_state, false), // Made writable for ValidatedAccount
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}

fn create_close_instruction(
    user: &Pubkey,
    vault: &Pubkey,
    vault_state: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &CLOSE_DISCRIMINATOR,
        vec![
            AccountMeta::new(*user, true),
            AccountMeta::new(*vault, false),
            AccountMeta::new(*vault_state, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}

#[test]
fn test_initialize_vault() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, _) = find_vault_pda(&state_pda);

    let owner_account = Account::new(10_000_000_000, 0, &system_program::id());
    let state_account = Account::default();
    let vault_account = Account::default();

    let instruction = create_initialize_instruction(&owner, &state_pda, &vault_pda);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (owner, owner_account),
        (state_pda, state_account),
        (vault_pda, vault_account),
        (system_program_key, system_program_account),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        &accounts,
        &[
            Check::success(),
            Check::account(&state_pda).data(&create_vault_state_data(&owner, state_bump, find_vault_pda(&state_pda).1)).build(),
            Check::account(&vault_pda).lamports(mollusk.sysvars.rent.minimum_balance(0)).build(),
        ],
    );
}

#[test]
fn test_deposit_to_vault() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);
    let deposit_amount = 5_000_000_000;

    let user_initial_balance = 10_000_000_000;
    let vault_initial_balance = mollusk.sysvars.rent.minimum_balance(0);

    let user_account = Account::new(user_initial_balance, 0, &system_program::id());
    let vault_account = Account::new(vault_initial_balance, 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = create_deposit_instruction(&owner, &vault_pda, &state_pda, deposit_amount);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (owner, user_account),
        (vault_pda, vault_account),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        &accounts,
        &[
            Check::success(),
            Check::account(&owner).lamports(user_initial_balance - deposit_amount).build(),
            Check::account(&vault_pda).lamports(vault_initial_balance + deposit_amount).build(),
        ],
    );
}

#[test]
fn test_withdraw_from_vault() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);
    let withdraw_amount = 3_000_000_000;

    let user_initial_balance = 5_000_000_000;
    let vault_initial_balance = 8_000_000_000;

    let user_account = Account::new(user_initial_balance, 0, &system_program::id());
    let vault_account = Account::new(vault_initial_balance, 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = create_withdraw_instruction(&owner, &vault_pda, &state_pda, withdraw_amount);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (owner, user_account),
        (vault_pda, vault_account),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        &accounts,
        &[
            Check::success(),
            Check::account(&owner).lamports(user_initial_balance + withdraw_amount).build(),
            Check::account(&vault_pda).lamports(vault_initial_balance - withdraw_amount).build(),
        ],
    );
}

#[test]
fn test_close_vault() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);

    let user_initial_balance = 5_000_000_000;
    let vault_balance = 2_000_000_000;

    let user_account = Account::new(user_initial_balance, 0, &system_program::id());
    let vault_account = Account::new(vault_balance, 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_rent = mollusk.sysvars.rent.minimum_balance(vault_state_data.len());
    let vault_state_account = Account {
        lamports: vault_state_rent,
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = create_close_instruction(&owner, &vault_pda, &state_pda);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (owner, user_account),
        (vault_pda, vault_account),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        &accounts,
        &[
            Check::success(),
            Check::account(&owner).lamports(user_initial_balance + vault_balance + vault_state_rent).build(),
            Check::account(&vault_pda).lamports(0).build(),
            Check::account(&state_pda).lamports(0).build(),
        ],
    );
}

#[test]
fn test_deposit_insufficient_funds() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);
    let deposit_amount = 15_000_000_000; // More than user has

    let user_initial_balance = 10_000_000_000;
    let vault_initial_balance = mollusk.sysvars.rent.minimum_balance(0);

    let user_account = Account::new(user_initial_balance, 0, &system_program::id());
    let vault_account = Account::new(vault_initial_balance, 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = create_deposit_instruction(&owner, &vault_pda, &state_pda, deposit_amount);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (owner, user_account),
        (vault_pda, vault_account),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    // This should fail due to insufficient funds
    let result = mollusk.process_instruction(&instruction, &accounts);
    assert!(result.program_result.is_err());
}

#[test]
fn test_unauthorized_deposit() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let unauthorized_user = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);
    let deposit_amount = 1_000_000_000;

    let unauthorized_user_account = Account::new(10_000_000_000, 0, &system_program::id());
    let vault_account = Account::new(mollusk.sysvars.rent.minimum_balance(0), 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = create_deposit_instruction(&unauthorized_user, &vault_pda, &state_pda, deposit_amount);
    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();
    
    let accounts = vec![
        (unauthorized_user, unauthorized_user_account),
        (vault_pda, vault_account),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    // This should fail due to incorrect owner validation
    let result = mollusk.process_instruction(&instruction, &accounts);
    assert!(result.program_result.is_err());
}

#[test]
fn test_full_vault_workflow() {
    let mollusk = create_mollusk();
    
    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);

    let initial_balance = 10_000_000_000;
    let deposit_amount = 5_000_000_000;
    let withdraw_amount = 2_000_000_000;

    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();

    // Step 1: Initialize
    let owner_account = Account::new(initial_balance, 0, &system_program::id());
    let state_account = Account::default();
    let vault_account = Account::default();

    let initialize_instruction = create_initialize_instruction(&owner, &state_pda, &vault_pda);
    let initialize_accounts = vec![
        (owner, owner_account.clone()),
        (state_pda, state_account),
        (vault_pda, vault_account),
        (system_program_key, system_program_account.clone()),
    ];

    let initialize_result = mollusk.process_instruction(&initialize_instruction, &initialize_accounts);
    assert!(initialize_result.program_result.is_ok());

    // Step 2: Deposit
    let vault_rent_balance = mollusk.sysvars.rent.minimum_balance(0);
    let user_balance_after_init = initial_balance - vault_rent_balance;

    let user_account_after_init = Account::new(user_balance_after_init, 0, &system_program::id());
    let vault_account_after_init = Account::new(vault_rent_balance, 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data.clone(),
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let deposit_instruction = create_deposit_instruction(&owner, &vault_pda, &state_pda, deposit_amount);
    let deposit_accounts = vec![
        (owner, user_account_after_init),
        (vault_pda, vault_account_after_init),
        (state_pda, vault_state_account.clone()),
        (system_program_key, system_program_account.clone()),
    ];

    let deposit_result = mollusk.process_instruction(&deposit_instruction, &deposit_accounts);
    assert!(deposit_result.program_result.is_ok());

    // Step 3: Withdraw
    let user_balance_after_deposit = user_balance_after_init - deposit_amount;
    let vault_balance_after_deposit = vault_rent_balance + deposit_amount;

    let user_account_after_deposit = Account::new(user_balance_after_deposit, 0, &system_program::id());
    let vault_account_after_deposit = Account::new(vault_balance_after_deposit, 0, &system_program::id());

    let withdraw_instruction = create_withdraw_instruction(&owner, &vault_pda, &state_pda, withdraw_amount);
    let withdraw_accounts = vec![
        (owner, user_account_after_deposit),
        (vault_pda, vault_account_after_deposit),
        (state_pda, vault_state_account.clone()),
        (system_program_key, system_program_account.clone()),
    ];

    let withdraw_result = mollusk.process_instruction(&withdraw_instruction, &withdraw_accounts);
    assert!(withdraw_result.program_result.is_ok());

    // Step 4: Close
    let user_balance_after_withdraw = user_balance_after_deposit + withdraw_amount;
    let vault_balance_after_withdraw = vault_balance_after_deposit - withdraw_amount;

    let user_account_after_withdraw = Account::new(user_balance_after_withdraw, 0, &system_program::id());
    let vault_account_after_withdraw = Account::new(vault_balance_after_withdraw, 0, &system_program::id());

    let close_instruction = create_close_instruction(&owner, &vault_pda, &state_pda);
    let close_accounts = vec![
        (owner, user_account_after_withdraw),
        (vault_pda, vault_account_after_withdraw),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    let close_result = mollusk.process_instruction(&close_instruction, &close_accounts);
    assert!(close_result.program_result.is_ok());
}

#[test]
fn test_compute_unit_benchmarking() {
    use mollusk_svm_bencher::MolluskComputeUnitBencher;

    solana_logger::setup_with("");

    let owner = Pubkey::new_unique();
    let (state_pda, state_bump) = find_vault_state_pda(&owner);
    let (vault_pda, vault_bump) = find_vault_pda(&state_pda);

    let mollusk = create_mollusk();

    let (system_program_key, system_program_account) = mollusk_svm::program::keyed_account_for_system_program();

    // Initialize benchmark
    let owner_account = Account::new(10_000_000_000, 0, &system_program::id());
    let state_account = Account::default();
    let vault_account = Account::default();

    let initialize_instruction = create_initialize_instruction(&owner, &state_pda, &vault_pda);
    let initialize_accounts = vec![
        (owner, owner_account),
        (state_pda, state_account),
        (vault_pda, vault_account),
        (system_program_key, system_program_account.clone()),
    ];

    // Deposit benchmark
    let user_account = Account::new(8_000_000_000, 0, &system_program::id());
    let vault_account_with_rent = Account::new(mollusk.sysvars.rent.minimum_balance(0), 0, &system_program::id());
    
    let vault_state_data = create_vault_state_data(&owner, state_bump, vault_bump);
    let vault_state_account = Account {
        lamports: mollusk.sysvars.rent.minimum_balance(vault_state_data.len()),
        data: vault_state_data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let deposit_instruction = create_deposit_instruction(&owner, &vault_pda, &state_pda, 1_000_000_000);
    let deposit_accounts = vec![
        (owner, user_account),
        (vault_pda, vault_account_with_rent),
        (state_pda, vault_state_account.clone()),
        (system_program_key, system_program_account.clone()),
    ];

    // Withdraw benchmark
    let user_account_withdraw = Account::new(5_000_000_000, 0, &system_program::id());
    let vault_account_withdraw = Account::new(3_000_000_000, 0, &system_program::id());

    let withdraw_instruction = create_withdraw_instruction(&owner, &vault_pda, &state_pda, 500_000_000);
    let withdraw_accounts = vec![
        (owner, user_account_withdraw),
        (vault_pda, vault_account_withdraw),
        (state_pda, vault_state_account.clone()),
        (system_program_key, system_program_account.clone()),
    ];

    // Close benchmark
    let user_account_close = Account::new(5_000_000_000, 0, &system_program::id());
    let vault_account_close = Account::new(2_000_000_000, 0, &system_program::id());

    let close_instruction = create_close_instruction(&owner, &vault_pda, &state_pda);
    let close_accounts = vec![
        (owner, user_account_close),
        (vault_pda, vault_account_close),
        (state_pda, vault_state_account),
        (system_program_key, system_program_account),
    ];

    MolluskComputeUnitBencher::new(mollusk)
        .bench(("initialize_vault", &initialize_instruction, &initialize_accounts))
        .bench(("deposit_1_sol", &deposit_instruction, &deposit_accounts))
        .bench(("withdraw_0.5_sol", &withdraw_instruction, &withdraw_accounts))
        .bench(("close_vault_with_2_sol", &close_instruction, &close_accounts))
        .must_pass(true)
        .out_dir("benches/results")
        .execute();
}