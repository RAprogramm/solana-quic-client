use solana_client::{
    nonblocking::{
        quic_client::{QuicLazyInitializedEndpoint, QuicTpuConnection},
        rpc_client::RpcClient,
        tpu_connection::TpuConnection,
    },
    tpu_connection::ClientStats,
};
use solana_connection_cache::connection_cache_stats::ConnectionCacheStats;
use solana_sdk::signature::Signature;
use std::{net::SocketAddr, sync::Arc};
use tracing::{error, info};

use crate::config::Config;

pub struct QuicManager {
    pub connection: Arc<QuicTpuConnection>,
    pub stats: Arc<ClientStats>,
    pub rpc_client: Arc<RpcClient>,
}

impl QuicManager {
    pub async fn new(rpc_client: Arc<RpcClient>, socket_addr: SocketAddr) -> Self {
        let endpoint = Arc::new(QuicLazyInitializedEndpoint::default());
        let stats = Arc::new(ClientStats::default());
        let connection_stats = Arc::new(ConnectionCacheStats::default());

        let quic_tpu_connection = QuicTpuConnection::new(endpoint, socket_addr, connection_stats);

        QuicManager {
            connection: Arc::new(quic_tpu_connection),
            stats,
            rpc_client,
        }
    }

    pub async fn send_transaction(&self, config: &Config) -> Result<Signature, String> {
        let max_attempts = 1; // Увеличение числа попыток
        for attempt in 0..max_attempts {
            let blockhash = self
                .rpc_client
                .get_latest_blockhash()
                .await
                .map_err(|e| format!("Failed to get blockhash: {}", e))?;
            info!("[ BLOCKHASH ] - {:#?}", blockhash);

            let transaction = config.create_transaction(blockhash);

            info!(
            "[ TRANSACTION\n\tSENDER: {:?}\n\tRECEIVER: {:?}\n\tBLOCKHASH: {:?}\n\tSIGNATURE: {:?}\n]",
            transaction.message.account_keys[0],
            transaction.message.account_keys[1],
            transaction.message.recent_blockhash,
            transaction.signatures
        );

            let serialized_tx = bincode::serialize(&transaction).unwrap();

            let send_result = tokio::time::timeout(
                std::time::Duration::from_secs(60), // Увеличение таймаута до 60 секунд
                self.connection.send_data(&serialized_tx),
            )
            .await;

            match send_result {
                Ok(Ok(_)) => {
                    if let Some(signature) = transaction.signatures.first() {
                        return Ok(*signature);
                    } else {
                        return Err("No signature found in the transaction".to_string());
                    }
                }
                Ok(Err(e)) => {
                    error!(
                        "Attempt {}: Failed to send transaction via QUIC: {:#?}",
                        attempt + 1,
                        e
                    );
                    if attempt + 1 < max_attempts {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
                Err(_) => {
                    error!(
                        "Attempt {}: Timed out while sending transaction via QUIC",
                        attempt + 1
                    );
                    if attempt + 1 < max_attempts {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }

        Err("Failed to send transaction via QUIC after multiple attempts".to_string())
    }

    pub async fn check_confirm_transaction(&self, signature: &Signature) -> Result<bool, String> {
        let transaction_with_meta = self
            .rpc_client
            .get_transaction(
                signature,
                solana_transaction_status::UiTransactionEncoding::Json,
            )
            .await;
        info!("META {:#?}", transaction_with_meta);

        let max_attempts = 10;
        for _ in 0..max_attempts {
            let statuses = self
                .rpc_client
                .get_signature_statuses(&[*signature])
                .await
                .map_err(|e| format!("Failed to get signature statuses: {}", e))?;

            if let Some(Some(status)) = statuses.value.first() {
                if status.confirmations.is_some() {
                    return Ok(true);
                } else {
                    info!("Transaction not confirmed yet, retrying...");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Err("Transaction failed to confirm".to_string())
    }
}
