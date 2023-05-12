// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

// import {ERC20} from "solmate/tokens/ERC20.sol";
import {ILSSVMPairFactoryLike} from "./protocols/LSSVMPairFactory/contracts/ILSSVMPairFactoryLike.sol";
import {LSSVMPairETH} from "./protocols/LSSVMPairFactory/contracts/LSSVMPairETH.sol";
import {LSSVMPair} from "./protocols/LSSVMPairFactory/contracts/LSSVMPair.sol";
import {CurveErrorCodes} from "./protocols/LSSVMPairFactory/contracts/bonding-curves/CurveErrorCodes.sol";

/// @title Sudo Pair Quoter                                                         // sudo对 报价
/// @author FrankieIsLost <frankie@paradigm.xyz>
/// @notice An contract to simplify getting sell quotes from sudo pools             // 一个合约，用于简化从sudo池获取卖出报价
contract SudoPairQuoter {

    /// Note that this contract CANNOT set storage, since we are injecting its bytecode into a eth_call code override 
    /// 请注意，该合约不能设置存储，因为我们正在将其字节码注入到 eth_call 代码覆盖中
    struct SellQuote {
        bool quoteAvailable;
        address nftAddress;
        uint256 price;
    }

    // get sell quote for a single pool
    // 获取单个池的卖出报价
    function getSellQuote(address payable pool_address) public view returns (SellQuote memory sell_quote) {
        ILSSVMPairFactoryLike factory = ILSSVMPairFactoryLike(0xb16c1342E617A5B6E4b631EB114483FDB289c0A4);          
        //check that the pool is an ETH pair
        bool isEthPair = factory.isPair(pool_address, ILSSVMPairFactoryLike.PairVariant.ENUMERABLE_ETH)             // 判断是否是ETH对
            || factory.isPair(pool_address, ILSSVMPairFactoryLike.PairVariant.MISSING_ENUMERABLE_ETH);              // 判断是否是ETH对
        if (!isEthPair) return SellQuote(false, address(0), 0);                                                     // 不是ETH对，返回false
        //check that you can sell into the pair (i.e. it is a token or trade pool)
        LSSVMPairETH pair = LSSVMPairETH(pool_address);                                                             // 获取pair合约
        bool canSellToPool = pair.poolType() == LSSVMPair.PoolType.TOKEN || pair.poolType() == LSSVMPair.PoolType.TRADE; // 判断是否是token或trade池
        if (!canSellToPool) return SellQuote(false, address(0), 0);                                                 // 不是token或trade池，返回false
        //get sell quote and make sure pool holds enough ETH to cover it                                            // 获取卖出报价，并确保池有足够的ETH来支付
        
        (CurveErrorCodes.Error error,,, uint256 outputAmount,) = pair.getSellNFTQuote(1);                           // 获取卖出报价
        if (error != CurveErrorCodes.Error.OK || outputAmount > address(pair).balance) return SellQuote(false, address(0), 0); // 报价错误或者池中没有足够的ETH，返回false
        address nftAddress = address(pair.nft());                                                                   // 获取 nft 地址
        //return valid quote
        return SellQuote(true, nftAddress, outputAmount);                                                           // 返回 true
    }
    //get sell quotes for multiple pools                                    
    // 获取多个池的卖出报价
    function getMultipleSellQuotes(address payable[] memory pool_addresses)                                         
        public
        view
        returns (SellQuote[] memory sell_quotes)
    {
        sell_quotes = new SellQuote[](pool_addresses.length);                                                       // 初始化 sell_quotes
        for (uint256 i = 0; i < pool_addresses.length; i++) {                                                       // 遍历池地址
            sell_quotes[i] = getSellQuote(pool_addresses[i]);                                                       // 获取卖出报价
        }
        return sell_quotes;
        
    }
}
