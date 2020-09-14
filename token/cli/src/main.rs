use clap::{
    crate_description, crate_name, crate_version, value_t, value_t_or_exit, App, AppSettings, Arg,
    SubCommand,
};
use solana_account_decoder::{parse_token::TokenAccountType, UiAccountData};
use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_amount, is_keypair, is_pubkey_or_keypair, is_url},
    keypair::signer_from_path,
};
use solana_client::{rpc_client::RpcClient, rpc_request::TokenAccountsFilter};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::*,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{
    self,
    instruction::*,
    native_mint,
    pack::Pack,
    state::{Account, Mint},
};
use std::process::exit;

struct Config {
    rpc_client: RpcClient,
    verbose: bool,
    owner: Box<dyn Signer>,
    fee_payer: Box<dyn Signer>,
    commitment_config: CommitmentConfig,
}

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<Option<Transaction>, Error>;

macro_rules! unique_signers {
    ($vec:ident) => {
        $vec.sort_by_key(|l| l.pubkey());
        $vec.dedup();
    };
}

fn check_fee_payer_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.fee_payer.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            config.fee_payer.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

fn check_owner_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.owner.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Owner, {}, has insufficient balance: {} required, {} available",
            config.owner.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

fn command_create_token(config: &Config, decimals: u8, token: Box<dyn Signer>) -> CommandResult {
    println!("Creating token {}", token.pubkey());

    let minimum_balance_for_rent_exemption = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Mint::LEN)?;

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &token.pubkey(),
                minimum_balance_for_rent_exemption,
                Mint::LEN as u64,
                &spl_token::id(),
            ),
            initialize_mint(
                &spl_token::id(),
                &token.pubkey(),
                &config.owner.pubkey(),
                None,
                decimals,
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        minimum_balance_for_rent_exemption + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    let mut signers = vec![
        config.fee_payer.as_ref(),
        config.owner.as_ref(),
        token.as_ref(),
    ];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_create_account(
    config: &Config,
    token: Pubkey,
    account: Box<dyn Signer>,
) -> CommandResult {
    println!("Creating account {}", account.pubkey());

    let minimum_balance_for_rent_exemption = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Account::LEN)?;

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &config.fee_payer.pubkey(),
                &account.pubkey(),
                minimum_balance_for_rent_exemption,
                Account::LEN as u64,
                &spl_token::id(),
            ),
            initialize_account(
                &spl_token::id(),
                &account.pubkey(),
                &token,
                &config.owner.pubkey(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        minimum_balance_for_rent_exemption + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    let mut signers = vec![
        config.fee_payer.as_ref(),
        account.as_ref(),
        config.owner.as_ref(),
    ];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_assign(config: &Config, account: Pubkey, new_owner: Pubkey) -> CommandResult {
    println!(
        "Assigning {}\n  Current owner: {}\n  New owner: {}",
        account,
        config.owner.pubkey(),
        new_owner
    );

    let mut transaction = Transaction::new_with_payer(
        &[set_authority(
            &spl_token::id(),
            &account,
            Some(&new_owner),
            AuthorityType::AccountOwner,
            &config.owner.pubkey(),
            &[],
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_transfer(
    config: &Config,
    sender: Pubkey,
    ui_amount: f64,
    recipient: Pubkey,
) -> CommandResult {
    println!(
        "Transfer {} tokens\n  Sender: {}\n  Recipient: {}",
        ui_amount, sender, recipient
    );

    let sender_token_balance = config
        .rpc_client
        .get_token_account_balance_with_commitment(&sender, config.commitment_config)?
        .value;
    let source_account = config
        .rpc_client
        .get_account_with_commitment(&sender, config.commitment_config)?
        .value
        .unwrap_or_default();
    let data = source_account.data.to_vec();
    let mint_pubkey = Account::unpack_from_slice(&data)?.mint;
    let amount = spl_token::ui_amount_to_amount(ui_amount, sender_token_balance.decimals);

    let mut transaction = Transaction::new_with_payer(
        &[transfer2(
            &spl_token::id(),
            &sender,
            &mint_pubkey,
            &recipient,
            &config.owner.pubkey(),
            &[],
            amount,
            sender_token_balance.decimals,
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_burn(config: &Config, source: Pubkey, ui_amount: f64) -> CommandResult {
    println!("Burn {} tokens\n  Source: {}", ui_amount, source);

    let source_token_balance = config
        .rpc_client
        .get_token_account_balance_with_commitment(&source, config.commitment_config)?
        .value;
    let source_account = config
        .rpc_client
        .get_account_with_commitment(&source, config.commitment_config)?
        .value
        .unwrap_or_default();
    let data = source_account.data.to_vec();
    let mint_pubkey = Account::unpack_from_slice(&data)?.mint;
    let amount = spl_token::ui_amount_to_amount(ui_amount, source_token_balance.decimals);
    let mut transaction = Transaction::new_with_payer(
        &[burn2(
            &spl_token::id(),
            &source,
            &mint_pubkey,
            &config.owner.pubkey(),
            &[],
            amount,
            source_token_balance.decimals,
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_mint(
    config: &Config,
    token: Pubkey,
    ui_amount: f64,
    recipient: Pubkey,
) -> CommandResult {
    println!(
        "Minting {} tokens\n  Token: {}\n  Recipient: {}",
        ui_amount, token, recipient
    );

    let recipient_token_balance = config
        .rpc_client
        .get_token_account_balance_with_commitment(&recipient, config.commitment_config)?
        .value;
    let amount = spl_token::ui_amount_to_amount(ui_amount, recipient_token_balance.decimals);

    let mut transaction = Transaction::new_with_payer(
        &[mint_to2(
            &spl_token::id(),
            &token,
            &recipient,
            &config.owner.pubkey(),
            &[],
            amount,
            recipient_token_balance.decimals,
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_wrap(config: &Config, sol: f64) -> CommandResult {
    let account = Keypair::new();
    let lamports = sol_to_lamports(sol);
    println!("Wrapping {} SOL into {}", sol, account.pubkey());

    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &config.owner.pubkey(),
                &account.pubkey(),
                lamports,
                Account::LEN as u64,
                &spl_token::id(),
            ),
            initialize_account(
                &spl_token::id(),
                &account.pubkey(),
                &native_mint::id(),
                &config.owner.pubkey(),
            )?,
        ],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_owner_balance(config, lamports)?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref(), &account];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_unwrap(config: &Config, address: Pubkey) -> CommandResult {
    println!("Unwrapping {}", address);
    println!(
        "  Amount: {} SOL\n  Recipient: {}",
        lamports_to_sol(
            config
                .rpc_client
                .get_balance_with_commitment(&address, config.commitment_config)?
                .value
        ),
        config.owner.pubkey()
    );

    let mut transaction = Transaction::new_with_payer(
        &[close_account(
            &spl_token::id(),
            &address,
            &config.owner.pubkey(),
            &config.owner.pubkey(),
            &[],
        )?],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let mut signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    unique_signers!(signers);
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_balance(config: &Config, address: Pubkey) -> CommandResult {
    let balance = config
        .rpc_client
        .get_token_account_balance_with_commitment(&address, config.commitment_config)?
        .value;

    if config.verbose {
        println!("ui amount: {}", balance.ui_amount);
        println!("decimals: {}", balance.decimals);
        println!("amount: {}", balance.amount);
    } else {
        println!("{}", balance.ui_amount);
    }
    Ok(None)
}

fn command_supply(config: &Config, address: Pubkey) -> CommandResult {
    let supply = config
        .rpc_client
        .get_token_supply_with_commitment(&address, config.commitment_config)?
        .value;

    println!("{}", supply.ui_amount);
    Ok(None)
}

fn command_accounts(config: &Config, token: Option<Pubkey>) -> CommandResult {
    let accounts = config
        .rpc_client
        .get_token_accounts_by_owner_with_commitment(
            &config.owner.pubkey(),
            match token {
                Some(token) => TokenAccountsFilter::Mint(token),
                None => TokenAccountsFilter::ProgramId(spl_token::id()),
            },
            config.commitment_config,
        )?
        .value;
    if accounts.is_empty() {
        println!("None");
    }

    println!("Account                                      Token                                        Balance");
    println!("-------------------------------------------------------------------------------------------------");
    for keyed_account in accounts {
        let address = keyed_account.pubkey;

        if let UiAccountData::Json(parsed_account) = keyed_account.account.data {
            if parsed_account.program != "spl-token" {
                println!(
                    "{:<44} Unsupported account program: {}",
                    address, parsed_account.program
                );
            } else {
                match serde_json::from_value(parsed_account.parsed) {
                    Ok(TokenAccountType::Account(ui_token_account)) => println!(
                        "{:<44} {:<44} {}",
                        address, ui_token_account.mint, ui_token_account.token_amount.ui_amount
                    ),
                    Ok(_) => println!("{:<44} Unsupported token account", address),
                    Err(err) => println!("{:<44} Account parse failure: {}", address, err),
                }
            }
        } else {
            println!("{:<44} Unsupported account data format", address);
        }
    }
    Ok(None)
}

fn main() {
    let default_decimals = &format!("{}", native_mint::DECIMALS);
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .help("JSON RPC URL for the cluster.  Default from the configuration file."),
        )
        .arg(
            Arg::with_name("owner")
                .long("owner")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the token owner account. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .arg(
            Arg::with_name("fee_payer")
                .long("fee-payer")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the fee-payer account. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .subcommand(SubCommand::with_name("create-token").about("Create a new token")
                .arg(
                    Arg::with_name("decimals")
                        .long("decimals")
                        .validator(|s| {
                            s.parse::<u8>().map_err(|e| format!("{}", e))?;
                            Ok(())
                        })
                        .value_name("DECIMALS")
                        .takes_value(true)
                        .default_value(&default_decimals)
                        .help("Number of base 10 digits to the right of the decimal place"),
                )
                .arg(
                    Arg::with_name("token_keypair")
                        .value_name("KEYPAIR")
                        .validator(is_keypair)
                        .takes_value(true)
                        .index(1)
                        .help(
                            "Specify the token keypair. \
                             This may be a keypair file or the ASK keyword. \
                             [default: randomly generated keypair]"
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("create-account")
                .about("Create a new token account")
                .arg(
                    Arg::with_name("token")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token that the account will hold"),
                )
                .arg(
                    Arg::with_name("account_keypair")
                        .value_name("KEYPAIR")
                        .validator(is_keypair)
                        .takes_value(true)
                        .index(2)
                        .help(
                            "Specify the account keypair. \
                             This may be a keypair file or the ASK keyword. \
                             [default: randomly generated keypair]"
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("assign")
                .about("Assign a token or token account to a new owner")
                .arg(
                    Arg::with_name("address")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The address of the token account"),
                )
                .arg(
                    Arg::with_name("new_owner")
                        .validator(is_pubkey_or_keypair)
                        .value_name("OWNER_ADDRESS")
                        .takes_value(true)
                        .index(2)
                        .required(true)
                        .help("The address of the new owner"),
                ),
        )
        .subcommand(
            SubCommand::with_name("transfer")
                .about("Transfer tokens between accounts")
                .arg(
                    Arg::with_name("sender")
                        .validator(is_pubkey_or_keypair)
                        .value_name("SENDER_TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token account address of the sender"),
                )
                .arg(
                    Arg::with_name("amount")
                        .validator(is_amount)
                        .value_name("TOKEN_AMOUNT")
                        .takes_value(true)
                        .index(2)
                        .required(true)
                        .help("Amount to send, in tokens"),
                )
                .arg(
                    Arg::with_name("recipient")
                        .validator(is_pubkey_or_keypair)
                        .value_name("RECIPIENT_TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(3)
                        .required(true)
                        .help("The token account address of recipient"),
                ),
        )
        .subcommand(
            SubCommand::with_name("burn")
                .about("Burn tokens from an account")
                .arg(
                    Arg::with_name("source")
                        .validator(is_pubkey_or_keypair)
                        .value_name("SOURCE_TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token account address to burn from"),
                )
                .arg(
                    Arg::with_name("amount")
                        .validator(is_amount)
                        .value_name("TOKEN_AMOUNT")
                        .takes_value(true)
                        .index(2)
                        .required(true)
                        .help("Amount to burn, in tokens"),
                ),
        )
        .subcommand(
            SubCommand::with_name("mint")
                .about("Mint new tokens")
                .arg(
                    Arg::with_name("token")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token to mint"),
                )
                .arg(
                    Arg::with_name("amount")
                        .validator(is_amount)
                        .value_name("TOKEN_AMOUNT")
                        .takes_value(true)
                        .index(2)
                        .required(true)
                        .help("Amount to mint, in tokens"),
                )
                .arg(
                    Arg::with_name("recipient")
                        .validator(is_pubkey_or_keypair)
                        .value_name("RECIPIENT_TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(3)
                        .required(true)
                        .help("The token account address of recipient"),
                ),
        )
        .subcommand(
            SubCommand::with_name("balance")
                .about("Get token account balance")
                .arg(
                    Arg::with_name("address")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token account address"),
                ),
        )
        .subcommand(
            SubCommand::with_name("supply")
                .about("Get token supply")
                .arg(
                    Arg::with_name("address")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The token address"),
                ),
        )
        .subcommand(
            SubCommand::with_name("accounts")
                .about("List all token accounts by owner")
                .arg(
                    Arg::with_name("token")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .help("Limit results to the given token. [Default: list accounts for all tokens]"),
                ),
        )
        .subcommand(
            SubCommand::with_name("wrap")
                .about("Wrap native SOL in a SOL token account")
                .arg(
                    Arg::with_name("amount")
                        .validator(is_amount)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("Amount of SOL to wrap"),
                ),
        )
        .subcommand(
            SubCommand::with_name("unwrap")
                .about("Unwrap a SOL token account")
                .arg(
                    Arg::with_name("address")
                        .validator(is_pubkey_or_keypair)
                        .value_name("TOKEN_ACCOUNT_ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .help("The address of the token account to unwrap"),
                ),
        )
        .get_matches();

    let mut wallet_manager = None;

    let config = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let json_rpc_url = value_t!(matches, "json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let owner = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "owner",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let fee_payer = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "fee_payer",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let verbose = matches.is_present("verbose");

        Config {
            rpc_client: RpcClient::new(json_rpc_url),
            verbose,
            owner,
            fee_payer,
            commitment_config: CommitmentConfig::single(),
        }
    };

    solana_logger::setup_with_default("solana=info");

    let _ = match matches.subcommand() {
        ("create-token", Some(arg_matches)) => {
            let decimals = value_t_or_exit!(arg_matches, "decimals", u8);
            let token = if arg_matches.is_present("token_keypair") {
                signer_from_path(
                    &matches,
                    &value_t_or_exit!(arg_matches, "token_keypair", String),
                    "token_keypair",
                    &mut wallet_manager,
                )
                .unwrap_or_else(|e| {
                    eprintln!("error: {}", e);
                    exit(1);
                })
            } else {
                Box::new(Keypair::new())
            };

            command_create_token(&config, decimals, token)
        }
        ("create-account", Some(arg_matches)) => {
            let token = pubkey_of(arg_matches, "token").unwrap();
            let account = if arg_matches.is_present("account_keypair") {
                signer_from_path(
                    &matches,
                    &value_t_or_exit!(arg_matches, "account_keypair", String),
                    "account_keypair",
                    &mut wallet_manager,
                )
                .unwrap_or_else(|e| {
                    eprintln!("error: {}", e);
                    exit(1);
                })
            } else {
                Box::new(Keypair::new())
            };

            command_create_account(&config, token, account)
        }
        ("assign", Some(arg_matches)) => {
            let address = pubkey_of(arg_matches, "address").unwrap();
            let new_owner = pubkey_of(arg_matches, "new_owner").unwrap();
            command_assign(&config, address, new_owner)
        }
        ("transfer", Some(arg_matches)) => {
            let sender = pubkey_of(arg_matches, "sender").unwrap();
            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            let recipient = pubkey_of(arg_matches, "recipient").unwrap();
            command_transfer(&config, sender, amount, recipient)
        }
        ("burn", Some(arg_matches)) => {
            let source = pubkey_of(arg_matches, "source").unwrap();
            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            command_burn(&config, source, amount)
        }
        ("mint", Some(arg_matches)) => {
            let token = pubkey_of(arg_matches, "token").unwrap();
            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            let recipient = pubkey_of(arg_matches, "recipient").unwrap();
            command_mint(&config, token, amount, recipient)
        }
        ("wrap", Some(arg_matches)) => {
            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            command_wrap(&config, amount)
        }
        ("unwrap", Some(arg_matches)) => {
            let address = pubkey_of(arg_matches, "address").unwrap();
            command_unwrap(&config, address)
        }
        ("balance", Some(arg_matches)) => {
            let address = pubkey_of(arg_matches, "address").unwrap();
            command_balance(&config, address)
        }
        ("supply", Some(arg_matches)) => {
            let address = pubkey_of(arg_matches, "address").unwrap();
            command_supply(&config, address)
        }
        ("accounts", Some(arg_matches)) => {
            let token = pubkey_of(arg_matches, "token");
            command_accounts(&config, token)
        }
        _ => unreachable!(),
    }
    .and_then(|transaction| {
        if let Some(transaction) = transaction {
            // TODO: Upgrade to solana-client 1.3 and
            // `send_and_confirm_transaction_with_spinner_and_commitment()` with single
            // confirmation by default for better UX
            let signature = config
                .rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &transaction,
                    config.commitment_config,
                )?;
            println!("Signature: {}", signature);
        }
        Ok(())
    })
    .map_err(|err| {
        eprintln!("{}", err);
        exit(1);
    });
}
