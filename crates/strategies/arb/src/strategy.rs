use std::collections::HashMap;

use std::sync::Arc;

use async_trait::async_trait;

use bindings::lssvm_pair_factory::{LSSVMPairFactory, NewPairFilter};
use bindings::sudo_opensea_arb::SudoOpenseaArb;
use bindings::sudo_pair_quoter::{SellQuote, SudoPairQuoter, SUDOPAIRQUOTER_DEPLOYED_BYTECODE};
use tracing::info;

use crate::constants::FACTORY_DEPLOYMENT_BLOCK;
use crate::types::Config;
use anyhow::Result;
use arbot_core::collectors::block_collector::NewBlock;
use arbot_core::collectors::opensea_order_collector::OpenseaOrder;
use arbot_core::executors::mempool_executor::{GasBidInfo, SubmitTxToMempool};
use arbot_core::types::Strategy;
use arbot_core::utilities::state_override_middleware::StateOverrideMiddleware;
use ethers::providers::Middleware;
use ethers::types::{Filter, H256};
use ethers::types::{H160, U256};

use opensea_stream::schema::Chain;
use opensea_v2::client::OpenSeaV2Client;

use super::constants::{LSSVM_PAIR_FACTORY_ADDRESS, POOL_EVENT_SIGNATURES};
use super::types::{
    fulfill_listing_response_to_basic_order_parameters, 
    hash_to_fulfill_listing_request, 
    Action,
    Event,
};

#[derive(Debug, Clone)]
pub struct OpenseaSudoArb<M> {
    /// Ethers 客户端                                                   
    client: Arc<M>,                                                         

    /// Opensea V2 客户端                             
    opensea_client: OpenSeaV2Client,                                       

    /// 用于获取 pair 历史的 LSSVM pair factory 合约               
    lssvm_pair_factory: Arc<LSSVMPairFactory<M>>,                             

    /// 批量读取 pair 状态的 Quoter
    quoter: SudoPairQuoter<StateOverrideMiddleware<Arc<M>>>,

    /// Arb 合约
    arb_contract: SudoOpenseaArb<M>,                                        

    /// Map NFT 地址到 交易该 NFT 的 Sudo pair 地址列表    
    sudo_pools: HashMap<H160, Vec<H160>>,              

    /// Map Sudo pool 地址到当前的出价 (以 ETH 为单位) pool addresses--> current bid Map (in ETH)
    pool_bids: HashMap<H160, U256>,                                         

    /// 出价的利润数量
    bid_percentage: u64,
}

impl<M: Middleware + 'static> OpenseaSudoArb<M> {

    /// 获取 block 范围内部署的所有 pools
    pub fn new(client: Arc<M>, opensea_client: OpenSeaV2Client, config: Config) -> Self {

        // 设置 Pair factory 合约
        let lssvm_pair_factory = Arc::new(
            LSSVMPairFactory::new(*LSSVM_PAIR_FACTORY_ADDRESS, client.clone())
            );

        // 设置 pair quoter 合约
        let mut state_override = StateOverrideMiddleware::new(client.clone());
        
        // 重写账户，使用合约的字节码
        let addr = state_override.add_code(SUDOPAIRQUOTER_DEPLOYED_BYTECODE.clone());

        // 使用重写的 客户端 实例化合约
        let quoter = SudoPairQuoter::new(addr, Arc::new(state_override));

        // 设置 arb 合约
        let arb_contract = SudoOpenseaArb::new(
                config.arb_contract_address, 
                client.clone()
            );

        Self {
            client,
            opensea_client,
            lssvm_pair_factory,
            quoter,
            arb_contract,
            sudo_pools: HashMap::new(),
            pool_bids: HashMap::new(),
            bid_percentage: config.bid_percentage,
        }
    }

}

#[async_trait]
impl<M: Middleware + 'static> Strategy<Event, Action> for OpenseaSudoArb<M> {

    // Sync state on startup.
    // In order to sync this strategy, we need to get the current bid for all Sudo pools.
    async fn sync_state(&mut self) -> Result<()> {

        // Block in which the pool factory was deployed.
        let start_block = FACTORY_DEPLOYMENT_BLOCK;                                 // pool factory 部署的区块

        let current_block = self.client.get_block_number().await?.as_u64();         // 当前区块

        // Get all Sudo pool addresses deployed in the block range. 
        let pool_addresses = self.get_new_pools(start_block, current_block).await?; // 获取在区块范围内 部署的所有 Sudo pool 地址
        info!("found {} deployed sudo pools", pool_addresses.len());                // 打印日志

        // Get current bids for update state for all Sudo pools.
        for addresses in pool_addresses.chunks(200) {                               // 每次处理 200 个 Sudo pool 地址
            let quotes = self.get_quotes_for_pools(addresses.to_vec()).await?;      // 获取这些 Sudo pool 的报价
            self.update_internal_pool_state(quotes);                                // 更新内部 Sudo pool 状态
        }
        info!(
            "done syncing state, found available pools for {} collections",         // 打印日志
            self.sudo_pools.len()
        );

        Ok(())
    }

    // 处理传入的事件, 看看我们是否可以 arb 新的订单, 并在新的区块上更新内部状态
    async fn process_event(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::OpenseaOrder(order) => self.process_order_event(*order).await,
            Event::NewBlock(block) => match self.process_new_block_event(block).await {
                Ok(_) => None,
                Err(e) => {
                    panic!("Strategy is out of sync {}", e);
                }
            },
        }
    }
}

impl<M: Middleware + 'static> OpenseaSudoArb<M> {
    // Process new orders as they come in.
    async fn process_order_event(&mut self, event: OpenseaOrder) -> Option<Action> {
        let nft_address = event.listing.context.item.nft_id.address;
        info!("processing order event for address {}", nft_address);

        // Ignore orders that are not on Ethereum.
        match event.listing.context.item.nft_id.network {
            Chain::Ethereum => {}
            _ => return None,
        }
        // Ignore orders with non-eth payment.
        if event.listing.payment_token.address != H160::zero() {
            return None;
        }

        // Find pool with highest bid.
        let pools = self.sudo_pools.get(&nft_address)?;
        let (max_pool, max_bid) = pools
            .iter()
            .filter_map(|pool| self.pool_bids.get(pool).map(|bid| (pool, bid)))
            .max_by(|a, b| a.1.cmp(b.1))?;

        // Ignore orders that are not profitable.
        if max_bid <= &event.listing.base_price {
            return None;
        }

        // Build arb tx.
        self.build_arb_tx(event.listing.order_hash, *max_pool, *max_bid)
            .await
    }

    /// Process new block events, updating the internal state.
    async fn process_new_block_event(&mut self, event: NewBlock) -> Result<()> {
        info!("processing new block {}", event.number);
        // Find new pools tthat were created in the last block.
        let new_pools = self
            .get_new_pools(event.number.as_u64(), event.number.as_u64())
            .await?;
        // Find existing pools that were touched in the last block.
        let touched_pools = self
            .get_touched_pools(event.number.as_u64(), event.number.as_u64())
            .await?;
        // Get quotes for all new and touched pools and update state.
        let quotes = self
            .get_quotes_for_pools([new_pools, touched_pools].concat())
            .await?;
        self.update_internal_pool_state(quotes);
        Ok(())
    }

    /// Build arb tx from order hash and sudo pool params.
    /// 从 order hash 和 sudo pool 参数， 构建 arb 套利交易 (arb tx) 
    async fn build_arb_tx(&self, order_hash: H256, sudo_pool: H160, sudo_bid: U256 ) -> Option<Action> {

        // 从 Opensea V2 API 获取完整的订单
        let response = self
            .opensea_client
            .fulfill_listing(hash_to_fulfill_listing_request(order_hash))
            .await;

        let order = match response {
            Ok(order) => order,
            Err(e) => {
                info!("Error getting order from opensea: {}", e);
                return None;
            }
        };

        // Parse out arb contract parameters.
        let payment_value = order.fulfillment_data.transaction.value;
        let total_profit = sudo_bid - payment_value;

        // Build arb tx.
        let tx = self
            .arb_contract
            .execute_arb(
                fulfill_listing_response_to_basic_order_parameters(order),
                payment_value.into(),
                sudo_pool,
            )
            .tx;

        Some(Action::SubmitTx(SubmitTxToMempool {
            tx,
            gas_bid_info: Some(GasBidInfo {
                total_profit,
                bid_percentage: self.bid_percentage,
            }),
        }))
    }

    /// Get quotes for a list of pools.
    async fn get_quotes_for_pools(&self, pools: Vec<H160>) -> Result<Vec<(H160, SellQuote)>> {
        let quotes = self.quoter.get_multiple_sell_quotes(pools.clone()).await?;
        let res = pools
            .into_iter()
            .zip(quotes.into_iter())
            .collect::<Vec<(H160, SellQuote)>>();
        Ok(res)
    }

    /// Update the internal state of the strategy with new pool addresses and quotes.
    fn update_internal_pool_state(&mut self, pools_and_quotes: Vec<(H160, SellQuote)>) {
        for (pool_address, quote) in pools_and_quotes {
            // If a quote is available, update both the pool_bids and the sudo_pools maps.
            if quote.quote_available {
                self.pool_bids.insert(pool_address, quote.price);
                self.sudo_pools
                    .entry(quote.nft_address)
                    .or_insert(vec![])
                    .push(pool_address);
            }
            // If a quote is unavailable, remove from both the pool_bids and the sudo_pools maps.
            else {
                self.pool_bids.remove(&pool_address);
                if let Some(addresses) = self.sudo_pools.get_mut(&quote.nft_address) {
                    addresses.retain(|address| *address != pool_address);
                }
            }
        }
    }

    /// Find all pools that were touched in a given block range.
    async fn get_touched_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
        let address_list = self.pool_bids.keys().cloned().collect::<Vec<_>>();
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(address_list)
            .events(&*POOL_EVENT_SIGNATURES);

        let events = self.client.get_logs(&filter).await?;
        let touched_pools = events.iter().map(|event| event.address).collect::<Vec<_>>();
        Ok(touched_pools)
    }

    /// Find all pools that were created in a given block range.
    async fn get_new_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
        let mut pool_addresses = vec![];

        // Maxium range for a single Alchemy query is 2000 blocks.
        for block in (from_block..to_block).step_by(2000) {
            let events = self
                .lssvm_pair_factory
                .event::<NewPairFilter>()
                .from_block(block)
                .to_block(block + 2000)
                .query()
                .await?;

            let addresses = events
                .iter()
                .map(|event| event.pool_address)
                .collect::<Vec<_>>();

            info!(
                "found {} new pools in block range, total progress: {}%",
                addresses.len(),
                100 * (block - from_block) / (to_block - from_block)
            );
            pool_addresses.extend(addresses);
        }
        Ok(pool_addresses)
    }
}
