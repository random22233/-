use borsh::{BorshDeserialize, BorshSerialize};
use clap::{App, Arg, SubCommand};
use solana_client::rpc_client::RpcClient;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    system_program,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

// Define instruction types
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum DepositInstruction {
    InitializeAccount,
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
}

// Define the data structure for user account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserAccount {
    pub owner: Pubkey,
    pub balance: u64,
}

fn main() {
    let matches = App::new("Solana Deposit Client")
        .version("1.0")
        .author("Your Name")
        .about("Client for interacting with Solana Deposit Program")
        .arg(
            Arg::with_name("keypair")
                .short("k")
                .long("keypair")
                .value_name("KEYPAIR")
                .help("Keypair file path")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("url")
                .short("u")
                .long("url")
                .value_name("URL")
                .help("RPC URL (default: devnet)")
                .takes_value(true)
                .default_value("https://api.devnet.solana.com"),
        )
        .arg(
            Arg::with_name("program-id")
                .short("p")
                .long("program-id")
                .value_name("PUBKEY")
                .help("Program ID")
                .takes_value(true)
                .required(true),
        )
        .subcommand(SubCommand::with_name("init").about("Initialize a user account"))
        .subcommand(
            SubCommand::with_name("deposit")
                .about("Deposit SOL")
                .arg(
                    Arg::with_name("amount")
                        .short("a")
                        .long("amount")
                        .value_name("AMOUNT")
                        .help("Amount in SOL to deposit")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("withdraw")
                .about("Withdraw SOL")
                .arg(
                    Arg::with_name("amount")
                        .short("a")
                        .long("amount")
                        .value_name("AMOUNT")
                        .help("Amount in SOL to withdraw")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(SubCommand::with_name("balance").about("Get account balance"))
        .get_matches();

    // Parse command line arguments
    let keypair_path = matches.value_of("keypair").unwrap();
    let url = matches.value_of("url").unwrap();
    let program_id = Pubkey::from_str(matches.value_of("program-id").unwrap())
        .expect("Failed to parse program ID");

    // Load keypair
    let payer = read_keypair_file(keypair_path).expect("Failed to read keypair file");

    // Create RPC client
    let client = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    // Process subcommands
    match matches.subcommand() {
        ("init", Some(_)) => {
            initialize_account(&client, &payer, &program_id);
        }
        ("deposit", Some(sub_matches)) => {
            let amount = sub_matches
                .value_of("amount")
                .unwrap()
                .parse::<f64>()
                .expect("Amount must be a number");
            let lamports = (amount * 1_000_000_000.0) as u64; // Convert SOL to lamports
            deposit(&client, &payer, &program_id, lamports);
        }
        ("withdraw", Some(sub_matches)) => {
            let amount = sub_matches
                .value_of("amount")
                .unwrap()
                .parse::<f64>()
                .expect("Amount must be a number");
            let lamports = (amount * 1_000_000_000.0) as u64; // Convert SOL to lamports
            withdraw(&client, &payer, &program_id, lamports);
        }
        ("balance", Some(_)) => {
            get_balance(&client, &payer, &program_id);
        }
        _ => {
            println!("Invalid command. Use --help for usage information.");
        }
    }
}

fn initialize_account(client: &RpcClient, payer: &Keypair, program_id: &Pubkey) {
    println!("Initializing user account...");

    // Derive user data account
    let (user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", payer.pubkey().as_ref()],
        program_id,
    );

    // Create instruction
    let instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(user_data_account, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DepositInstruction::InitializeAccount.try_to_vec().unwrap(),
    };

    // Create and send transaction
    let recent_blockhash = client.get_latest_blockhash().expect("Failed to get blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("Account initialized successfully!");
            println!("Transaction signature: {}", signature);
        }
        Err(err) => {
            println!("Error initializing account: {}", err);
        }
    }
}

fn deposit(client: &RpcClient, payer: &Keypair, program_id: &Pubkey, amount: u64) {
    println!("Depositing {} lamports...", amount);

    // Derive user data account
    let (user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", payer.pubkey().as_ref()],
        program_id,
    );

    // Derive vault account
    let (vault_account, _) = Pubkey::find_program_address(&[b"vault"], program_id);

    // Create instruction
    let instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(user_data_account, false),
            AccountMeta::new(vault_account, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DepositInstruction::Deposit { amount }.try_to_vec().unwrap(),
    };

    // Create and send transaction
    let recent_blockhash = client.get_latest_blockhash().expect("Failed to get blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("Deposit successful!");
            println!("Transaction signature: {}", signature);
        }
        Err(err) => {
            println!("Error making deposit: {}", err);
        }
    }
}

fn withdraw(client: &RpcClient, payer: &Keypair, program_id: &Pubkey, amount: u64) {
    println!("Withdrawing {} lamports...", amount);

    // Derive user data account
    let (user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", payer.pubkey().as_ref()],
        program_id,
    );

    // Derive vault account
    let (vault_account, _) = Pubkey::find_program_address(&[b"vault"], program_id);

    // Create instruction
    let instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(user_data_account, false),
            AccountMeta::new(vault_account, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DepositInstruction::Withdraw { amount }.try_to_vec().unwrap(),
    };

    // Create and send transaction
    let recent_blockhash = client.get_latest_blockhash().expect("Failed to get blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("Withdrawal successful!");
            println!("Transaction signature: {}", signature);
        }
        Err(err) => {
            println!("Error making withdrawal: {}", err);
        }
    }
}

fn get_balance(client: &RpcClient, payer: &Keypair, program_id: &Pubkey) {
    println!("Getting account balance...");

    // Derive user data account
    let (user_data_account, _) = Pubkey::find_program_address(
        &[b"user-account", payer.pubkey().as_ref()],
        program_id,
    );

    // Get account data
    match client.get_account_data(&user_data_account) {
        Ok(data) => {
            // Deserialize account data
            let user_account = UserAccount::try_from_slice(&data).expect("Failed to deserialize account data");
            
            // Display balance
            println!("Balance: {} SOL", user_account.balance as f64 / 1_000_000_000.0);
        }
        Err(err) => {
            println!("Error getting balance: {}. Make sure the account is initialized.", err);
        }
    }
}
