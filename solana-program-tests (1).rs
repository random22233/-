#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{BorshDeserialize, BorshSerialize};
    use solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        rent::Rent,
        system_program,
    };
    use solana_program_test::{processor, ProgramTest};
    use solana_sdk::{
        account::Account,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use std::str::FromStr;

    // Define the data structure for user account
    #[derive(BorshSerialize, BorshDeserialize, Debug)]
    pub struct UserAccount {
        pub owner: Pubkey,
        pub balance: u64,
    }

    // Define instruction types
    #[derive(BorshSerialize, BorshDeserialize, Debug)]
    pub enum DepositInstruction {
        InitializeAccount,
        Deposit { amount: u64 },
        Withdraw { amount: u64 },
    }

    // Assume your program ID
    const PROGRAM_ID: &str = "Your_Program_ID_Here";

    // Test initialize account
    #[tokio::test]
    async fn test_initialize_account() {
        // Create program test
        let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
        let mut program_test = ProgramTest::new(
            "solana_deposit_program",
            program_id,
            processor!(process_instruction),
        );

        // Start program
        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Derive user data account
        let (user_data_account, _) = Pubkey::find_program_address(
            &[b"user-account", payer.pubkey().as_ref()],
            &program_id,
        );

        // Create instruction
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(user_data_account, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: DepositInstruction::InitializeAccount.try_to_vec().unwrap(),
        };

        // Create transaction
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        // Send transaction
        banks_client.process_transaction(transaction).await.unwrap();

        // Verify account was created with expected data
        let account = banks_client.get_account(user_data_account).await.unwrap().unwrap();
        let user_data = UserAccount::try_from_slice(&account.data).unwrap();
        assert_eq!(user_data.owner, payer.pubkey());
        assert_eq!(user_data.balance, 0);
    }

    // Test deposit
    #[tokio::test]
    async fn test_deposit() {
        // Create program test
        let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
        let mut program_test = ProgramTest::new(
            "solana_deposit_program",
            program_id,
            processor!(process_instruction),
        );

        // Add vault account
        let (vault_account, vault_bump) = Pubkey::find_program_address(&[b"vault"], &program_id);
        program_test.add_account(
            vault_account,
            Account {
                lamports: 0,
                data: vec![],
                owner: program_id,
                executable: false,
                rent_epoch: 0,
            },
        );

        // Start program
        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Derive user data account
        let (user_data_account, _) = Pubkey::find_program_address(
            &[b"user-account", payer.pubkey().as_ref()],
            &program_id,
        );

        // First initialize the account
        let init_instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(user_data_account, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: DepositInstruction::InitializeAccount.try_to_vec().unwrap(),
        };

        let init_transaction = Transaction::new_signed_with_payer(
            &[init_instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        banks_client.process_transaction(init_transaction).await.unwrap();

        // Now deposit some SOL
        let amount = 1_000_000; // 0.001 SOL in lamports
        let deposit_instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(user_data_account, false),
                AccountMeta::new(vault_account, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: DepositInstruction::Deposit { amount }.try_to_vec().unwrap(),
        };

        let deposit_transaction = Transaction::new_signed_with_payer(
            &[deposit_instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        banks_client.process_transaction(deposit_transaction).await.unwrap();

        // Verify deposit was successful
        let account = banks_client.get_account(user_data_account).await.unwrap().unwrap();
        let user_data = UserAccount::try_from_slice(&account.data).unwrap();
        assert_eq!(user_data.balance, amount);

        // Verify vault received the lamports
        let vault = banks_client.get_account(vault_account).await.unwrap().unwrap();
        assert_eq!(vault.lamports, amount);
    }

    // Test withdraw
    #[tokio::test]
    async fn test_withdraw() {
        // Create program test
        let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
        let mut program_test = ProgramTest::new(
            "solana_deposit_program",
            program_id,
            processor!(process_instruction),
        );

        // Start program
        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Derive accounts
        let (user_data_account, _) = Pubkey::find_program_address(
            &[b"user-account", payer.pubkey().as_ref()],
            &program_id,
        );
        let (vault_account, _) = Pubkey::find_program_address(&[b"