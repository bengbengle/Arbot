// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Seaport} from "../src/protocols/Seaport/contracts/Seaport.sol";
import {LSSVMPairETH} from "../src/protocols/LSSVMPairFactory/contracts/LSSVMPairETH.sol";
import {BasicOrderParameters} from "../src/protocols/Seaport/contracts/lib/ConsiderationStructs.sol";
import {IERC721} from "../src/protocols/LSSVMPairFactory/contracts/imports/IERC721.sol";
import {Owned} from "solmate/auth/Owned.sol";

contract SudoOpenseaArb is Owned {

    constructor() Owned(msg.sender) {}

    Seaport seaport = Seaport(0x00000000000001ad428e4906aE43D8F9852d0dD6);

    function executeArb(BasicOrderParameters calldata basicOrder, uint256 paymentValue, address payable sudo_pool) public {
        
        uint256 initialBalance = address(this).balance;                             // get initial balance

        // buy NFT on opensea
        seaport.fulfillBasicOrder{value: paymentValue}(basicOrder);                 // buy NFT

        // set approval for sudo pool 
        IERC721(basicOrder.offerToken).approve(sudo_pool, basicOrder.offerIdentifier);  // approve sudo pool

        // sell into pool
        uint256[] memory nftIds = new uint256[](1);                                 // create array of nft ids
        // set nft id
        nftIds[0] = basicOrder.offerIdentifier;                                     

        // sell nft into pool
        LSSVMPairETH(sudo_pool).swapNFTsForToken(                                   
            nftIds,
            0, // we don't need to set min output since we revert later on if execution isn't profitable
            payable(address(this)),
            false,
            address(0)
        );

        // revert if we didn't make a profit                                            // 如果没有利润，就恢复
        require(address(this).balance > initialBalance, "no profit");
    }

    function withdraw() public onlyOwner {
        payable(msg.sender).transfer(address(this).balance);
    }

    fallback() external payable {}

    receive() external payable {}

}