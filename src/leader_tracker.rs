use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_response::RpcContactInfo};
use solana_sdk::clock::Slot;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info};

pub trait LeaderTracker: Send + Sync {
    /// get_leaders returns the next slot leaders in order
    fn get_leaders(&self) -> Vec<RpcContactInfo>;
}

const NUM_LEADERS_PER_SLOT: usize = 4;

#[derive(Clone)]
pub struct LeaderTrackerImpl {
    rpc_client: Arc<RpcClient>,
    cur_slot: Arc<AtomicU64>,
    cur_leaders: Arc<DashMap<Slot, RpcContactInfo>>,
    num_leaders: usize,
    leader_offset: i64,
}

impl LeaderTrackerImpl {
    pub async fn new(
        rpc_client: Arc<RpcClient>,
        num_leaders: usize,
        leader_offset: i64,
        ws_url: String,
    ) -> Self {
        let cur_slot = Arc::new(AtomicU64::new(0));

        let initial_slot = rpc_client.get_slot().await.unwrap_or(0);
        cur_slot.store(initial_slot, Ordering::Relaxed);

        let leader_tracker = Self {
            rpc_client,
            cur_slot,
            cur_leaders: Arc::new(DashMap::new()),
            num_leaders,
            leader_offset,
        };
        leader_tracker.start_websocket_listener(ws_url);
        leader_tracker.poll_slot_leaders();
        leader_tracker
    }

    /// Start WebSocket listener for slot updates
    fn start_websocket_listener(&self, ws_url: String) {
        let cur_slot = self.cur_slot.clone();
        tokio::spawn(async move {
            info!("Starting WebSocket listener...");
            let (ws_stream, _) = match tokio_tungstenite::connect_async(ws_url).await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    return;
                }
            };

            let (mut write, mut read) = ws_stream.split();

            // Subscribe to slot updates
            match write
                .send(Message::Text(
                    r#"{"jsonrpc":"2.0","id":1,"method":"slotSubscribe"}"#.to_string(),
                ))
                .await
            {
                Ok(_) => info!("WebSocket subscribed to slot updates"),
                Err(e) => {
                    error!("Failed to send subscribe message: {:#?}", e);
                    return;
                }
            };

            while let Some(Ok(message)) = read.next().await {
                if let Message::Text(text) = message {
                    if let Ok(response) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(slot) = response["params"]["result"]["slot"].as_u64() {
                            cur_slot.store(slot, Ordering::Relaxed);
                        }
                    }
                }
            }
        });
    }

    /// poll_slot_leaders polls every minute for the next 1000 slot leaders and populates the cur_leaders map with the slot and ContactInfo of each leader
    fn poll_slot_leaders(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            loop {
                let start = std::time::Instant::now();
                if let Err(e) = self_clone.poll_slot_leaders_once().await {
                    error!("Error polling slot leaders: {}", e);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                let duration = start.elapsed();
                info!("poll_slot_leaders took {:?}", duration);
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    pub async fn poll_slot_leaders_once(&self) -> Result<(), String> {
        let next_slot = self.cur_slot.load(Ordering::Relaxed);

        // polling 1000 slots ahead is more than enough
        let slot_leaders = self
            .rpc_client
            .get_slot_leaders(next_slot, 1000)
            .await
            .map_err(|e| format!("Error getting slot leaders: {}", e))?;

        let new_cluster_nodes = self
            .rpc_client
            .get_cluster_nodes()
            .await
            .map_err(|e| format!("Error getting cluster nodes: {}", e))?;

        let cluster_node_map: HashMap<_, _> = new_cluster_nodes
            .into_iter()
            .map(|node| (node.pubkey.clone(), node))
            .collect();

        for (i, leader) in slot_leaders.iter().enumerate() {
            if let Some(contact_info) = cluster_node_map.get(&leader.to_string()) {
                self.cur_leaders
                    .insert(next_slot + i as u64, contact_info.clone());
            } else {
                error!("Leader {} not found in cluster nodes", leader);
            }
        }

        self.clean_up_slot_leaders();
        Ok(())
    }

    fn clean_up_slot_leaders(&self) {
        let cur_slot = self.cur_slot.load(Ordering::Relaxed);
        let slots_to_remove: Vec<_> = self
            .cur_leaders
            .iter()
            .filter(|leader| *leader.key() < cur_slot)
            .map(|leader| *leader.key())
            .collect();

        for slot in slots_to_remove {
            self.cur_leaders.remove(&slot);
        }
    }
}

impl LeaderTracker for LeaderTrackerImpl {
    fn get_leaders(&self) -> Vec<RpcContactInfo> {
        let start_slot = self.cur_slot.load(Ordering::Relaxed) + self.leader_offset as u64;
        let end_slot = start_slot + (self.num_leaders * NUM_LEADERS_PER_SLOT) as u64;
        let mut leaders = IndexMap::new();

        for slot in start_slot..end_slot {
            if let Some(leader) = self.cur_leaders.get(&slot) {
                leaders.insert(leader.pubkey.clone(), leader.value().clone());
            }
            if leaders.len() >= self.num_leaders {
                break;
            }
        }

        info!(
            "leaders: {:#?}, start_slot: {:#?}",
            leaders.keys(),
            start_slot
        );

        leaders.values().cloned().collect()
    }
}
