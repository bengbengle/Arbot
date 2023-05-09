use anyhow::Result;
use clap::Parser;
use ethers::types::H160;

use opensea_v2::client::{OpenSeaApiConfig, OpenSeaV2Client};

use ethers::prelude::MiddlewareBuilder;
use ethers::providers::{Provider, Ws};

use artemis_core::collectors::block_collector::BlockCollector;
// use artemis_core::collectors::opensea_order_collector::OpenseaOrderCollector;

use artemis_core::executors::mempool_executor::MempoolExecutor;

use ethers::signers::{LocalWallet, Signer};

use arb::strategy::OpenseaSudoArb;
use arb::types::{Action, Config, Event};

use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

use std::str::FromStr;
use std::sync::Arc;

use artemis_core::engine::Engine;
use artemis_core::types::{CollectorMap, ExecutorMap};

use std::env;
use dotenv::dotenv;

/// CLI Options.
#[derive(Parser, Debug)]
pub struct Args {
    /// Ethereum node WS endpoint.
    #[arg(long)]
    pub wss: String,

    /// Key for the OpenSea API.
    #[arg(long)]
    pub opensea_api_key: String,

    /// Private key for sending txs.
    #[arg(long)]
    pub private_key: String,

    /// Address of the arb contract.
    #[arg(long)]
    pub arb_contract_address: String,

    /// Percentage of profit to pay in gas.                                 
    #[arg(long)]                                      // 利润的百分比
    pub bid_percentage: u64,
}

impl Default for Args {

    fn default() -> Args {
        dotenv().ok();

        Args {
            wss: env::var("wss").unwrap(),
            opensea_api_key: env::var("opensea_api_key").unwrap(),
            bid_percentage: env::var("bid_percentage").unwrap().parse().unwrap(),
            private_key: env::var("private_key").unwrap(),
            arb_contract_address: env::var("arb_contract_address").unwrap(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    
    // Set up tracing and parse args                                       // 设置追踪和解析参数。
    let filter = filter::Targets::new()
        .with_target("arb", Level::INFO)
        .with_target("artemis_core", Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let args = Args::default();

    println!("args: {:?}", args);
    // Set up ethers provider.
    let ws = Ws::connect(args.wss).await?;                                              // 连接以太坊节点
    let provider = Provider::new(ws);                                                  

    let wallet: LocalWallet = args.private_key.parse().unwrap();                        // 创建以太坊钱包
    let address = wallet.address();                                                     // 获取钱包地址

    let provider = Arc::new(provider.nonce_manager(address).with_signer(wallet));     

    // Set up opensea client.
    let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {                     
        api_key: args.opensea_api_key.clone(),
    });

    // Set up engine.
    let mut engine: Engine<Event, Action> = Engine::default();

    // Set up block collector.                                                          // 设置块收集器。
    let block_collector = Box::new(BlockCollector::new(provider.clone()));
    let block_collector = CollectorMap::new(block_collector, Event::NewBlock);          // 创建块收集器
    engine.add_collector(Box::new(block_collector));                                    // 添加块收集器

    // Set up opensea collector.                                                
    // let opensea_collector = Box::new(OpenseaOrderCollector::new(args.opensea_api_key));
    // let opensea_collector = CollectorMap::new(opensea_collector, |e| Event::OpenseaOrder(Box::new(e)));
    // engine.add_collector(Box::new(opensea_collector));

    // Set up opensea sudo arb strategy.                                                // 设置 opensea sudo arb 策略
    let config = Config {
        arb_contract_address: H160::from_str(&args.arb_contract_address)?, 
        bid_percentage: args.bid_percentage,
    };
    let strategy = OpenseaSudoArb::new(Arc::new(provider.clone()), opensea_client, config);
    engine.add_strategy(Box::new(strategy));

    // Set up flashbots executor.                                                       // 设置 flashbots 执行器
    let executor = Box::new(MempoolExecutor::new(provider.clone()));                    // 创建执行器
    let executor = ExecutorMap::new(executor, |action| match action {                   // 创建执行器映射
        Action::SubmitTx(tx) => Some(tx),                                               // 提交交易
    });
    engine.add_executor(Box::new(executor));

    // Start engine.                                                                    // 启动引擎
    if let Ok(mut set) = engine.run().await {                                           
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
    }
    Ok(())
}
