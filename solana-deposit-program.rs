use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

// Define program ID
solana_program::declare_id!("Your_Program_ID_Here");

// Define instruction types
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum DepositInstruction {
    /// Инициализация аккаунта пользователя
    /// 0. `[signer]` Пользователь, который будет владельцем аккаунта
    /// 1. `[writable]` Аккаунт данных пользователя (PDA)
    /// 2. `[]` System program
    InitializeAccount,

    /// Внесение депозита
    /// 0. `[signer]` Пользователь, который вносит депозит
    /// 1. `[writable]` Аккаунт данных пользователя (PDA)
    /// 2. `[writable]` Vault аккаунт программы (PDA)
    /// 3. `[]` System program
    Deposit { amount: u64 },

    /// Вывод средств
    /// 0. `[signer]` Пользователь, который выводит средства
    /// 1. `[writable]` Аккаунт данных пользователя (PDA)
    /// 2. `[writable]` Vault аккаунт программы (PDA)
    /// 3. `[]` System program
    Withdraw { amount: u64 },
}

// Define the data structure for user account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserAccount {
    pub owner: Pubkey,
    pub balance: u64,
}

// Program entrypoint
entrypoint!(process_instruction);

// Process instruction function
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = DepositInstruction::try_from_slice(instruction_data)?;

    match instruction {
        DepositInstruction::InitializeAccount => process_initialize_account(program_id, accounts),
        DepositInstruction::Deposit { amount } => process_deposit(program_id, accounts, amount),
        DepositInstruction::Withdraw { amount } => process_withdraw(program_id, accounts, amount),
    }
}

// Initialize account function
fn process_initialize_account(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get the accounts
    let user_account = next_account_info(account_info_iter)?;
    let user_data_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    // Verify the user is a signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Derive the PDA for user data account
    let (expected_user_data_account, bump_seed) = Pubkey::find_program_address(
        &[b"user-account", user_account.key.as_ref()],
        program_id,
    );

    // Verify the user data account is the expected PDA
    if expected_user_data_account != *user_data_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Calculate the size of the user data account
    let user_data_size = std::mem::size_of::<UserAccount>();

    // Calculate the rent required for the account
    let rent = Rent::get()?;
    let rent_lamports = rent.minimum_balance(user_data_size);

    // Create the user data account
    invoke_signed(
        &system_instruction::create_account(
            user_account.key,
            user_data_account.key,
            rent_lamports,
            user_data_size as u64,
            program_id,
        ),
        &[
            user_account.clone(),
            user_data_account.clone(),
            system_program.clone(),
        ],
        &[&[b"user-account", user_account.key.as_ref(), &[bump_seed]]],
    )?;

    // Initialize the user data account
    let user_data = UserAccount {
        owner: *user_account.key,
        balance: 0,
    };

    // Serialize the data and store it in the account
    user_data.serialize(&mut &mut user_data_account.data.borrow_mut()[..])?;

    msg!("User account initialized");
    Ok(())
}

// Deposit function
fn process_deposit(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get the accounts
    let user_account = next_account_info(account_info_iter)?;
    let user_data_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    // Verify the user is a signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Derive the PDA for user data account
    let (expected_user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", user_account.key.as_ref()],
        program_id,
    );

    // Verify the user data account is the expected PDA
    if expected_user_data_account != *user_data_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify the vault account is correct
    let (expected_vault_account, _) = Pubkey::find_program_address(
        &[b"vault"],
        program_id,
    );

    if expected_vault_account != *vault_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Transfer SOL from user to vault
    invoke(
        &system_instruction::transfer(user_account.key, vault_account.key, amount),
        &[
            user_account.clone(),
            vault_account.clone(),
            system_program.clone(),
        ],
    )?;

    // Update user account balance
    let mut user_data = UserAccount::try_from_slice(&user_data_account.data.borrow())?;
    user_data.balance += amount;
    user_data.serialize(&mut &mut user_data_account.data.borrow_mut()[..])?;

    msg!("Deposited {} lamports", amount);
    Ok(())
}

// Withdraw function
fn process_withdraw(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get the accounts
    let user_account = next_account_info(account_info_iter)?;
    let user_data_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    // Verify the user is a signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Derive the PDA for user data account
    let (expected_user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", user_account.key.as_ref()],
        program_id,
    );

    // Verify the user data account is the expected PDA
    if expected_user_data_account != *user_data_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify the vault account is correct
    let (expected_vault_account, vault_bump) = Pubkey::find_program_address(
        &[b"vault"],
        program_id,
    );

    if expected_vault_account != *vault_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify user has enough balance
    let mut user_data = UserAccount::try_from_slice(&user_data_account.data.borrow())?;
    if user_data.balance < amount {
        return Err(ProgramError::InsufficientFunds);
    }

    // Update user account balance
    user_data.balance -= amount;
    user_data.serialize(&mut &mut user_data_account.data.borrow_mut()[..])?;

    // Transfer SOL from vault to user
    invoke_signed(
        &system_instruction::transfer(vault_account.key, user_account.key, amount),
        &[
            vault_account.clone(),
            user_account.clone(),
            system_program.clone(),
        ],
        &[&[b"vault", &[vault_bump]]],
    )?;

    msg!("Withdrawn {} lamports", amount);
    Ok(())
}
