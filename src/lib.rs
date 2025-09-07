use star_frame::{
    anyhow::ensure,
    prelude::*,
    program::system::{Transfer, TransferCpiAccounts},
};

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

#[zero_copy(pod)]
#[derive(Default, Debug, Eq, PartialEq, ProgramAccount)]
#[program_account(seeds = VaultStateSeeds)]
pub struct VaultState {
    pub owner: Pubkey,
    pub state_bump: u8,
    pub vault_bump: u8,
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

    // System-owned vault PDA for storing lamports
    #[validate(arg = Seeds(VaultSeeds { state: *self.state.pubkey() }))]
    pub vault: Seeded<Mut<SystemAccount>, VaultSeeds>,

    pub system_program: Program<System>,
}

#[star_frame_instruction]
fn InitializeIx(a: &mut InitializeAccounts, _run: (), ctx: &mut Context) -> Result<()> {
    // Get rent exemption amount for the vault (0 data bytes for SystemAccount)
    let rent = ctx.get_rent()?;
    let rent_exempt_lamports = rent.minimum_balance(0);

    // What is the purpose of this transfer? Why does this account need to be funded?
    System::cpi(
        Transfer {
            lamports: rent_exempt_lamports,
        },
        TransferCpiAccounts {
            funder: *a.owner.account_info(),
            recipient: *a.vault.account_info(),
        },
        None,
    )
    .invoke()?;

    **a.state.data_mut()? = VaultState {
        owner: *a.owner.pubkey(),
        state_bump: a.state.access_seeds().bump,
        vault_bump: a.vault.access_seeds().bump,
    };

    Ok(())
}

/* -------------------- Deposit -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct DepositIx {
    #[ix_args(run)]
    pub amount: u64,
}

#[derive(AccountSet)]
pub struct DepositAccounts {
    pub user: Signer<Mut<SystemAccount>>,
    #[validate(arg = SeedsWithBump {
        seeds: VaultSeeds { state: *self.vault_state.pubkey() },
        bump: self.vault_state.data_mut()?.vault_bump,
    })]
    pub vault: Seeded<Mut<SystemAccount>, VaultSeeds>,
    // Validate that the user is the owner of the vault state account
    #[validate(arg = self.user.pubkey())]
    pub vault_state: ValidatedAccount<VaultState>,

    pub system_program: Program<System>,
}

// Why does this instruction need to exist? Can't the user just do a manual system transfer to the vault PDA?
#[star_frame_instruction]
fn DepositIx(a: &mut DepositAccounts, amount: u64) -> Result<()> {
    ensure!(a.user.lamports() >= amount, "Insufficient funds");

    System::cpi(
        Transfer { lamports: amount },
        TransferCpiAccounts {
            funder: *a.user.account_info(),
            recipient: *a.vault.account_info(),
        },
        None,
    )
    .invoke()?;

    Ok(())
}
/* -------------------- Withdraw -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct WithdrawIx {
    #[ix_args(run)]
    pub amount: u64,
}

#[derive(AccountSet)]
pub struct WithdrawAccounts {
    pub user: Signer<Mut<SystemAccount>>,
    #[validate(arg = SeedsWithBump {
        seeds: VaultSeeds { state: *self.vault_state.pubkey() },
        bump: self.vault_state.data_mut()?.vault_bump,
    })]
    pub vault: Seeded<Mut<SystemAccount>, VaultSeeds>,
    // Validate that the user is the owner of the vault state account
    #[validate(arg = self.user.pubkey())]
    pub vault_state: ValidatedAccount<VaultState>,
    pub system_program: Program<System>,
}

#[star_frame_instruction]
fn WithdrawIx(a: &mut WithdrawAccounts, amount: u64, ctx: &mut Context) -> Result<()> {
    let minimum_lamports = ctx.get_rent()?.minimum_balance(0);
    let available_lamports = a.vault.lamports().saturating_sub(minimum_lamports);
    ensure!(
        a.vault.lamports() >= available_lamports,
        "Insufficient funds"
    );

    let signer_seeds = a.vault.access_seeds().seeds_with_bump();
    System::cpi(
        Transfer { lamports: amount },
        TransferCpiAccounts {
            funder: *a.vault.account_info(),
            recipient: *a.user.account_info(),
        },
        None,
    )
    .invoke_signed(&[&signer_seeds])?;

    Ok(())
}

/* -------------------- Close -------------------- */

#[derive(BorshSerialize, BorshDeserialize, Debug, InstructionArgs)]
pub struct CloseIx;

#[derive(AccountSet)]
pub struct CloseAccounts {
    #[validate(recipient)]
    pub user: Signer<Mut<SystemAccount>>,
    #[validate(arg = SeedsWithBump {
        seeds: VaultSeeds { state: *self.vault_state.pubkey() },
        bump: self.vault_state.data_mut()?.vault_bump,
    })]
    pub vault: Seeded<Mut<SystemAccount>, VaultSeeds>,
    // Validate that the user is the owner of the vault state account
    #[validate(arg = self.user.pubkey())]
    // Close the vault state account at the end of the instruction
    #[cleanup(arg = CloseAccount(()))]
    pub vault_state: ValidatedAccount<VaultState>,
    pub system_program: Program<System>,
}

#[star_frame_instruction]
fn CloseIx(a: &mut CloseAccounts, _run: (), _ctx: &mut Context) -> Result<()> {
    let lamports = a.vault.lamports();
    if lamports > 0 {
        let signer_seeds = a.vault.access_seeds().seeds_with_bump();
        System::cpi(
            Transfer { lamports },
            TransferCpiAccounts {
                funder: *a.vault.account_info(),
                recipient: *a.user.account_info(),
            },
            None,
        )
        .invoke_signed(&[&signer_seeds])?;
    }
    Ok(())
}
