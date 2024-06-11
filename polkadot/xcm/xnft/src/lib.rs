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

#![recursion_limit = "256"]
// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// pub mod migration;

// #[cfg(test)]
// pub mod mock;
// #[cfg(test)]
// mod tests;

// pub mod weights;

use core::result::Result;
use frame_support::{
	pallet_prelude::*,
	traits::{
		tokens::asset_ops::{common_asset_kinds::Instance, AssetDefinition},
		EnsureOriginWithArg,
	},
};
use frame_system::{pallet_prelude::*, Config as SystemConfig};
use sp_runtime::{traits::StaticLookup, DispatchResult};
use sp_std::prelude::*;
use xcm::latest::*;
use xcm_builder::unique_instances::{derivatives::*, NonFungibleAsset};

pub use pallet::*;
use xcm_executor::traits::{Error as ExecutorError, MatchesInstance};
// pub use weights::WeightInfo;

/// The log target of this pallet.
pub const LOG_TARGET: &'static str = "runtime::xnft";

/// A type alias for the account ID type used in the dispatchable functions of this pallet.
type AccountIdLookupOf<T> = <<T as SystemConfig>::Lookup as StaticLookup>::Source;

type DerivativeIdSourceOf<T, I> = <T as Config<I>>::DerivativeIdSource;

type DerivativeIdOf<T, I> = <T as Config<I>>::DerivativeId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// The in-code storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		// Weight information for extrinsics in this pallet.
		// type WeightInfo: WeightInfo;

		type DerivativeId: Member + Parameter + MaxEncodedLen;

		type DerivativeIdSource: Member + Parameter + MaxEncodedLen;
	}

	#[pallet::storage]
	#[pallet::getter(fn foreign_asset_to_derivative_id_source)]
	pub type ForeignAssetToDerivativeIdSource<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128, AssetId, DerivativeIdSourceOf<T, I>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn derivative_id_source_to_foreign_asset)]
	pub type DerivativeIdSourceToForeignAsset<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128, DerivativeIdSourceOf<T, I>, AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn foreign_nft_to_derivative_id)]
	pub type ForeignNftToDerivativeId<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128, NonFungibleAsset, DerivativeIdOf<T, I>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn derivative_id_to_foreign_nft)]
	pub type DerivativeIdToForeignNft<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128, DerivativeIdOf<T, I>, NonFungibleAsset, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The origin has no permission to do the operation.
		NoPermission,

		/// Can't perform an operation due to the invalid state of storage.
		InvalidState,
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {}
}

impl<T: Config<I>, I: 'static> TryRegisterDerivative<T::DerivativeId> for Pallet<T, I> {
	fn try_register_derivative(
		foreign_asset: &NonFungibleAsset,
		instance_id: &T::DerivativeId,
	) -> DispatchResult {
		<ForeignNftToDerivativeId<T, I>>::insert(foreign_asset, instance_id);
		<DerivativeIdToForeignNft<T, I>>::insert(instance_id, foreign_asset);

		Ok(())
	}

	fn is_derivative_registered(foreign_asset: &NonFungibleAsset) -> bool {
		<ForeignNftToDerivativeId<T, I>>::contains_key(foreign_asset)
	}
}

impl<T: Config<I>, I: 'static> TryDeregisterDerivative<T::DerivativeId> for Pallet<T, I> {
	fn try_deregister_derivative(instance_id: &T::DerivativeId) -> DispatchResult {
		let foreign_asset = <DerivativeIdToForeignNft<T, I>>::take(instance_id)
			.ok_or(pallet::Error::<T, I>::InvalidState)?;

		<ForeignNftToDerivativeId<T, I>>::remove(foreign_asset);

		Ok(())
	}

	fn is_derivative(instance_id: &T::DerivativeId) -> bool {
		<DerivativeIdToForeignNft<T, I>>::contains_key(instance_id)
	}
}

impl<T: Config<I>, I: 'static> MatchesInstance<RegisterDerivativeId<T::DerivativeIdSource>>
	for Pallet<T, I>
{
	fn matches_instance(
		asset: &Asset,
	) -> Result<RegisterDerivativeId<T::DerivativeIdSource>, ExecutorError> {
		match asset.fun {
			Fungibility::NonFungible(asset_instance) => {
				let instance_id_source = <ForeignAssetToDerivativeIdSource<T, I>>::get(&asset.id)
					.ok_or(ExecutorError::AssetNotHandled)?;

				Ok(RegisterDerivativeId {
					foreign_asset: NonFungibleAsset {
						id: asset.id.clone(),
						instance: asset_instance,
					},
					instance_id_source,
				})
			},
			Fungibility::Fungible(_) => Err(ExecutorError::AssetNotHandled),
		}
	}
}
