import {
  Connection,
  Keypair,
  Signer,
  PublicKey,
  Transaction,
  TransactionSignature,
  ConfirmOptions,
  sendAndConfirmRawTransaction,
  RpcResponseAndContext,
  SimulatedTransactionResponse,
  Commitment,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as bs58 from 'bs58'
import * as splToken from '@solana/spl-token'
import fs from 'fs'
import * as anchor from '@project-serum/anchor'
import * as pool_api from './pool_api'

const sleep = (ms : number) => {
    return new Promise(resolve => setTimeout(resolve, ms));
};

async function airdrop(conn : Connection, address : PublicKey){
  let hash = await conn.requestAirdrop(address,LAMPORTS_PER_SOL)
  await conn.confirmTransaction(hash)
  await sleep(10000)
}

async function displayStates(conn : Connection,addresses : PublicKey[]){
    for(let i=0; i<addresses.length; i++){
        let amount = (await conn.getTokenAccountBalance(addresses[i])).value.amount
        console.log(addresses[i].toBase58() + " : " + amount);
    }
}

async function test() {
    console.log("You are clever")
    let conn = new Connection("https://api.devnet.solana.com",'confirmed')
    let creator = Keypair.fromSecretKey(bs58.decode("2pUVo4mVSnebLyLmMTHgPRNbk7rgZki77bsYgbsuuQX9585N4aKNXWJRpyc98qnpgRKRH2hzB8VVnqeffurW39F4"))
    let bidder = Keypair.fromSecretKey(bs58.decode("3UQAPFCYqzR9JN7ERJVLN7dHR6sHZjigr8dfd2dTbCBS8WnfhBefiVGLy2NZhSJJLpiFMbvuoSBBkDaDMtMEXqDc"))
    
    let pool = Keypair.generate()  
    let tokenMint = await splToken.Token.createMint(conn,creator,creator.publicKey,null,9,splToken.TOKEN_PROGRAM_ID)
    let creator_token = await tokenMint.createAccount(creator.publicKey)
    let bidder_token = await tokenMint.createAccount(bidder.publicKey)
    await tokenMint.mintTo(creator_token,creator,[],1000)
    await tokenMint.mintTo(bidder_token,creator,[],1000)

    await pool_api.initPool(
        conn,creator,pool,tokenMint.publicKey,
    )


    let nft_mint = await splToken.Token.createMint(conn,creator,creator.publicKey,null,0,splToken.TOKEN_PROGRAM_ID);
    let nft_seller_account = await nft_mint.createAccount(creator.publicKey);
    let metadata = {
        name : 'nft',
        symbol : 'coff',
        uri : 'https://arweave.net/a03hkxJcMxG4DR-VtkE0WMMXL8-NWluV9-IU5RtMFKc',
        sellerFeeBasis_points : 300, //3% (0 - 10000)
        creators : [
            {address: creator.publicKey, verified:false, share:90},
            {address: bidder.publicKey, verified:false, share:10}
        ],
        isMutable : true,
    }
    await pool_api.mintNft(
        conn,creator,pool.publicKey,nft_mint.publicKey,nft_seller_account,metadata,
    )

    await pool_api.initSaleManager(
        conn,creator,pool.publicKey,nft_mint.publicKey,
    )
    let sale_manager = (await PublicKey.findProgramAddress([pool.publicKey.toBuffer(),nft_mint.publicKey.toBuffer()],pool_api.programId))[0]
    let nft_manager_account = await nft_mint.createAccount(sale_manager)
    let manager_pot = await tokenMint.createAccount(sale_manager)
    await pool_api.sellNft(
        conn,creator,pool.publicKey,nft_mint.publicKey,nft_seller_account,nft_manager_account,manager_pot,100,
    )

    let nft_bidder_account = await nft_mint.createAccount(bidder.publicKey)
    await pool_api.buyNft(
        conn,bidder,pool.publicKey,nft_mint.publicKey,nft_bidder_account,bidder_token,
    )
    await displayStates(conn,[creator_token,bidder_token])
    // // await pool_api.redeemNft(
    // //     conn,creator,pool.publicKey,nft_mint.publicKey,nft_seller_account
    // // )
    await pool_api.withdrawFund(
        conn,creator,sale_manager,creator_token,
    )
    //bidder is a collaborator. So he can withdraw 10%(share).
    await pool_api.withdrawFund(
        conn,bidder,sale_manager,bidder_token,
    )
    await displayStates(conn,[creator_token,bidder_token])

    ////////////////////////////////    Auction      ///////////////////////////////////////
    await pool_api.sellNftByAuction(
        conn,bidder,pool.publicKey,nft_mint.publicKey,nft_bidder_account,nft_manager_account,manager_pot,100,30
    )

    await pool_api.placeBid(
        conn,creator,pool.publicKey,nft_mint.publicKey,creator_token,120,
    )
    await displayStates(conn,[creator_token,bidder_token])
}

test()
