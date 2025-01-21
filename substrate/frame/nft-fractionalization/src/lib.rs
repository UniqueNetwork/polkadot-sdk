// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # NFT Fractionalization Pallet
//!
//! This pallet provides the basic functionality that should allow users
//! to leverage partial ownership, transfers, and sales, of illiquid assets,
//! whether real-world assets represented by their digital twins, or NFTs,
//! or original NFTs.
//!
//! The functionality allows a user to lock an NFT they own, create a new
//! fungible asset, and mint a set amount of tokens (`fractions`).
//!
//! It also allows the user to burn 100% of the asset and to unlock the NFT
//! into their account.
//!
//! ### Functions
//!
//! * `fractionalize`: Lock the NFT and create and mint a new fungible asset.
//! * `unify`: Return 100% of the asset and unlock the NFT.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

mod types;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

use frame_system::Config as SystemConfig;
pub use pallet::*;
pub use scale_info::Type;
pub use types::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::DispatchResult,
		ensure,
		pallet_prelude::*,
		sp_runtime::traits::{AccountIdConversion, StaticLookup},
		traits::{
			fungible::{
				hold::Mutate as HoldMutateFungible, Inspect as InspectFungible,
				Mutate as MutateFungible,
			},
			fungibles::{
				metadata::{MetadataDeposit, Mutate as MutateMetadata},
				Create, Destroy, Inspect, Mutate,
			},
			tokens::{
				asset_ops::{
					common_strategies::{Bytes, CanTransfer, JustDo, Ownership},
					AssetDefinition, InspectMetadata, Transfer, UpdateMetadata,
				},
				AssetId, Balance as AssetBalance,
				Fortitude::Polite,
				Precision::{BestEffort, Exact},
				Preservation::{Expendable, Preserve},
			},
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{One, Zero};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The currency mechanism, used for paying for deposits.
		type Currency: InspectFungible<Self::AccountId>
			+ MutateFungible<Self::AccountId>
			+ HoldMutateFungible<Self::AccountId, Reason = Self::RuntimeHoldReason>;

		/// Overarching hold reason.
		type RuntimeHoldReason: From<HoldReason>;

		/// The deposit paid by the user locking an NFT. The deposit is returned to the original NFT
		/// owner when the asset is unified and the NFT is unlocked.
		#[pallet::constant]
		type Deposit: Get<DepositOf<Self>>;

		/// The type used to identify an NFT.
		type NftId: Member + Parameter + MaxEncodedLen;

		/// The type used to describe the amount of fractions converted into assets.
		type AssetBalance: AssetBalance;

		/// The type used to identify the assets created during fractionalization.
		type AssetId: AssetId;

		/// Registry for the minted assets.
		type Assets: Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Self::AssetBalance>
			+ Create<Self::AccountId>
			+ Destroy<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateMetadata<Self::AccountId>
			+ MetadataDeposit<DepositOf<Self>>;

		/// Registry for minted NFTs.
		type Nfts: AssetDefinition<Id = Self::NftId>
			+ InspectMetadata<Ownership<Self::AccountId>>
			+ UpdateMetadata<CanTransfer>
			+ Transfer<JustDo<Self::AccountId>>;

		type FractionalizedNfts: AssetDefinition<Id = Self::NftId>
			+ InspectMetadata<Bytes<FractionalizedName>>
			+ InspectMetadata<Bytes<FractionalizedSymbol>>;

		/// The pallet's id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// A set of helper functions for benchmarking.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self::AssetId, Self::NftCollectionId, Self::NftId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Keeps track of the corresponding NFT ID, asset ID and amount minted.
	#[pallet::storage]
	#[pallet::getter(fn nft_to_asset)]
	pub type NftToAsset<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::NftId,
		Details<AssetIdOf<T>, AssetBalanceOf<T>, DepositOf<T>, T::AccountId>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An NFT was successfully fractionalized.
		NftFractionalized {
			nft: T::NftId,
			fractions: AssetBalanceOf<T>,
			asset: AssetIdOf<T>,
			beneficiary: T::AccountId,
		},
		/// An NFT was successfully returned back.
		NftUnified { nft: T::NftId, asset: AssetIdOf<T>, beneficiary: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset ID does not correspond to locked NFT.
		IncorrectAssetId,
		/// The signing account has no permission to do the operation.
		NoPermission,
		/// NFT doesn't exist.
		NftNotFound,
		/// NFT has not yet been fractionalised.
		NftNotFractionalized,
	}

	/// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Reserved for a fractionalized NFT.
		#[codec(index = 0)]
		Fractionalized,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock the NFT and mint a new fungible asset.
		///
		/// The dispatch origin for this call must be Signed.
		/// The origin must be the owner of the NFT they are trying to lock.
		///
		/// `Deposit` funds of sender are reserved.
		///
		/// - `nft_id`: The ID used to identify the NFT.
		/// Is used within the context of `pallet_nfts`.
		/// - `asset_id`: The ID of the new asset. It must not exist.
		/// Is used within the context of `pallet_assets`.
		/// - `beneficiary`: The account that will receive the newly created asset.
		/// - `fractions`: The total issuance of the newly created asset class.
		///
		/// Emits `NftFractionalized` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::fractionalize())]
		pub fn fractionalize(
			origin: OriginFor<T>,
			nft_id: T::NftId,
			asset_id: AssetIdOf<T>,
			beneficiary: AccountIdLookupOf<T>,
			fractions: AssetBalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			let nft_owner = T::Nfts::inspect_metadata(&nft_id, Ownership::default())?;
			ensure!(nft_owner == who, Error::<T>::NoPermission);

			let pallet_account = Self::get_pallet_account();
			let deposit = T::Deposit::get();
			T::Currency::hold(&HoldReason::Fractionalized.into(), &nft_owner, deposit)?;
			Self::do_lock_nft(&nft_id)?;
			Self::do_create_asset(asset_id.clone(), pallet_account.clone())?;
			Self::do_mint_asset(asset_id.clone(), &beneficiary, fractions)?;
			Self::do_set_metadata(asset_id.clone(), &who, &pallet_account, &nft_id)?;

			NftToAsset::<T>::insert(
				&nft_id,
				Details { asset: asset_id.clone(), fractions, asset_creator: nft_owner, deposit },
			);

			Self::deposit_event(Event::NftFractionalized {
				nft: nft_id,
				fractions,
				asset: asset_id,
				beneficiary,
			});

			Ok(())
		}

		/// Burn the total issuance of the fungible asset and return (unlock) the locked NFT.
		///
		/// The dispatch origin for this call must be Signed.
		///
		/// `Deposit` funds will be returned to `asset_creator`.
		///
		/// - `nft_id`: The ID used to identify the NFT within the given collection.
		/// Is used within the context of `pallet_nfts`.
		/// - `asset_id`: The ID of the asset being returned and destroyed. Must match
		/// the original ID of the created asset, corresponding to the NFT.
		/// Is used within the context of `pallet_assets`.
		/// - `beneficiary`: The account that will receive the unified NFT.
		///
		/// Emits `NftUnified` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::unify())]
		pub fn unify(
			origin: OriginFor<T>,
			nft_id: T::NftId,
			asset_id: AssetIdOf<T>,
			beneficiary: AccountIdLookupOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			NftToAsset::<T>::try_mutate_exists(nft_id.clone(), |maybe_details| {
				let details = maybe_details.take().ok_or(Error::<T>::NftNotFractionalized)?;
				ensure!(details.asset == asset_id, Error::<T>::IncorrectAssetId);

				let deposit = details.deposit;
				let asset_creator = details.asset_creator;
				Self::do_burn_asset(asset_id.clone(), &who, details.fractions)?;
				Self::do_unlock_nft(&nft_id, &beneficiary)?;
				T::Currency::release(
					&HoldReason::Fractionalized.into(),
					&asset_creator,
					deposit,
					BestEffort,
				)?;

				Self::deposit_event(Event::NftUnified {
					nft: nft_id,
					asset: asset_id,
					beneficiary,
				});

				Ok(())
			})
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account ID of the pallet.
		///
		/// This actually does computation. If you need to keep using it, then make sure you cache
		/// the value and only call this once.
		fn get_pallet_account() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Prevent further transferring of NFT.
		fn do_lock_nft(nft_id: &T::NftId) -> DispatchResult {
			T::Nfts::update_metadata(nft_id, CanTransfer::default(), false)
		}

		/// Remove the transfer lock and transfer the NFT to the account returning the tokens.
		fn do_unlock_nft(nft_id: &T::NftId, account: &T::AccountId) -> DispatchResult {
			T::Nfts::update_metadata(nft_id, CanTransfer::default(), true)?;
			T::Nfts::transfer(nft_id, JustDo(account.clone()))
		}

		/// Create the new asset.
		fn do_create_asset(asset_id: AssetIdOf<T>, admin: T::AccountId) -> DispatchResult {
			T::Assets::create(asset_id, admin, false, One::one())
		}

		/// Mint the `amount` of tokens with `asset_id` into the beneficiary's account.
		fn do_mint_asset(
			asset_id: AssetIdOf<T>,
			beneficiary: &T::AccountId,
			amount: AssetBalanceOf<T>,
		) -> DispatchResult {
			T::Assets::mint_into(asset_id, beneficiary, amount)?;
			Ok(())
		}

		/// Burn tokens from the account.
		fn do_burn_asset(
			asset_id: AssetIdOf<T>,
			account: &T::AccountId,
			amount: AssetBalanceOf<T>,
		) -> DispatchResult {
			T::Assets::burn_from(asset_id.clone(), account, amount, Expendable, Exact, Polite)?;
			T::Assets::start_destroy(asset_id, None)
		}

		/// Set the metadata for the newly created asset.
		fn do_set_metadata(
			asset_id: AssetIdOf<T>,
			depositor: &T::AccountId,
			pallet_account: &T::AccountId,
			nft_id: &T::NftId,
		) -> DispatchResult {
			let fractionalized_name: &[u8] =
				&T::FractionalizedNfts::inspect_metadata(&nft_id, Bytes(FractionalizedName))?;
			let symbol: &[u8] =
				&T::FractionalizedNfts::inspect_metadata(&nft_id, Bytes(FractionalizedSymbol))?;
			let existential_deposit = T::Currency::minimum_balance();
			let pallet_account_balance = T::Currency::balance(&pallet_account);

			if pallet_account_balance < existential_deposit {
				T::Currency::transfer(&depositor, &pallet_account, existential_deposit, Preserve)?;
			}
			let metadata_deposit = T::Assets::calc_metadata_deposit(fractionalized_name, symbol);
			if !metadata_deposit.is_zero() {
				T::Currency::transfer(&depositor, &pallet_account, metadata_deposit, Preserve)?;
			}
			T::Assets::set(asset_id, &pallet_account, fractionalized_name.into(), symbol.into(), 0)
		}
	}
}

/// Bytes to be used as the name of the fractionalized asset.
pub struct FractionalizedName;

/// Bytes to be used as the symbol of the fractionalized asset.
pub struct FractionalizedSymbol;
