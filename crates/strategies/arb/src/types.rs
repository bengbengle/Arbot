use arbot_core::{
    collectors::{block_collector::NewBlock, opensea_order_collector::OpenseaOrder},
    executors::mempool_executor::SubmitTxToMempool,
};

use bindings::zone_interface::{
    AdditionalRecipient, 
    BasicOrderParameters
};

use ethers::types::{
    Chain, 
    H160, 
    H256
};

use opensea_v2::types::{
    FulfillListingRequest, 
    FulfillListingResponse, 
    Fulfiller, 
    Listing, 
    ProtocolVersion,
};

/// Core Event enum for the current strategy.                           // 当前策略的 核心事件枚举
#[derive(Debug, Clone)]
pub enum Event {
    NewBlock(NewBlock),                                                 // 新区块 事件
    OpenseaOrder(Box<OpenseaOrder>),                                    // opensea 挂单 事件
}

/// Core Action enum for the current strategy.                          // 当前策略的 核心动作枚举
#[derive(Debug, Clone)]
pub enum Action {
    SubmitTx(SubmitTxToMempool),                                        // 提交交易
}

///  我们需要传递给策略的 配置变量
#[derive(Debug, Clone)]
pub struct Config {
    pub arb_contract_address: H160,                                     // 套利合约 地址
    pub bid_percentage: u64,                                            // 利润的百分比
}

/// 将哈希转换为 fulfill listing 请求 的 函数
pub fn hash_to_fulfill_listing_request(hash: H256) -> FulfillListingRequest {

    FulfillListingRequest {
        listing: Listing {
            hash,
            chain: Chain::Mainnet,
            protocol_version: ProtocolVersion::V1_4,
        },
        fulfiller: Fulfiller {
            address: H160::zero(),
        },
    }
}

/// 将 fulfill listing 响应 转换为 基本订单参数 的 函数
pub fn fulfill_listing_response_to_basic_order_parameters(val: FulfillListingResponse) -> BasicOrderParameters {

    println!("购买成功, fulfill_listing_response_to_basic_order_parameters: {:?}", val);

    let params = val.fulfillment_data.transaction.input_data.parameters;

    let recipients: Vec<AdditionalRecipient> = params
        .additional_recipients
        .iter()
        .map(|ar| AdditionalRecipient {
            recipient: ar.recipient,
            amount: ar.amount,
        })
        .collect();

    BasicOrderParameters {
        consideration_token: params.consideration_token,                                    // 购买 token
        consideration_identifier: params.consideration_identifier,                          // 购买 token 的标识符
        consideration_amount: params.consideration_amount,                                  // 购买 token 的数量
        offerer: params.offerer,                                                            // 卖家
        zone: params.zone,                                                                  // 区域
        offer_token: params.offer_token,                                                    // 出售 token
        offer_identifier: params.offer_identifier,                                          // 出售 token 的标识符
        offer_amount: params.offer_amount,                                                  // 出售 token 的数量
        basic_order_type: params.basic_order_type,                                          // 订单类型
        start_time: params.start_time,                                                      // 开始时间
        end_time: params.end_time,                                                          // 结束时间
        zone_hash: params.zone_hash.into(),                                                 // 区域哈希
        salt: params.salt,                                                                  // 盐
        offerer_conduit_key: params.offerer_conduit_key.into(),                             // 卖家通道密钥
        fulfiller_conduit_key: params.fulfiller_conduit_key.into(),                         // 买家通道密钥
        total_original_additional_recipients: params.total_original_additional_recipients,  // 总原始附加收件人
        additional_recipients: recipients,                                                  // 附加收件人
        signature: params.signature,                                                        // 签名
    }
}
