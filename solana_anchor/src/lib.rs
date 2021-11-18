pub mod utils;
use borsh::{BorshDeserialize,BorshSerialize};
use {
    crate::utils::*,
    anchor_lang::{
        prelude::*,
        AnchorDeserialize,
        AnchorSerialize,
        Key,
        solana_program::{
            program::{invoke,invoke_signed},
            program_pack::Pack,
            sysvar::{clock::Clock},
        }      
    },
    metaplex_token_metadata::{
        instruction::{create_metadata_accounts,create_master_edition,update_metadata_accounts,update_primary_sale_happened_via_token},
    },
    spl_token::state,
};
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const POOL_SIZE : usize = 32+1+32;
pub const MAX_CREATOR_NUM : usize = 6;
pub const CREATOR_SIZE : usize = 32+1+1;
pub const MAX_SALE_MANAGER_SIZE : usize = 32+32+32+32+32+8+1+1+1+32+1;
pub const SALE_POT_SIZE : usize = 1+32+32+8+1+32+1+2+CREATOR_SIZE*MAX_CREATOR_NUM;
pub const AUCTION_DATA_SIZE : usize = 8+32+32+1+1;
pub const PREFIX : &str = "auction";
//sell
//buy
//redeem
//withdraw_fund

#[program]
pub mod solana_anchor {
    use super::*;

    pub fn init_pool(
        ctx : Context<InitPool>,
        ) -> ProgramResult {
        let pool = &mut ctx.accounts.pool;
        let sale_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.sale_mint.data.borrow())?;
        pool.owner = *ctx.accounts.owner.key;
        pool.sale_mint = *ctx.accounts.sale_mint.key;
        Ok(())
    }

    pub fn set_authority(
        ctx : Context<SetAuthority>,
        ) -> ProgramResult {
        let pool = &mut ctx.accounts.pool;
        pool.owner = *ctx.accounts.new_owner.key;
        Ok(())
    }

    pub fn mint_nft(
        ctx : Context<MintNft>,
        _data : Metadata,
        ) -> ProgramResult {
        let mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.mint.data.borrow())?;
        let token_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.token_account.data.borrow())?;
        if mint.decimals != 0 {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if mint.supply != 0 {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if token_account.mint != *ctx.accounts.mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }

        spl_token_mint_to(
            TokenMintToParams{
                mint : ctx.accounts.mint.clone(),
                account : ctx.accounts.token_account.clone(),
                owner : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
                amount : 1 as u64,
            }
        )?;

        let mut creators : Vec<metaplex_token_metadata::state::Creator> = 
            vec![metaplex_token_metadata::state::Creator{
                address: ctx.accounts.pool.key(),
                verified : true,
                share : 0,
            }];
        creators.pop();
        for c in _data.creators {

            creators.push(metaplex_token_metadata::state::Creator{
                address : c.address,
                verified : false,
                share : c.share,
            });
        }

        invoke(
            &create_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                *ctx.accounts.mint.key,
                *ctx.accounts.owner.key,
                *ctx.accounts.owner.key,
                *ctx.accounts.owner.key,
                _data.name,
                _data.symbol,
                _data.uri,
                Some(creators),
                _data.seller_fee_basis_points,
                true,
                _data.is_mutable,
            ),
            &[
                ctx.accounts.metadata.clone(),
                ctx.accounts.mint.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.token_program.clone(),
                ctx.accounts.system_program.to_account_info().clone(),
                ctx.accounts.rent.to_account_info().clone(),
            ]
        )?;

        invoke(
            &create_master_edition(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.master_edition.key,
                *ctx.accounts.mint.key,
                *ctx.accounts.owner.key,
                *ctx.accounts.owner.key,
                *ctx.accounts.metadata.key,
                *ctx.accounts.owner.key,
                None,
            ),
            &[
                ctx.accounts.master_edition.clone(),
                ctx.accounts.mint.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.metadata.clone(),
                ctx.accounts.token_program.clone(),
                ctx.accounts.system_program.to_account_info().clone(),
                ctx.accounts.rent.to_account_info().clone(),
            ]
        )?;
        Ok(())
    }

    pub fn init_sale_manager(
        ctx : Context<InitSaleManager>,
        _bump : u8
        ) -> ProgramResult {
        let sale_manager = &mut ctx.accounts.sale_manager;
        sale_manager.bump = _bump;
        sale_manager.pool = ctx.accounts.pool.key();
        sale_manager.nft_mint = *ctx.accounts.nft_mint.key;
        sale_manager.sale_state = 0;
        Ok(())
    }

    pub fn sell_nft(
        ctx : Context<SellNft>,
        _price : u64,
        ) -> ProgramResult {
        let pool = &ctx.accounts.pool;

        let sale_manager_key = ctx.accounts.sale_manager.key();
        let sale_manager = &mut ctx.accounts.sale_manager;
        sale_manager.sale_pot = ctx.accounts.sale_pot.key();
        let sale_pot = &mut ctx.accounts.sale_pot;
        let manager_pot : state::Account = state::Account::unpack_from_slice(&ctx.accounts.manager_pot.data.borrow())?;
        let nft_seller_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_seller_token.data.borrow())?;
        let nft_manager_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_manager_token.data.borrow())?;
        let metadata : metaplex_token_metadata::state::Metadata =  metaplex_token_metadata::state::Metadata::from_account_info(&ctx.accounts.metadata)?;
        if nft_seller_token.owner != *ctx.accounts.owner.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_seller_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_manager_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if manager_pot.mint != pool.sale_mint {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if manager_pot.owner != sale_manager_key {
            return Err(PoolError::InvalidTokenAccount.into());
        }

        sale_manager.seller=*ctx.accounts.owner.key;
        sale_manager.price=_price;
        sale_manager.sale_state =1;
        sale_manager.nft_pot = *ctx.accounts.nft_manager_token.key;
        sale_manager.is_auction_mode = false;

        // sale_manager.is_primary = metadata.primary_sale_happened;
        sale_pot.sale_manager = sale_manager_key;
        sale_pot.is_used = false;
        sale_pot.price = _price;
        sale_pot.pool_pot=*ctx.accounts.manager_pot.key;
        sale_pot.is_primary = !metadata.primary_sale_happened;
        if sale_pot.is_primary {
            sale_pot.seller_verified = true;
        } else {
            sale_pot.seller_verified = false;
        }
        sale_pot.seller = *ctx.accounts.owner.key;
        if let Some(creators) = metadata.data.creators{
            for c in creators {
                sale_pot.creators.push(Creator{
                    address : c.address,
                    verified : false,
                    share : c.share,
                })
            }
        }
        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.nft_seller_token.clone(),
                destination : ctx.accounts.nft_manager_token.clone(),
                amount : 1,
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;
        invoke(
            &update_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                *ctx.accounts.owner.key,
                Some(sale_manager_key),
                None,
                None,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                ctx.accounts.owner.clone(),
            ]
        )?;
        Ok(())
    }

    pub fn buy_nft(
        ctx : Context<BuyNft>,
        ) -> ProgramResult {
        let pool = &ctx.accounts.pool;
        let pool_info = ctx.accounts.pool.to_account_info().clone();
        let sale_manager_info1 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_info2 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_info3 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_key = ctx.accounts.sale_manager.key();
        let sale_manager = &mut ctx.accounts.sale_manager;
        let sale_pot = &mut ctx.accounts.sale_pot;
        let nft_manager_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_manager_token.data.borrow())?;
        let nft_bidder_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_bidder_token.data.borrow())?;
        let manager_pot : state::Account = state::Account::unpack_from_slice(&ctx.accounts.manager_pot.data.borrow())?;
        let bidder_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.bidder_token.data.borrow())?;
        let metadata : metaplex_token_metadata::state::Metadata =  metaplex_token_metadata::state::Metadata::from_account_info(&ctx.accounts.metadata)?;

        if nft_manager_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_bidder_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if manager_pot.mint != pool.sale_mint {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if bidder_token.mint != pool.sale_mint {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if bidder_token.owner != *ctx.accounts.owner.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if bidder_token.amount < sale_manager.price {
            return Err(PoolError::NotEnoughTokenAmount.into());
        }
        if sale_manager.sale_state != 1 {
            return Err(PoolError::InvalidSaleState.into());
        }
        if sale_pot.pool_pot != *ctx.accounts.manager_pot.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if sale_manager.nft_mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if sale_manager.seller == *ctx.accounts.owner.key {
            return Err(PoolError::InvalidBidder.into());
        }
        if sale_manager.is_auction_mode {
            return Err(PoolError::InvalidAuctionMode.into());
        }
        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.bidder_token.clone(),
                destination : ctx.accounts.manager_pot.clone(),
                amount : sale_manager.price,
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;

        let sale_manager_seeds = &[
            sale_manager.pool.as_ref(),
            sale_manager.nft_mint.as_ref(),
            &[sale_manager.bump]
        ];
        invoke_signed(
            &update_primary_sale_happened_via_token(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                sale_manager_key,
                *ctx.accounts.nft_manager_token.key,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                sale_manager_info3,
                ctx.accounts.nft_manager_token.clone(),
            ],
            &[sale_manager_seeds]
        )?;
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.nft_manager_token.clone(),
                destination : ctx.accounts.nft_bidder_token.clone(),
                amount : 1,
                authority : sale_manager_info1,
                authority_signer_seeds : sale_manager_seeds,
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;

        invoke_signed(
            &update_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                sale_manager_key,
                Some(*ctx.accounts.owner.key),
                None,
                None,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                sale_manager_info2,
            ],
            &[sale_manager_seeds]
        )?;

        sale_manager.sale_state=2;
        sale_pot.is_used=true;        
        Ok(())
    }

    pub fn redeem_nft(
        ctx : Context<RedeemNft>
        ) -> ProgramResult {
        let sale_manager_info1 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_info2 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_key = ctx.accounts.sale_manager.key();
        let sale_manager = &mut ctx.accounts.sale_manager;
        
        let nft_manager_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_manager_token.data.borrow())?;
        let nft_seller_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_seller_token.data.borrow())?;
        let metadata : metaplex_token_metadata::state::Metadata =  metaplex_token_metadata::state::Metadata::from_account_info(&ctx.accounts.metadata)?;
        if sale_manager.sale_state != 1 {
            return Err(PoolError::InvalidSaleState.into());
        }
        if sale_manager.nft_mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if nft_manager_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_seller_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if sale_manager.seller != *ctx.accounts.owner.key {
            return Err(PoolError::InvalidSeller.into())
        }        

        let sale_manager_seeds = &[
            sale_manager.pool.as_ref(),
            sale_manager.nft_mint.as_ref(),
            &[sale_manager.bump]
        ];
        
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.nft_manager_token.clone(),
                destination : ctx.accounts.nft_seller_token.clone(),
                amount : 1,
                authority : sale_manager_info1,
                authority_signer_seeds : sale_manager_seeds,
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;
        invoke_signed(
            &update_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                sale_manager_key,
                Some(*ctx.accounts.owner.key),
                None,
                None,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                sale_manager_info2,
            ],
            &[sale_manager_seeds]
        )?;
        sale_manager.sale_state=0;
        Ok(())
    }

    pub fn withdraw_fund(
        ctx : Context<WithdrawFund>,
        ) -> ProgramResult {
        let sale_manager_info = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager = &ctx.accounts.sale_manager;
        let sale_pot = &mut ctx.accounts.sale_pot;
        let sale_manager_seeds = &[
            sale_manager.pool.as_ref(),
            sale_manager.nft_mint.as_ref(),
            &[sale_manager.bump]
        ];
        if !sale_pot.is_used {
            return Err(PoolError::NotAllowedWithdrawFund.into());
        }

        let pool_pot : state::Account = state::Account::unpack_from_slice(&ctx.accounts.pool_pot.data.borrow())?;
        let mut amount : u64 = 0;
        if sale_pot.is_primary {
            // sale_pot.seller_verified = true;
            let mut share : u8 = 0;
            for i in 0..sale_pot.creators.len(){
                if sale_pot.creators[i].verified==false && sale_pot.creators[i].address==*ctx.accounts.owner.key {
                    sale_pot.creators[i].verified=true;
                    share = sale_pot.creators[i].share
                }
            }
            amount =((sale_pot.price as f64) * (share as f64) / (100.0 as f64)) as u64;
        } else {
            if sale_pot.seller_verified==false && sale_pot.seller == *ctx.accounts.owner.key {
                amount = ((sale_pot.price as f64) * ((10000-sale_pot.seller_fee_basis_points) as f64) / (10000 as f64)) as u64;
                sale_pot.seller_verified = true;
            }
            let mut share : u8 = 0;
            for i in 0..sale_pot.creators.len(){
                if sale_pot.creators[i].verified==false && sale_pot.creators[i].address==*ctx.accounts.owner.key {
                    sale_pot.creators[i].verified=true;
                    share = sale_pot.creators[i].share
                }
            }
            amount = amount + ((sale_pot.price as f64) * (sale_pot.seller_fee_basis_points as f64) / (10000 as f64) * (share as f64) / (100.0 as f64)) as u64
        }
        if amount == 0 {
            return Err(PoolError::InvalidAmount.into());
        }

        if amount > pool_pot.amount {
            amount = pool_pot.amount;
        }
        
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.pool_pot.clone(),
                destination : ctx.accounts.withdraw_pot.clone(),
                amount : amount,
                authority : sale_manager_info,
                authority_signer_seeds : sale_manager_seeds,
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;

        Ok(())
    }

    pub fn sell_nft_by_auction(
        ctx : Context<SellNftByAuction>,
        _price : u64,
        _ended_at : i64,
        ) -> ProgramResult {
        let pool = &ctx.accounts.pool;
        let sale_manager_key = ctx.accounts.sale_manager.key();
        let sale_manager = &mut ctx.accounts.sale_manager;
        sale_manager.sale_pot = ctx.accounts.sale_pot.key();
        sale_manager.auction_data = ctx.accounts.auction_data.key();
        let sale_pot = &mut ctx.accounts.sale_pot;
        let auction_data = &mut ctx.accounts.auction_data;
        let manager_pot : state::Account = state::Account::unpack_from_slice(&ctx.accounts.manager_pot.data.borrow())?;
        let nft_seller_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_seller_token.data.borrow())?;
        let nft_manager_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_manager_token.data.borrow())?;
        let metadata : metaplex_token_metadata::state::Metadata =  metaplex_token_metadata::state::Metadata::from_account_info(&ctx.accounts.metadata)?;
        let clock = Clock::from_account_info(&ctx.accounts.clock_sysvar)?;
        if nft_seller_token.owner != *ctx.accounts.owner.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_seller_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_manager_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if manager_pot.mint != pool.sale_mint {
            return Err(PoolError::InvalidMintAccount.into());
        }
        if manager_pot.owner != sale_manager_key {
            return Err(PoolError::InvalidTokenAccount.into());
        }

        sale_manager.seller=*ctx.accounts.owner.key;
        sale_manager.price=_price;
        sale_manager.sale_state =1;
        sale_manager.nft_pot = *ctx.accounts.nft_manager_token.key;
        sale_manager.is_auction_mode = true;

        auction_data.ended_at=_ended_at+clock.unix_timestamp;
        auction_data.auction_state = 1;
        auction_data.gap_tick_percentage =10;

        // sale_manager.is_primary = metadata.primary_sale_happened;
        sale_pot.sale_manager = sale_manager_key;
        sale_pot.is_used = false;
        sale_pot.price = _price;
        sale_pot.pool_pot=*ctx.accounts.manager_pot.key;
        sale_pot.is_primary = !metadata.primary_sale_happened;
        if sale_pot.is_primary {
            sale_pot.seller_verified = true;
        } else {
            sale_pot.seller_verified = false;
        }
        sale_pot.seller = *ctx.accounts.owner.key;
        if let Some(creators) = metadata.data.creators{
            for c in creators {
                sale_pot.creators.push(Creator{
                    address : c.address,
                    verified : false,
                    share : c.share,
                })
            }
        }
        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.nft_seller_token.clone(),
                destination : ctx.accounts.nft_manager_token.clone(),
                amount : 1,
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;
        invoke(
            &update_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                *ctx.accounts.owner.key,
                Some(sale_manager_key),
                None,
                None,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                ctx.accounts.owner.clone(),
            ]
        )?;
        Ok(())        
    }
    
    pub fn place_bid(
        ctx : Context<PlaceBid>,
        _price : u64,
        ) -> ProgramResult {
        let sale_manager_info = ctx.accounts.sale_manager.to_account_info().clone();
        let pool = &ctx.accounts.pool;
        let sale_manager = &mut ctx.accounts.sale_manager;
        let sale_pot = &mut ctx.accounts.sale_pot;
        let auction_data = &mut ctx.accounts.auction_data;

        let manager_pot : state::Account = state::Account::unpack_from_slice(&ctx.accounts.manager_pot.data.borrow())?;
        let bidder_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.bidder_token.data.borrow())?;
        let clock = Clock::from_account_info(&ctx.accounts.clock_sysvar)?;
        
        if sale_manager.pool != pool.key() {
            return Err(PoolError::InvalidPoolAccount.into());
        }       
        if !sale_manager.is_auction_mode {
            return Err(PoolError::InvalidAuctionMode.into());
        }
        if manager_pot.mint != pool.sale_mint {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if bidder_token.mint != pool.sale_mint {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if sale_manager.sale_state != 1 {
            return Err(PoolError::InvalidSaleState.into());
        }
        if sale_pot.pool_pot != *ctx.accounts.manager_pot.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if sale_manager.seller == *ctx.accounts.owner.key {
            return Err(PoolError::InvalidBidder.into());
        }
        if bidder_token.amount < _price {
            return Err(PoolError::NotEnoughTokenAmount.into());
        }
        if _price < sale_manager.price {
            return Err(PoolError::NotEnoughTokenAmount.into());
        }
        if auction_data.ended_at < clock.unix_timestamp {
            auction_data.auction_state = 3;
            sale_pot.is_used = true;
            return Err(PoolError::EndedAuction.into());    
        }
        if auction_data.auction_state == 3 {
            return Err(PoolError::EndedAuction.into());
        }
        
        if auction_data.auction_state == 2 {
            if _price < sale_manager.price * (100u64 + auction_data.gap_tick_percentage as u64) / 100u64 {
                return Err(PoolError::NotEnoughTokenAmountForGapTick.into());
            }
            let sale_manager_seeds = &[
                sale_manager.pool.as_ref(),
                sale_manager.nft_mint.as_ref(),
                &[sale_manager.bump]
            ];
            if *ctx.accounts.prev_bidder_token.key != auction_data.last_bidder_token {
                return Err(PoolError::InvalidPrevBidderToken.into());
            }
            spl_token_transfer(
                TokenTransferParams{
                    source : ctx.accounts.manager_pot.clone(),
                    destination : ctx.accounts.prev_bidder_token.clone(),
                    amount : sale_manager.price,
                    authority : sale_manager_info,
                    authority_signer_seeds : sale_manager_seeds,
                    token_program : ctx.accounts.token_program.clone(),
                }
            )?;
        }

        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.bidder_token.clone(),
                destination : ctx.accounts.manager_pot.clone(),
                amount : _price,
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;

        sale_manager.price = _price;
        sale_manager.sale_state = 2;
        sale_pot.price = _price;

        auction_data.last_bidder = *ctx.accounts.owner.key;
        auction_data.last_bidder_token = *ctx.accounts.bidder_token.key;
        auction_data.auction_state = 2;

        Ok(())
    }

    pub fn claim_bid(
        ctx : Context<ClaimBid>,
        ) -> ProgramResult {
        let sale_manager_info1 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_info2 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_info3 = ctx.accounts.sale_manager.to_account_info().clone();
        let sale_manager_key = ctx.accounts.sale_manager.key();
        let sale_manager = &mut ctx.accounts.sale_manager;
        let sale_pot = &mut ctx.accounts.sale_pot;
        let clock = Clock::from_account_info(&ctx.accounts.clock_sysvar)?;
        if sale_manager.auction_data != ctx.accounts.auction_data.key() {
            return Err(PoolError::InvalidAuctionDataAccount.into());
        }
        let auction_data = &mut ctx.accounts.auction_data;
        let nft_manager_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_manager_token.data.borrow())?;
        let nft_bidder_token : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_bidder_token.data.borrow())?;
        if nft_manager_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_bidder_token.mint != *ctx.accounts.nft_mint.key {
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if sale_manager.sale_state == 2 {
            return Err(PoolError::InvalidSaleState.into());
        }
        if !sale_manager.is_auction_mode {
            return Err(PoolError::InvalidAuctionMode.into());
        }
        if auction_data.last_bidder != *ctx.accounts.owner.key {
            return Err(PoolError::InvalidBidder.into());
        }
        if auction_data.ended_at < clock.unix_timestamp {
            auction_data.auction_state = 3;
            sale_pot.is_used = true;
        }
        if auction_data.auction_state != 3 {
            return Err(PoolError::InvalidAuctionState.into());
        }


        let sale_manager_seeds = &[
            sale_manager.pool.as_ref(),
            sale_manager.nft_mint.as_ref(),
            &[sale_manager.bump]
        ];
        invoke_signed(
            &update_primary_sale_happened_via_token(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                sale_manager_key,
                *ctx.accounts.nft_manager_token.key,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                sale_manager_info3,
                ctx.accounts.nft_manager_token.clone(),
            ],
            &[sale_manager_seeds]
        )?;
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.nft_manager_token.clone(),
                destination : ctx.accounts.nft_bidder_token.clone(),
                amount : 1,
                authority : sale_manager_info1,
                authority_signer_seeds : sale_manager_seeds,
                token_program : ctx.accounts.token_program.clone(),
            }
        )?;

        invoke_signed(
            &update_metadata_accounts(
                *ctx.accounts.token_metadata_program.key,
                *ctx.accounts.metadata.key,
                sale_manager_key,
                Some(*ctx.accounts.owner.key),
                None,
                None,
            ),
            &[
                ctx.accounts.token_metadata_program.clone(),
                ctx.accounts.metadata.clone(),
                sale_manager_info2,
            ],
            &[sale_manager_seeds]
        )?;
        Ok(())
    }

    // pub fn check_auction_ended(
    //     ctx : Context<CheckAuctionEnded>,
    //     ) -> ProgramResult {
    //     let auction_data = &mut ctx.accounts.auction_data;
    //     let clock = Clock::from_account_info(&ctx.accounts.clock_sysvar)?;
    //     if auction_data.ended_at < clock.unix_timestamp {
    //         auction_data.auction_state = 3;
    //         sale_pot.is_used = true;
    //     }
    //     Ok(())
    // }
}

// #[derive(Accounts)]
// pub struct CheckAuctionEnded<'info> {
//     #[account(mut,signer)]
//     owner : AccountInfo<'info>,

//     #[account(mut)]
//     auction_data : ProgramAccount<'info,AuctionData>,

//     clock_sysvar : AccountInfo<'info>,    
// }

#[derive(Accounts)]
pub struct ClaimBid<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut,seeds=[sale_manager.pool.key().as_ref(),sale_manager.nft_mint.as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(mut)]
    sale_pot : ProgramAccount<'info,SalePot>,

    #[account(mut)]
    auction_data : ProgramAccount<'info,AuctionData>,

    #[account(mut,owner=spl_token::id())]
    nft_manager_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    nft_bidder_token : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    clock_sysvar : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct PlaceBid<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,  

    #[account(mut,seeds=[pool.key().as_ref(),sale_manager.nft_mint.as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(mut)]
    sale_pot : ProgramAccount<'info,SalePot>,

    #[account(mut)]
    auction_data : ProgramAccount<'info,AuctionData>,   

    #[account(mut,owner=spl_token::id())]
    manager_pot : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    bidder_token : AccountInfo<'info>,

    #[account(mut)]
    prev_bidder_token : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    clock_sysvar : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SellNftByAuction<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut,seeds=[pool.key().as_ref(),(*nft_mint.key).as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(init,payer=owner,space=8+AUCTION_DATA_SIZE)]
    auction_data : ProgramAccount<'info,AuctionData>,

    #[account(init,payer=owner,space=8+SALE_POT_SIZE)]
    sale_pot : ProgramAccount<'info,SalePot>,

    #[account(mut,owner=spl_token::id())]
    nft_seller_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    nft_manager_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    manager_pot : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    system_program : Program<'info,System>,

    clock_sysvar : AccountInfo<'info>,    
}

#[derive(Accounts)]
pub struct WithdrawFund<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(mut)]
    sale_pot : ProgramAccount<'info,SalePot>,

    #[account(mut,owner=spl_token::id())]
    pool_pot : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    withdraw_pot : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,     
}

#[derive(Accounts)]
pub struct RedeemNft<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut,seeds=[pool.key().as_ref(),(*nft_mint.key).as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(mut,owner=spl_token::id())]
    nft_seller_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    nft_manager_token : AccountInfo<'info>,  

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,  
}

#[derive(Accounts)]
pub struct BuyNft<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut,seeds=[pool.key().as_ref(),(*nft_mint.key).as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(mut)]
    sale_pot : ProgramAccount<'info,SalePot>,    

    #[account(mut,owner=spl_token::id())]
    nft_manager_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    nft_bidder_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    manager_pot : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    bidder_token : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SellNft<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut,seeds=[pool.key().as_ref(),(*nft_mint.key).as_ref()],bump=sale_manager.bump)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    #[account(init,payer=owner,space=8+SALE_POT_SIZE)]
    sale_pot : ProgramAccount<'info,SalePot>,

    #[account(mut,owner=spl_token::id())]
    nft_seller_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    nft_manager_token : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    manager_pot : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct InitSaleManager<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(init,seeds=[pool.key().as_ref(),(*nft_mint.key).as_ref()],bump=_bump,payer=owner,space=8+MAX_SALE_MANAGER_SIZE)]
    sale_manager : ProgramAccount<'info,SaleManager>,

    system_program : Program<'info,System>
}

#[derive(Accounts)]
pub struct MintNft<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    pool : ProgramAccount<'info,Pool>,

    #[account(mut,owner=spl_token::id())]
    mint : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    token_account : AccountInfo<'info>,

    #[account(mut)]
    metadata : AccountInfo<'info>,

    #[account(mut)]
    master_edition : AccountInfo<'info>,

    #[account(address=metaplex_token_metadata::id())]
    token_metadata_program : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    system_program : Program<'info,System>,

    rent : Sysvar<'info,Rent>,
}

#[derive(Accounts)]
pub struct SetAuthority<'info>{
    #[account(mut, has_one=owner)]
    pool : ProgramAccount<'info,Pool>,

    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut)]
    new_owner : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitPool<'info>{
    #[account(init, payer=owner, space=8+POOL_SIZE)]
    pool : ProgramAccount<'info,Pool>,

    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    sale_mint : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

#[account]
pub struct Pool{
    pub owner : Pubkey,
    pub sale_mint : Pubkey,
}


#[derive(AnchorSerialize,AnchorDeserialize,Clone)]
pub struct Creator {
    pub address : Pubkey,
    pub verified : bool,
    pub share : u8,
}

#[derive(AnchorSerialize,AnchorDeserialize,Clone,Default)]
pub struct Metadata{
    pub name : String,
    pub symbol : String,
    pub uri : String,
    pub seller_fee_basis_points : u16,
    pub creators : Vec<Creator>,
    pub is_mutable : bool,
}


#[account]
pub struct SaleManager{
    pub pool : Pubkey,
    pub seller : Pubkey,
    pub nft_mint : Pubkey,
    pub nft_pot : Pubkey,
    pub sale_pot : Pubkey,
    pub price : u64,
    pub sale_state : u8,
    pub is_primary : bool,
    pub is_auction_mode : bool,
    pub auction_data : Pubkey,
    pub bump : u8,
}

#[account]
pub struct SalePot{
    pub is_used : bool,
    pub sale_manager : Pubkey,
    pub pool_pot : Pubkey,
    pub price : u64,
    pub is_primary : bool,
    pub seller : Pubkey,
    pub seller_verified : bool,
    pub seller_fee_basis_points : u16,
    pub creators : Vec<Creator>,
}

#[account]
pub struct AuctionData{
    pub ended_at : i64,
    pub last_bidder : Pubkey,
    pub last_bidder_token : Pubkey,
    pub auction_state : u8,
    pub gap_tick_percentage : u8,
}

#[error]
pub enum PoolError {
    #[msg("Token mint to failed")]
    TokenMintToFailed,

    #[msg("Token set authority failed")]
    TokenSetAuthorityFailed,

    #[msg("Token transfer failed")]
    TokenTransferFailed,

    #[msg("Invalid mint account")]
    InvalidMintAccount,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Invalid pool account")]
    InvalidPoolAccount,

    #[msg("Not whitelisted")]
    NotWhitelisted,

    #[msg("Mint amount is zero")]
    MintAmountIsZero,

    #[msg("Already on sale")]
    AlreadyTrading,

    #[msg("Invalid price")]
    InvalidPrice,

    #[msg("Invalid sale state")]
    InvalidSaleState,

    #[msg("Not enough token amount")]
    NotEnoughTokenAmount,

    #[msg("Invalid bidder")]
    InvalidBidder,

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("InvalidSeller")]
    InvalidSeller,

    #[msg("InvalidAuctionMode")]
    InvalidAuctionMode,

    #[msg("Not enough token amount for gap tick")]
    NotEnoughTokenAmountForGapTick,

    #[msg("Invalid prev bidder token")]
    InvalidPrevBidderToken,

    #[msg("Ended auction")]
    EndedAuction,

    #[msg("Not allowed to withdraw fund")]
    NotAllowedWithdrawFund,

    #[msg("Invalid auction data account")]
    InvalidAuctionDataAccount,

    #[msg("Invalid auction state")]
    InvalidAuctionState,
}