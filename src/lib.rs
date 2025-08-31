use std::borrow::BorrowMut;
use star_frame::{anyhow::ensure, cpi::{invoke, invoke_signed}, pinocchio::sysvars::{rent::Rent, Sysvar}, prelude::*, solana_instruction::Instruction};

#[derive(StarFrameProgram)]
#[program(
    instruction_set = VaultIxSet,
    id = "GxpAtbXpkbDu5b86TidcmuF5RF9UJm821rqJ5W3S4T12"
)]
pub struct VaultProgram;

#[derive(InstructionSet)]
pub enum VaultIxSet {
    Initialize(InitializeIx),
    Deposit(DepositIx),
    Withdraw(WithdrawIx),
    Close(CloseIx),
}

/* -------------------- PDA Seeds -------------------- */

#[derive(Debug, GetSeeds, Clone)]
#[get_seeds(seed_const = b"STATE")]
pub struct VaultStateSeeds {
    pub owner: Pubkey,
}

#[derive(Debug, GetSeeds, Clone)]
#[get_seeds(seed_const = b"VAULT")]
pub struct VaultSeeds {
    pub state: Pubkey,
}

/* -------------------- Program Account -------------------- */

#[derive(Align1, Pod, Zeroable, Default, Copy, Clone, Debug, Eq, PartialEq, ProgramAccount)]
#[program_account(seeds = VaultStateSeeds)]
#[repr(C, packed)]
pub struct VaultState {
    pub owner: Pubkey,
    pub state_bump: u8,
    pub vault_bump: u8,
}

impl VaultState {
    pub const INIT_SPACE: usize = 32 + 1 + 1; // Pubkey + 2 bumps
}

/* Let the account validate itself */
impl AccountValidate<&Pubkey> for VaultState {
    fn validate_account(self_ref: &Self::Ref<'_>, owner: &Pubkey) -> Result<()> {
        ensure!(self_ref.owner == *owner, "Incorrect owner");
        Ok(())
    }
}

/* -------------------- Initialize -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct InitializeIx;

#[derive(AccountSet)]
pub struct InitializeAccounts {
    #[validate(funder)]
    pub owner: Signer<Mut<SystemAccount>>,

    // Program-owned state account
    #[validate(arg = (
        Create(()),
        Seeds(VaultStateSeeds { owner: *self.owner.pubkey() }),
    ))]
    pub state: Init<Seeded<Account<VaultState>>>,

    // System-owned vault PDA for storing lamports - no seeds validation needed for creation
    pub vault: Mut<SystemAccount>,

    pub system_program: Program<System>,
}

impl StarFrameInstruction for InitializeIx {
    type ReturnType = ();
    type Accounts<'b, 'c> = InitializeAccounts;

    fn process(
        a: &mut Self::Accounts<'_, '_>,
        _run: Self::RunArg<'_>,
        _ctx: &mut Context,
    ) -> Result<()> {
        // Derive the vault PDA manually
        let vault_seeds = VaultSeeds { state: *a.state.pubkey() };
        let seeds = vault_seeds.seeds();
        let (vault_pda, vault_bump) = Pubkey::find_program_address(&seeds, &crate::ID);
        
        // Ensure the provided vault account matches the expected PDA
        ensure!(*a.vault.pubkey() == vault_pda, "Invalid vault PDA");

        // Get rent exemption amount for the vault (0 data bytes for SystemAccount)
        let rent = Rent::get()?;
        let rent_exempt_lamports = rent.minimum_balance(0);

        // Build system transfer instruction
        let transfer_accounts = [
            a.owner.account_info().clone(),
            a.vault.account_info().clone(),
        ];
        
        let transfer_data = [
            2u8, 0, 0, 0, // System Program Transfer instruction discriminant
        ].iter()
        .chain(rent_exempt_lamports.to_le_bytes().iter())
        .cloned()
        .collect::<Vec<u8>>();

        let transfer_ix = Instruction {
            program_id: System::ID,
            accounts: vec![
                AccountMeta::new(*a.owner.pubkey(), true),
                AccountMeta::new(*a.vault.pubkey(), false),
            ],
            data: transfer_data,
        };

        invoke(&transfer_ix, &transfer_accounts)?;

        // Initialize the state account
        let vault_state = VaultState {
            owner: *a.owner.pubkey(),
            state_bump: a.state.access_seeds().bump,
            vault_bump,
        };
        
        let data = bytemuck::bytes_of(&vault_state);
        a.state.account_data_mut()?.copy_from_slice(data);
        
        Ok(())
    }
}

/* -------------------- Deposit -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct DepositIx {
    #[ix_args(&run)]
    pub amount: u64,
}

#[derive(AccountSet)]
pub struct DepositAccounts {
    #[validate(funder)]
    pub user: Signer<Mut<SystemAccount>>,

    pub vault: Mut<SystemAccount>,

    #[validate(arg = Seeds(VaultStateSeeds { owner: *self.user.pubkey() }))]
    pub vault_state: Seeded<Account<VaultState>, VaultStateSeeds>,

    pub system_program: Program<System>,
}

impl StarFrameInstruction for DepositIx {
    type ReturnType = ();
    type Accounts<'b, 'c> = DepositAccounts;

    fn process(
        a: &mut Self::Accounts<'_, '_>,
        amount: &u64,
        _ctx: &mut Context,
    ) -> Result<()> {
        let amount = *amount;
        
        // Verify vault PDA
        let _vault_state_data = a.vault_state.data()?;
        let vault_seeds = VaultSeeds { state: *a.vault_state.pubkey() };
        let seeds = vault_seeds.seeds();
        let (expected_vault, _) = Pubkey::find_program_address(&seeds, &crate::ID);
        ensure!(*a.vault.pubkey() == expected_vault, "Invalid vault PDA");
        
        // Ensure user has enough lamports
        let user_lamports = a.user.lamports();
        ensure!(user_lamports >= amount, "Insufficient funds");

        // Build system transfer instruction
        let transfer_accounts = [
            a.user.account_info().clone(),
            a.vault.account_info().clone(),
        ];
        
        let transfer_data = [
            2u8, 0, 0, 0, // System Program Transfer instruction discriminant
        ].iter()
        .chain(amount.to_le_bytes().iter())
        .cloned()
        .collect::<Vec<u8>>();

        let transfer_ix = Instruction {
            program_id: System::ID,
            accounts: vec![
                AccountMeta::new(*a.user.pubkey(), true),
                AccountMeta::new(*a.vault.pubkey(), false),
            ],
            data: transfer_data,
        };

        invoke(&transfer_ix, &transfer_accounts)?;

        Ok(())
    }
}

/* -------------------- Withdraw -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct WithdrawIx {
    #[ix_args(&run)]
    pub amount: u64,
}

#[derive(AccountSet)]
pub struct WithdrawAccounts {
    #[validate(funder)]
    pub user: Signer<Mut<SystemAccount>>,

    pub vault: Mut<SystemAccount>,

    #[validate(arg = Seeds(VaultStateSeeds { owner: *self.user.pubkey() }))]
    pub vault_state: Seeded<Account<VaultState>, VaultStateSeeds>,

    pub system_program: Program<System>,
}

impl StarFrameInstruction for WithdrawIx {
    type ReturnType = ();
    type Accounts<'b, 'c> = WithdrawAccounts;

    fn process(
        a: &mut Self::Accounts<'_, '_>,
        amount: &u64,
        _ctx: &mut Context,
    ) -> Result<()> {
        let amount = *amount;
        let vault_state_data = a.vault_state.data()?;
        
        // Verify vault PDA
        let vault_seeds = VaultSeeds { state: *a.vault_state.pubkey() };
        let seeds = vault_seeds.seeds();
        let (expected_vault, _) = Pubkey::find_program_address(&seeds, &crate::ID);
        ensure!(*a.vault.pubkey() == expected_vault, "Invalid vault PDA");
        
        // Check vault has sufficient funds
        let vault_lamports = a.vault.lamports();
        let rent = Rent::get()?;
        let rent_exempt = rent.minimum_balance(0);
        let available_lamports = vault_lamports.saturating_sub(rent_exempt);
        
        ensure!(available_lamports >= amount, "Insufficient vault balance");

        // Create signer seeds for vault PDA
        let bump_seed = [vault_state_data.vault_bump];
        let signer_seeds = [&seeds[..], &[&bump_seed]].concat();

        // Build system transfer instruction with PDA signing
        let transfer_accounts = [
            a.vault.account_info().clone(),
            a.user.account_info().clone(),
        ];
        
        let transfer_data = [
            2u8, 0, 0, 0, // System Program Transfer instruction discriminant
        ].iter()
        .chain(amount.to_le_bytes().iter())
        .cloned()
        .collect::<Vec<u8>>();

        let transfer_ix = Instruction {
            program_id: System::ID,
            accounts: vec![
                AccountMeta::new(*a.vault.pubkey(), true), // vault signs as PDA
                AccountMeta::new(*a.user.pubkey(), false),
            ],
            data: transfer_data,
        };

        invoke_signed(&transfer_ix, &transfer_accounts, &[&signer_seeds])?;

        Ok(())
    }
}

/* -------------------- Close -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct CloseIx;

#[derive(AccountSet)]
pub struct CloseAccounts {
    #[validate(funder)]
    pub user: Signer<Mut<SystemAccount>>,

    pub vault: Mut<SystemAccount>,

    #[validate(arg = Seeds(VaultStateSeeds { owner: *self.user.pubkey() }))]
    pub vault_state: Mut<Seeded<Account<VaultState>, VaultStateSeeds>>,

    pub system_program: Program<System>,
}

impl StarFrameInstruction for CloseIx {
    type ReturnType = ();
    type Accounts<'b, 'c> = CloseAccounts;

    fn process(
        a: &mut Self::Accounts<'_, '_>,
        _args: Self::RunArg<'_>,
        _ctx: &mut Context,
    ) -> Result<()> {
        let vault_state_data = a.vault_state.data()?;
        
        // Verify vault PDA
        let vault_seeds = VaultSeeds { state: *a.vault_state.pubkey() };
        let seeds = vault_seeds.seeds();
        let (expected_vault, _) = Pubkey::find_program_address(&seeds, &crate::ID);
        ensure!(*a.vault.pubkey() == expected_vault, "Invalid vault PDA");
        
        let vault_lamports = a.vault.lamports();

        // Create signer seeds for vault PDA
        let bump_seed = [vault_state_data.vault_bump];
        let signer_seeds = [&seeds[..], &[&bump_seed]].concat();

        // Transfer all lamports from vault back to user
        if vault_lamports > 0 {
            let transfer_accounts = [
                a.vault.account_info().clone(),
                a.user.account_info().clone(),
            ];
            
            let transfer_data = [
                2u8, 0, 0, 0, // System Program Transfer instruction discriminant
            ].iter()
            .chain(vault_lamports.to_le_bytes().iter())
            .cloned()
            .collect::<Vec<u8>>();

            let transfer_ix = Instruction {
                program_id: System::ID,
                accounts: vec![
                    AccountMeta::new(*a.vault.pubkey(), true), // vault signs as PDA
                    AccountMeta::new(*a.user.pubkey(), false),
                ],
                data: transfer_data,
            };

            invoke_signed(&transfer_ix, &transfer_accounts, &[&signer_seeds])?;
        }

        // Close the state account by transferring its lamports to user and clearing data
        let state_lamports = a.vault_state.account_info().lamports();
        if state_lamports > 0 {
            // Manual lamport transfer and data clearing for account closure
            let vault_state_info = a.vault_state.account_info().clone();
            let user_info = a.user.account_info().clone();
            
            // Transfer lamports
            *vault_state_info.lamports().borrow_mut() = 0;
            *user_info.lamports().borrow_mut() += state_lamports;
            
            // Clear the account data
            let mut data = vault_state_info.try_borrow_mut_data()?;
            data.fill(0);
        }

        Ok(())
    }
}