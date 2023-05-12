use ethers::{
    contract::EthEvent,
    prelude::Lazy,
    types::{Address, TxHash},
};

/// 区块号，sudo 工厂部署时候的区块号
pub const FACTORY_DEPLOYMENT_BLOCK: u64 = 14650730;

/// sudo 工厂地址
pub static LSSVM_PAIR_FACTORY_ADDRESS: Lazy<Address> = Lazy::new(|| {
    "0xb16c1342e617a5b6e4b631eb114483fdb289c0a4".parse().unwrap()
});

/// 事件签名的组，当池子被触发操作时会发出的事件
pub static POOL_EVENT_SIGNATURES: Lazy<Vec<TxHash>> = Lazy::new(|| {
    vec![
        bindings::lssvm_pair::SwapNFTInPairFilter::signature(),
        bindings::lssvm_pair::SwapNFTInPairFilter::signature(),
        bindings::lssvm_pair::SpotPriceUpdateFilter::signature(),
        bindings::lssvm_pair::TokenWithdrawalFilter::signature(),
    ]
});
