use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::leader_tracker::LeaderTracker;
use solana_client::nonblocking::rpc_client::RpcClient;

use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

use self::{
    config::{Config, Network},
    leader_tracker::LeaderTrackerImpl,
    quic_manager::QuicManager,
};

mod config;
mod leader_tracker;
mod quic_manager;

use clap::{ArgGroup, Parser};

#[derive(Debug, Parser)]
#[command(name = "Solana Transaction")]
#[command(group(
    ArgGroup::new("network")
        .required(true)
        .args(&["mainnet", "devnet", "helios_mainnet"]),
))]
pub struct Cli {
    #[arg(long)]
    pub mainnet: bool,
    #[arg(long)]
    pub devnet: bool,
    #[arg(long)]
    pub helios_mainnet: bool,
    #[arg(long, default_value_t = 1)]
    pub retry: u8,
}

#[tokio::main]
async fn main() {
    // Initialize the tracing subscriber for logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let cli = Cli::parse();

    let network = if cli.mainnet {
        Network::Mainnet
    } else if cli.helios_mainnet {
        Network::HeliosMainnet
    } else {
        Network::Devnet
    };

    let config = Config::new(network, cli.retry);

    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        config.rpc_url.clone(),
        config.commitment_level,
    ));
    info!("CONFIG {:#?}", config);

    let tracker =
        Arc::new(LeaderTrackerImpl::new(rpc_client.clone(), 4, 0, config.ws_url.clone()).await);
    tracker.poll_slot_leaders_once().await.unwrap();

    let mut attempts = 0;
    while attempts < config.retry {
        let leaders = tracker.get_leaders();

        if let Some(leader) = leaders.last() {
            info!("LEADER: {:#?}", leader);
            // берем первого лидера из списка с учетом смещения
            if let Some(tpu_quic) = &leader.tpu_quic {
                let manager = QuicManager::new(rpc_client.clone(), *tpu_quic).await;
                info!("QUIC: {:#?}", tpu_quic);

                match manager.send_transaction(&config).await {
                    Ok(signature) => {
                        info!("Transaction sent. Confirmation...");
                        match manager.check_confirm_transaction(&signature).await {
                            Ok(_) => {
                                info!("Transaction confirmed successfully.");
                                let full_url = config.generate_url(&signature.to_string());
                                info!("{}", full_url);
                                break;
                            }
                            Err(e) => error!("Error confirming transaction: {:#?}", e),
                        }
                    }
                    Err(e) => error!("Error sending transaction: {:#?}", e),
                }
            } else {
                error!("No QUIC address available for the current leader.");
            }
        } else {
            error!("No current leader available. Searching...");
        }
        attempts += 1;
        sleep(Duration::from_secs(1)).await;
    }

    if attempts >= config.retry {
        info!("Maximum number of attempts reached, stopping the application.");
    }
}
