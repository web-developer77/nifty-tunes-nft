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
            clock::UnixTimestamp,
        }      
    },
    metaplex_token_metadata::{
        instruction::{create_metadata_accounts,create_master_edition,update_metadata_accounts,update_primary_sale_happened_via_token},
    },
    spl_token::state,
};
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const MAX_CREATOR_NUM : usize = 6;
pub const CREATOR_SIZE : usize = 32+1+1;
pub const SALE_MANAGER_SIZE : usize = 32+32+32+32+32+8+1+1+1;

//Drag minting.
//Uploader has ability to set price for NFT (must be $1 or more)
//Uploader has ability to create timed NFT drop 


#[program]
pub mod solana_anchor {
    use super::*;

    pub fn mint_nft(
        ctx : Context<MintNft>,
        _bump : u8,
        _data : Metadata,
        ) -> ProgramResult {
        let mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.mint.data.borrow())?;
        let token_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.token_account.data.borrow())?;
        if mint.decimals != 0 {
            return Err(MarketError::InvalidMintAccount.into());
        }
        if mint.supply != 0 {
            return Err(MarketError::InvalidMintAccount.into());
        }
        if token_account.mint != *ctx.accounts.mint.key {
            return Err(MarketError::InvalidTokenAccount.into());
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
                address: *ctx.accounts.owner.key,
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

    pub fn init_sale_property(
        ctx : Context<InitSaleProperty>,
        _bump : u8,
        ) -> ProgramResult {
        let sale_property = &mut ctx.accounts.sale_property;
        sale_property.bump = _bump;
        sale_property.nft_mint = *ctx.accounts.nft_mint.key;
        sale_property.price = 0;
        sale_property.date = None;
        sale_property.sale_state = 0;
        Ok(())
    }

    pub fn set_sale_property(
        ctx : Context<SetSaleProperty>,
        _price : u64,
        _date : Option<UnixTimestamp>
        ) -> ProgramResult {
        let sale_property = &mut ctx.accounts.sale_property;
        sale_property.price = _price;
        sale_property.date = _date;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetSaleProperty<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut, seeds=[(*nft_mint.key).as_ref(),program_id.as_ref()], bump=sale_property.bump,)]
    sale_property : ProgramAccount<'info,SaleProperty>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct InitSaleProperty<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(init, seeds=[(*nft_mint.key).as_ref(),program_id.as_ref()], bump=_bump, payer=owner, space=8+SALE_MANAGER_SIZE)]
    sale_property : ProgramAccount<'info,SaleProperty>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    metadata : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct MintNft<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

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
pub struct SaleProperty{
    pub nft_mint : Pubkey,
    pub price : u64,
    pub date : Option<UnixTimestamp>,
    pub sale_state : u8,
    pub bump : u8,
}

#[account]
pub struct SaleManager{
    pub nft_mint : Pubkey,
    pub sale_pot : Pubkey,
    pub auction_data : Pubkey,
    pub auction_data_extended : Pubkey,
}

#[error]
pub enum MarketError {
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

    #[msg("Invalid seller")]
    InvalidSeller,
}