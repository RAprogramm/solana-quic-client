use std::str::FromStr;

use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    system_instruction::transfer,
    transaction::Transaction,
};

#[derive(Debug)]
pub enum Network {
    Mainnet,
    Devnet,
    HeliosMainnet,
}

#[derive(Debug)]
pub struct Config {
    pub rpc_url: String,
    pub ws_url: String,
    pub sender_key: String,
    pub receiver_key: String,
    pub amount: u64,
    pub retry: u8,
    pub network: Network,
    pub commitment_level: CommitmentConfig,
}

impl Config {
    pub fn new(network: Network, retry: u8) -> Self {
        match network {
            Network::Mainnet => Self {
                rpc_url: String::from("https://api.mainnet-beta.solana.com"),
                ws_url: String::from("wss://api.mainnet-beta.solana.com"),
                sender_key: String::from("5HXCzcx2hyJ4N4hNimytnfff5dGMLjuMv9M6kCkEWASWnUsckoNgGEb3SqQQBFQP5SCc6xDuoHtE9Rx3ESrH98xu"),
                receiver_key: String::from("HXeJrqomDdf4KoDfx36D27Lfffu7jmdVGjUSeEAprLRk"),
                amount: 1_000,
                retry,
                network: Network::Mainnet,
                commitment_level: CommitmentConfig::finalized()
            },
            Network::Devnet => Self {
                rpc_url: String::from("https://api.devnet.solana.com"),
                ws_url: String::from("wss://api.devnet.solana.com"),
                sender_key: String::from("/home/user/.config/solana/devnet.json"),
                receiver_key: String::from("/home/user/.config/solana/test-receiver.json"),
                amount: 1_000,
                retry,
                network: Network::Devnet,
                commitment_level: CommitmentConfig::finalized()
            },
            Network::HeliosMainnet => Self {
                rpc_url: String::from("https://mainnet.helius-rpc.com/?api-key=cbf2e1f7-5c84-4ba5-bfba-a66ef18a7faf"),
                ws_url: String::from("wss://mainnet.helius-rpc.com/?api-key=cb57e1r2-5c84-4ba5-bfba-a66ef18a7faf"),
                sender_key: String::from("5HXCzcx2hyJ4N4hNimytnfff5dGMLjuMv9M6kCkEWASWnUsckoNgGEb3SqQQBFQP5SCc6xDuoHtE9Rx3ESrH98xu"),
                receiver_key: String::from("HXeJrqomDdf4KoDfx36D2fffdu7jmdVGjUSeEAprLRk"),
                amount: 1_000,
                retry,
                network: Network::HeliosMainnet,
                commitment_level: CommitmentConfig::finalized()
            },
        }
    }

    fn setup_sender(&self) -> Keypair {
        match self.network {
            Network::Mainnet => Keypair::from_base58_string(&self.sender_key),
            Network::HeliosMainnet => Keypair::from_base58_string(&self.sender_key),
            Network::Devnet => {
                read_keypair_file(&self.sender_key).expect("Unable to read keypair file")
            }
        }
    }

    fn setup_receiver(&self) -> Pubkey {
        match self.network {
            Network::Mainnet => Pubkey::from_str(&self.receiver_key).expect("Invalid pubkey"),
            Network::HeliosMainnet => Pubkey::from_str(&self.receiver_key).expect("Invalid pubkey"),
            Network::Devnet => read_keypair_file(&self.receiver_key)
                .expect("Failed to read receiver keypair from file")
                .pubkey(),
        }
    }

    pub fn create_transaction(&self, blockhash: Hash) -> Transaction {
        let sender = Config::setup_sender(self);
        let receiver = Config::setup_receiver(self);

        let compute_unit_limit_instruction =
            ComputeBudgetInstruction::set_compute_unit_limit(50_000);
        let compute_unit_price_instruction =
            ComputeBudgetInstruction::set_compute_unit_price(10000);

        let transfer_instruction = transfer(&sender.pubkey(), &receiver, self.amount);

        Transaction::new_signed_with_payer(
            &[
                compute_unit_limit_instruction,
                compute_unit_price_instruction,
                transfer_instruction,
            ],
            Some(&sender.pubkey()),
            &[&sender],
            blockhash,
        )
    }

    pub fn generate_url(&self, transaction_number: &str) -> String {
        let base_url = "https://explorer.solana.com/tx/";
        let cluster = match self.network {
            Network::Mainnet => "",
            Network::HeliosMainnet => "",
            Network::Devnet => "?cluster=devnet",
        };

        format!(
            "Check transaction {}{}{}",
            base_url, transaction_number, cluster
        )
    }
}
