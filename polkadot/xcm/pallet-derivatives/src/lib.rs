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

extern crate alloc;

use alloc::{collections::BTreeMap, vec::Vec};

use frame_support::{
	pallet_prelude::*,
	traits::tokens::asset_ops::{
		common_strategies::WithOrigin, AssetDefinition, Create, CreateStrategy, Destroy,
		DestroyStrategy,
	},
};
use frame_system::pallet_prelude::*;
use sp_runtime::DispatchResult;
use xcm_builder::unique_instances::{
	derivatives::{DerivativesRegistry, IterDerivativesRegistry},
	DerivativesExtra,
};

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// The log target of this pallet.
pub const LOG_TARGET: &'static str = "runtime::xcm::derivatives";

type OriginalOf<T, I> = <T as Config<I>>::Original;
type DerivativeOf<T, I> = <T as Config<I>>::Derivative;
type DerivativeExtraOf<T, I> = <T as Config<I>>::DerivativeExtra;

// FIXME: replace with MetadataMap from XCM when XCM Asset Metadata is implemented
pub type MetadataMap = BTreeMap<Vec<u8>, Vec<u8>>;

pub struct DerivativeAsset<Original, Derivative> {
	pub original: Original,
	pub metadata: MetadataMap,
	_phantom: PhantomData<Derivative>,
}
impl<Original, Derivative> From<(Original, MetadataMap)> for DerivativeAsset<Original, Derivative> {
	fn from((original, metadata): (Original, MetadataMap)) -> Self {
		Self { original, metadata, _phantom: PhantomData }
	}
}

pub type RegistryMapping<Derivative> = Option<Derivative>;

impl<Original, Derivative> CreateStrategy for DerivativeAsset<Original, Derivative> {
	type Success = RegistryMapping<Derivative>;
}

pub struct DestroyWitness(pub MetadataMap);
impl DestroyStrategy for DestroyWitness {
	type Success = ();
}

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

		type Original: Member + Parameter + MaxEncodedLen;
		type Derivative: Member + Parameter + MaxEncodedLen;

		type DerivativeExtra: Member + Parameter + MaxEncodedLen;

		type Ops: AssetDefinition<Id = Self::Original>
			+ Create<
				WithOrigin<Self::RuntimeOrigin, DerivativeAsset<Self::Original, Self::Derivative>>,
			> + Destroy<WithOrigin<Self::RuntimeOrigin, DestroyWitness>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn original_to_derivative)]
	pub type OriginalToDerivative<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, OriginalOf<T, I>, DerivativeOf<T, I>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn derivative_to_original)]
	pub type DerivativeToOriginal<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, DerivativeOf<T, I>, OriginalOf<T, I>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn derivative_extra)]
	pub type DerivativeExtra<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, DerivativeOf<T, I>, DerivativeExtraOf<T, I>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// A derivative is created.
		DerivativeCreated { original: OriginalOf<T, I> },

		/// A mapping between an original asset ID and a local derivative asset ID is created.
		DerivativeMappingCreated { original: OriginalOf<T, I>, derivative_id: DerivativeOf<T, I> },

		/// A derivative is destroyed.
		DerivativeDestroyed { original: OriginalOf<T, I> },
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// A derivative already exists.
		DerivativeAlreadyExists,

		/// Failed to deregister a non-registered derivative.
		NoDerivativeToDeregister,

		/// Failed to find a derivative.
		DerivativeNotFound,

		/// The provided asset metadata is invalid.
		InvalidMetadata,

		/// The provided original asset is invalid.
		InvalidOriginal,
	}

	#[pallet::call(weight(T::WeightInfo))]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::call_index(0)]
		pub fn create_derivative(
			origin: OriginFor<T>,
			original: OriginalOf<T, I>,
			metadata: MetadataMap,
		) -> DispatchResult {
			let success = T::Ops::create(WithOrigin(origin, (original.clone(), metadata).into()))?;

			if let Some(derivative) = success {
				Self::try_register_derivative(&original, &derivative)?;
			}

			Self::deposit_event(Event::<T, I>::DerivativeCreated { original });

			Ok(())
		}

		#[pallet::call_index(1)]
		pub fn destroy_derivative(
			origin: OriginFor<T>,
			original: OriginalOf<T, I>,
			destroy_witness: MetadataMap,
		) -> DispatchResult {
			T::Ops::destroy(&original, WithOrigin(origin, DestroyWitness(destroy_witness)))?;

			Self::try_deregister_derivative_of(&original)
		}
	}
}

impl<T: Config<I>, I: 'static> DerivativesRegistry<OriginalOf<T, I>, DerivativeOf<T, I>>
	for Pallet<T, I>
{
	fn try_register_derivative(
		original: &OriginalOf<T, I>,
		derivative: &DerivativeOf<T, I>,
	) -> DispatchResult {
		ensure!(
			Self::original_to_derivative(original).is_none(),
			Error::<T, I>::DerivativeAlreadyExists,
		);

		<OriginalToDerivative<T, I>>::insert(original, derivative);
		<DerivativeToOriginal<T, I>>::insert(derivative, original);

		Self::deposit_event(Event::<T, I>::DerivativeMappingCreated {
			original: original.clone(),
			derivative_id: derivative.clone(),
		});

		Ok(())
	}

	fn try_deregister_derivative_of(original: &OriginalOf<T, I>) -> DispatchResult {
		let derivative = <OriginalToDerivative<T, I>>::take(&original)
			.ok_or(Error::<T, I>::NoDerivativeToDeregister)?;

		<DerivativeToOriginal<T, I>>::remove(&derivative);
		<DerivativeExtra<T, I>>::remove(&derivative);

		Self::deposit_event(Event::<T, I>::DerivativeDestroyed { original: original.clone() });

		Ok(())
	}

	fn get_derivative(original: &OriginalOf<T, I>) -> Option<DerivativeOf<T, I>> {
		<OriginalToDerivative<T, I>>::get(original)
	}

	fn get_original(derivative: &DerivativeOf<T, I>) -> Option<OriginalOf<T, I>> {
		<DerivativeToOriginal<T, I>>::get(derivative)
	}
}

impl<T: Config<I>, I: 'static> IterDerivativesRegistry<OriginalOf<T, I>, DerivativeOf<T, I>>
	for Pallet<T, I>
{
	fn iter_originals() -> impl Iterator<Item = OriginalOf<T, I>> {
		<OriginalToDerivative<T, I>>::iter_keys()
	}

	fn iter_derivatives() -> impl Iterator<Item = DerivativeOf<T, I>> {
		<OriginalToDerivative<T, I>>::iter_values()
	}

	fn iter() -> impl Iterator<Item = (OriginalOf<T, I>, DerivativeOf<T, I>)> {
		<OriginalToDerivative<T, I>>::iter()
	}
}

impl<T: Config<I>, I: 'static> DerivativesExtra<DerivativeOf<T, I>, DerivativeExtraOf<T, I>>
	for Pallet<T, I>
{
	fn get_derivative_extra(derivative: &DerivativeOf<T, I>) -> Option<DerivativeExtraOf<T, I>> {
		<DerivativeExtra<T, I>>::get(derivative)
	}

	fn set_derivative_extra(
		derivative: &DerivativeOf<T, I>,
		extra: Option<DerivativeExtraOf<T, I>>,
	) -> DispatchResult {
		ensure!(
			<DerivativeToOriginal<T, I>>::contains_key(derivative),
			Error::<T, I>::DerivativeNotFound,
		);

		<DerivativeExtra<T, I>>::set(derivative, extra);

		Ok(())
	}
}

pub trait WeightInfo {
	fn create_derivative() -> Weight;
	fn destroy_derivative() -> Weight;
}

pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
	fn create_derivative() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn destroy_derivative() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
}

pub struct DerivativeErrOps<Id>(PhantomData<Id>);
impl<Id> AssetDefinition for DerivativeErrOps<Id> {
	type Id = Id;
}
impl<D, S: CreateStrategy> Create<S> for DerivativeErrOps<D> {
	fn create(_strategy: S) -> Result<S::Success, DispatchError> {
		Err(DispatchError::BadOrigin)
	}
}
impl<D, S: DestroyStrategy> Destroy<S> for DerivativeErrOps<D> {
	fn destroy(_id: &Self::Id, _strategy: S) -> Result<S::Success, DispatchError> {
		Err(DispatchError::BadOrigin)
	}
}
