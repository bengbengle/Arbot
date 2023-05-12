# Opensea Sudo Arb

在 Seaport 和 Sudoswap 之间实施原子, 跨市场 NFT 套利的策略
我们 监听一系列 deaport 订单，并计算我们是否可以自动完成订单并将 NFT 出售到 sudoswap 池中同时获利

## Strategy 

### Sync

该策略首先通过重建内存中所有 Sudoswap 池的状态来同步其初始状态

1. 从 Sudoswap 工厂部署块开始，过滤所有发出的 "NewPair" 事件以构建完整的池列表 
2. 我们通过 eth_call 字节码注入 使用专门的 报价合约 批量读取所有池的报价 
3. 我们在内存中更新一对 HashMap 的状态, 以快速检索每个 NFT 集合的最佳报价

### Processing

初始同步数据完成后, 我们流式传输以下事件:

1. 新区块: 对于每个新区块, 我们找到所有被触及或创建的 sudo 池, 并在获得新报价后更新内存中的内部状态 
2. 海港订单: 
    我们流式传输海港订单, 过滤出具有有效 sudo 报价的集合上的卖单
    我们计算套利是否可用, 如果可用, 则向我们的原子套利合约提交交易

## Contracts

This strategy relies on two contracts:

1. [`SudoOpenseaArb`](/crates/strategies/arb/contracts/src/SudoOpenseaArb.sol):
    通过调用 `fulfillBasicOrder` 在海港购买 NFT，并通过调用 `swapNFTsForToken` 在 Sudoswap 上出售它来执行原子套利


2. [`SudoPairQuoter`](/crates/strategies/arb/contracts/src/SudoPairQuoter.sol): 
    批量读取合约, 检查 sudo pools 是否有有效的报价 

## Build and Test 

为了运行可靠性测试，您需要访问 `alchemy/infura key`。 您可以使用以下命令运行测试：

```sh
ETH_MAINNET_HTTP=<YOUR_KEY> forge test --root ./contracts
```

You can run the rust tests with the following command: 
您可以使用以下命令运行 Rust 测试：

```sh
cargo test
```

And if you need to regenerate rust bindings for contracts, you can run 
如果你需要为合约重新生成 Rust 绑定，你可以运行

```sh
forge bind --bindings-path ./bindings --root ./contracts --crate-name bindings
```