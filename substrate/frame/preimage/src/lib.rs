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

//! # Preimage Pallet
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! The Preimage pallet allows for the users and the runtime to store the preimage
//! of a hash on chain. This can be used by other pallets for storing and managing
//! large byte-blobs.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod migration;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

extern crate alloc;

use alloc::{borrow::Cow, vec::Vec};
use sp_runtime::{
	traits::{BadOrigin, Hash, Saturating},
	Perbill,
};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::Pays,
	ensure,
	pallet_prelude::Get,
	traits::{
		Consideration, Currency, Defensive, FetchResult, Footprint, PreimageProvider,
		PreimageRecipient, QueryPreimage, ReservableCurrency, StorePreimage,
	},
	BoundedSlice, BoundedVec,
};
use scale_info::TypeInfo;
pub use weights::WeightInfo;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

pub use pallet::*;

/// A type to note whether a preimage is owned by a user or the system.
#[derive(
	Clone,
	Eq,
	PartialEq,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
	DecodeWithMemTracking,
)]
pub enum OldRequestStatus<AccountId, Balance> {
	/// The associated preimage has not yet been requested by the system. The given deposit (if
	/// some) is being held until either it becomes requested or the user retracts the preimage.
	Unrequested { deposit: (AccountId, Balance), len: u32 },
	/// There are a non-zero number of outstanding requests for this hash by this chain. If there
	/// is a preimage registered, then `len` is `Some` and it may be removed iff this counter
	/// becomes zero.
	Requested { deposit: Option<(AccountId, Balance)>, count: u32, len: Option<u32> },
}

/// A type to note whether a preimage is owned by a user or the system.
#[derive(
	Clone,
	Eq,
	PartialEq,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
	DecodeWithMemTracking,
)]
pub enum RequestStatus<AccountId, Ticket> {
	/// The associated preimage has not yet been requested by the system. The given deposit (if
	/// some) is being held until either it becomes requested or the user retracts the preimage.
	Unrequested { ticket: (AccountId, Ticket), len: u32 },
	/// There are a non-zero number of outstanding requests for this hash by this chain. If there
	/// is a preimage registered, then `len` is `Some` and it may be removed iff this counter
	/// becomes zero.
	Requested { maybe_ticket: Option<(AccountId, Ticket)>, count: u32, maybe_len: Option<u32> },
}

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type TicketOf<T> = <T as Config>::Consideration;

/// Maximum size of preimage we can store is 4mb.
pub const MAX_SIZE: u32 = 4 * 1024 * 1024;
/// Hard-limit on the number of hashes that can be passed to `ensure_updated`.
///
/// Exists only for benchmarking purposes.
pub const MAX_HASH_UPGRADE_BULK_COUNT: u32 = 1024;

#[frame_support::pallet]
#[allow(deprecated)]
pub mod pallet {
	use super::*;

	/// The in-code storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Weight information for this pallet.
		type WeightInfo: weights::WeightInfo;

		/// Currency type for this pallet.
		// TODO#1569: Remove.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// An origin that can request a preimage be placed on-chain without a deposit or fee, or
		/// manage existing preimages.
		type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// A means of providing some cost while data is stored on-chain.
		type Consideration: Consideration<Self::AccountId, Footprint>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// A preimage has been noted.
		Noted { hash: T::Hash },
		/// A preimage has been requested.
		Requested { hash: T::Hash },
		/// A preimage has ben cleared.
		Cleared { hash: T::Hash },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Preimage is too large to store on-chain.
		TooBig,
		/// Preimage has already been noted on-chain.
		AlreadyNoted,
		/// The user is not authorized to perform this action.
		NotAuthorized,
		/// The preimage cannot be removed since it has not yet been noted.
		NotNoted,
		/// A preimage may not be removed when there are outstanding requests.
		Requested,
		/// The preimage request cannot be removed since no outstanding requests exist.
		NotRequested,
		/// More than `MAX_HASH_UPGRADE_BULK_COUNT` hashes were requested to be upgraded at once.
		TooMany,
		/// Too few hashes were requested to be upgraded (i.e. zero).
		TooFew,
	}

	/// A reason for this pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// The funds are held as storage deposit for a preimage.
		Preimage,
	}

	/// The request status of a given hash.
	#[deprecated = "RequestStatusFor"]
	#[pallet::storage]
	pub type StatusFor<T: Config> =
		StorageMap<_, Identity, T::Hash, OldRequestStatus<T::AccountId, BalanceOf<T>>>;

	/// The request status of a given hash.
	#[pallet::storage]
	pub type RequestStatusFor<T: Config> =
		StorageMap<_, Identity, T::Hash, RequestStatus<T::AccountId, TicketOf<T>>>;

	#[pallet::storage]
	pub type PreimageFor<T: Config> =
		StorageMap<_, Identity, (T::Hash, u32), BoundedVec<u8, ConstU32<MAX_SIZE>>>;

	#[pallet::call(weight = T::WeightInfo)]
	impl<T: Config> Pallet<T> {
		/// Register a preimage on-chain.
		///
		/// If the preimage was previously requested, no fees or deposits are taken for providing
		/// the preimage. Otherwise, a deposit is taken proportional to the size of the preimage.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::note_preimage(bytes.len() as u32))]
		pub fn note_preimage(origin: OriginFor<T>, bytes: Vec<u8>) -> DispatchResultWithPostInfo {
			// We accept a signed origin which will pay a deposit, or a root origin where a deposit
			// is not taken.
			let maybe_sender = Self::ensure_signed_or_manager(origin)?;
			let (system_requested, _) = Self::note_bytes(bytes.into(), maybe_sender.as_ref())?;
			if system_requested || maybe_sender.is_none() {
				Ok(Pays::No.into())
			} else {
				Ok(().into())
			}
		}

		/// Clear an unrequested preimage from the runtime storage.
		///
		/// If `len` is provided, then it will be a much cheaper operation.
		///
		/// - `hash`: The hash of the preimage to be removed from the store.
		/// - `len`: The length of the preimage of `hash`.
		#[pallet::call_index(1)]
		pub fn unnote_preimage(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			let maybe_sender = Self::ensure_signed_or_manager(origin)?;
			Self::do_unnote_preimage(&hash, maybe_sender)
		}

		/// Request a preimage be uploaded to the chain without paying any fees or deposits.
		///
		/// If the preimage requests has already been provided on-chain, we unreserve any deposit
		/// a user may have paid, and take the control of the preimage out of their hands.
		#[pallet::call_index(2)]
		pub fn request_preimage(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			T::ManagerOrigin::ensure_origin(origin)?;
			Self::do_request_preimage(&hash);
			Ok(())
		}

		/// Clear a previously made request for a preimage.
		///
		/// NOTE: THIS MUST NOT BE CALLED ON `hash` MORE TIMES THAN `request_preimage`.
		#[pallet::call_index(3)]
		pub fn unrequest_preimage(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			T::ManagerOrigin::ensure_origin(origin)?;
			Self::do_unrequest_preimage(&hash)
		}

		/// Ensure that the bulk of pre-images is upgraded.
		///
		/// The caller pays no fee if at least 90% of pre-images were successfully updated.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::ensure_updated(hashes.len() as u32))]
		pub fn ensure_updated(
			origin: OriginFor<T>,
			hashes: Vec<T::Hash>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			ensure!(hashes.len() > 0, Error::<T>::TooFew);
			ensure!(hashes.len() <= MAX_HASH_UPGRADE_BULK_COUNT as usize, Error::<T>::TooMany);

			let updated = hashes.iter().map(Self::do_ensure_updated).filter(|b| *b).count() as u32;
			let ratio = Perbill::from_rational(updated, hashes.len() as u32);

			let pays: Pays = (ratio < Perbill::from_percent(90)).into();
			Ok(pays.into())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn do_ensure_updated(h: &T::Hash) -> bool {
		#[allow(deprecated)]
		let r = match StatusFor::<T>::take(h) {
			Some(r) => r,
			None => return false,
		};
		let n = match r {
			OldRequestStatus::Unrequested { deposit: (who, amount), len } => {
				// unreserve deposit
				T::Currency::unreserve(&who, amount);
				// take consideration
				let Ok(ticket) =
					T::Consideration::new(&who, Footprint::from_parts(1, len as usize))
						.defensive_proof("Unexpected inability to take deposit after unreserved")
				else {
					return true
				};
				RequestStatus::Unrequested { ticket: (who, ticket), len }
			},
			OldRequestStatus::Requested { deposit: maybe_deposit, count, len: maybe_len } => {
				let maybe_ticket = if let Some((who, deposit)) = maybe_deposit {
					// unreserve deposit
					T::Currency::unreserve(&who, deposit);
					// take consideration
					if let Some(len) = maybe_len {
						let Ok(ticket) =
							T::Consideration::new(&who, Footprint::from_parts(1, len as usize))
								.defensive_proof(
									"Unexpected inability to take deposit after unreserved",
								)
						else {
							return true
						};
						Some((who, ticket))
					} else {
						None
					}
				} else {
					None
				};
				RequestStatus::Requested { maybe_ticket, count, maybe_len }
			},
		};
		RequestStatusFor::<T>::insert(h, n);
		true
	}

	/// Ensure that the origin is either the `ManagerOrigin` or a signed origin.
	fn ensure_signed_or_manager(
		origin: T::RuntimeOrigin,
	) -> Result<Option<T::AccountId>, BadOrigin> {
		if T::ManagerOrigin::ensure_origin(origin.clone()).is_ok() {
			return Ok(None)
		}
		let who = ensure_signed(origin)?;
		Ok(Some(who))
	}

	/// Store some preimage on chain.
	///
	/// If `maybe_depositor` is `None` then it is also requested. If `Some`, then it is not.
	///
	/// We verify that the preimage is within the bounds of what the pallet supports.
	///
	/// If the preimage was requested to be uploaded, then the user pays no deposits or tx fees.
	fn note_bytes(
		preimage: Cow<[u8]>,
		maybe_depositor: Option<&T::AccountId>,
	) -> Result<(bool, T::Hash), DispatchError> {
		let hash = T::Hashing::hash(&preimage);
		let len = preimage.len() as u32;
		ensure!(len <= MAX_SIZE, Error::<T>::TooBig);

		Self::do_ensure_updated(&hash);
		// We take a deposit only if there is a provided depositor and the preimage was not
		// previously requested. This also allows the tx to pay no fee.
		let status = match (RequestStatusFor::<T>::get(hash), maybe_depositor) {
			(Some(RequestStatus::Requested { maybe_ticket, count, .. }), _) =>
				RequestStatus::Requested { maybe_ticket, count, maybe_len: Some(len) },
			(Some(RequestStatus::Unrequested { .. }), Some(_)) =>
				return Err(Error::<T>::AlreadyNoted.into()),
			(Some(RequestStatus::Unrequested { ticket, len }), None) => RequestStatus::Requested {
				maybe_ticket: Some(ticket),
				count: 1,
				maybe_len: Some(len),
			},
			(None, None) =>
				RequestStatus::Requested { maybe_ticket: None, count: 1, maybe_len: Some(len) },
			(None, Some(depositor)) => {
				let ticket =
					T::Consideration::new(depositor, Footprint::from_parts(1, len as usize))?;
				RequestStatus::Unrequested { ticket: (depositor.clone(), ticket), len }
			},
		};
		let was_requested = matches!(status, RequestStatus::Requested { .. });
		RequestStatusFor::<T>::insert(hash, status);

		let _ = Self::insert(&hash, preimage)
			.defensive_proof("Unable to insert. Logic error in `note_bytes`?");

		Self::deposit_event(Event::Noted { hash });

		Ok((was_requested, hash))
	}

	// This function will add a hash to the list of requested preimages.
	//
	// If the preimage already exists before the request is made, the deposit for the preimage is
	// returned to the user, and removed from their management.
	fn do_request_preimage(hash: &T::Hash) {
		Self::do_ensure_updated(&hash);
		let (count, maybe_len, maybe_ticket) =
			RequestStatusFor::<T>::get(hash).map_or((1, None, None), |x| match x {
				RequestStatus::Requested { maybe_ticket, mut count, maybe_len } => {
					count.saturating_inc();
					(count, maybe_len, maybe_ticket)
				},
				RequestStatus::Unrequested { ticket, len } => (1, Some(len), Some(ticket)),
			});
		RequestStatusFor::<T>::insert(
			hash,
			RequestStatus::Requested { maybe_ticket, count, maybe_len },
		);
		if count == 1 {
			Self::deposit_event(Event::Requested { hash: *hash });
		}
	}

	// Clear a preimage from the storage of the chain, returning any deposit that may be reserved.
	//
	// If `len` is provided, it will be a much cheaper operation.
	//
	// If `maybe_owner` is provided, we verify that it is the correct owner before clearing the
	// data.
	fn do_unnote_preimage(
		hash: &T::Hash,
		maybe_check_owner: Option<T::AccountId>,
	) -> DispatchResult {
		Self::do_ensure_updated(&hash);
		match RequestStatusFor::<T>::get(hash).ok_or(Error::<T>::NotNoted)? {
			RequestStatus::Requested { maybe_ticket: Some((owner, ticket)), count, maybe_len } => {
				ensure!(maybe_check_owner.map_or(true, |c| c == owner), Error::<T>::NotAuthorized);
				let _ = ticket.drop(&owner);
				RequestStatusFor::<T>::insert(
					hash,
					RequestStatus::Requested { maybe_ticket: None, count, maybe_len },
				);
				Ok(())
			},
			RequestStatus::Requested { maybe_ticket: None, .. } => {
				ensure!(maybe_check_owner.is_none(), Error::<T>::NotAuthorized);
				Self::do_unrequest_preimage(hash)
			},
			RequestStatus::Unrequested { ticket: (owner, ticket), len } => {
				ensure!(maybe_check_owner.map_or(true, |c| c == owner), Error::<T>::NotAuthorized);
				let _ = ticket.drop(&owner);
				RequestStatusFor::<T>::remove(hash);

				Self::remove(hash, len);
				Self::deposit_event(Event::Cleared { hash: *hash });
				Ok(())
			},
		}
	}

	/// Clear a preimage request.
	fn do_unrequest_preimage(hash: &T::Hash) -> DispatchResult {
		Self::do_ensure_updated(&hash);
		match RequestStatusFor::<T>::get(hash).ok_or(Error::<T>::NotRequested)? {
			RequestStatus::Requested { mut count, maybe_len, maybe_ticket } if count > 1 => {
				count.saturating_dec();
				RequestStatusFor::<T>::insert(
					hash,
					RequestStatus::Requested { maybe_ticket, count, maybe_len },
				);
			},
			RequestStatus::Requested { count, maybe_len, maybe_ticket } => {
				debug_assert!(count == 1, "preimage request counter at zero?");
				match (maybe_len, maybe_ticket) {
					// Preimage was never noted.
					(None, _) => RequestStatusFor::<T>::remove(hash),
					// Preimage was noted without owner - just remove it.
					(Some(len), None) => {
						Self::remove(hash, len);
						RequestStatusFor::<T>::remove(hash);
						Self::deposit_event(Event::Cleared { hash: *hash });
					},
					// Preimage was noted with owner - move to unrequested so they can get refund.
					(Some(len), Some(ticket)) => {
						RequestStatusFor::<T>::insert(
							hash,
							RequestStatus::Unrequested { ticket, len },
						);
					},
				}
			},
			RequestStatus::Unrequested { .. } => return Err(Error::<T>::NotRequested.into()),
		}
		Ok(())
	}

	fn insert(hash: &T::Hash, preimage: Cow<[u8]>) -> Result<(), ()> {
		BoundedSlice::<u8, ConstU32<MAX_SIZE>>::try_from(preimage.as_ref())
			.map_err(|_| ())
			.map(|s| PreimageFor::<T>::insert((hash, s.len() as u32), s))
	}

	fn remove(hash: &T::Hash, len: u32) {
		PreimageFor::<T>::remove((hash, len))
	}

	fn have(hash: &T::Hash) -> bool {
		Self::len(hash).is_some()
	}

	fn len(hash: &T::Hash) -> Option<u32> {
		use RequestStatus::*;
		Self::do_ensure_updated(&hash);
		match RequestStatusFor::<T>::get(hash) {
			Some(Requested { maybe_len: Some(len), .. }) | Some(Unrequested { len, .. }) =>
				Some(len),
			_ => None,
		}
	}

	fn fetch(hash: &T::Hash, len: Option<u32>) -> FetchResult {
		let len = len.or_else(|| Self::len(hash)).ok_or(DispatchError::Unavailable)?;
		PreimageFor::<T>::get((hash, len))
			.map(|p| p.into_inner())
			.map(Into::into)
			.ok_or(DispatchError::Unavailable)
	}
}

impl<T: Config> PreimageProvider<T::Hash> for Pallet<T> {
	fn have_preimage(hash: &T::Hash) -> bool {
		Self::have(hash)
	}

	fn preimage_requested(hash: &T::Hash) -> bool {
		Self::do_ensure_updated(hash);
		matches!(RequestStatusFor::<T>::get(hash), Some(RequestStatus::Requested { .. }))
	}

	fn get_preimage(hash: &T::Hash) -> Option<Vec<u8>> {
		Self::fetch(hash, None).ok().map(Cow::into_owned)
	}

	fn request_preimage(hash: &T::Hash) {
		Self::do_request_preimage(hash)
	}

	fn unrequest_preimage(hash: &T::Hash) {
		let res = Self::do_unrequest_preimage(hash);
		debug_assert!(res.is_ok(), "do_unrequest_preimage failed - counter underflow?");
	}
}

impl<T: Config> PreimageRecipient<T::Hash> for Pallet<T> {
	type MaxSize = ConstU32<MAX_SIZE>; // 2**22

	fn note_preimage(bytes: BoundedVec<u8, Self::MaxSize>) {
		// We don't really care if this fails, since that's only the case if someone else has
		// already noted it.
		let _ = Self::note_bytes(bytes.into_inner().into(), None);
	}

	fn unnote_preimage(hash: &T::Hash) {
		// Should never fail if authorization check is skipped.
		let res = Self::do_unrequest_preimage(hash);
		debug_assert!(res.is_ok(), "unnote_preimage failed - request outstanding?");
	}
}

impl<T: Config> QueryPreimage for Pallet<T> {
	type H = T::Hashing;

	fn len(hash: &T::Hash) -> Option<u32> {
		Pallet::<T>::len(hash)
	}

	fn fetch(hash: &T::Hash, len: Option<u32>) -> FetchResult {
		Pallet::<T>::fetch(hash, len)
	}

	fn is_requested(hash: &T::Hash) -> bool {
		Self::do_ensure_updated(&hash);
		matches!(RequestStatusFor::<T>::get(hash), Some(RequestStatus::Requested { .. }))
	}

	fn request(hash: &T::Hash) {
		Self::do_request_preimage(hash)
	}

	fn unrequest(hash: &T::Hash) {
		let res = Self::do_unrequest_preimage(hash);
		debug_assert!(res.is_ok(), "do_unrequest_preimage failed - counter underflow?");
	}
}

impl<T: Config> StorePreimage for Pallet<T> {
	const MAX_LENGTH: usize = MAX_SIZE as usize;

	fn note(bytes: Cow<[u8]>) -> Result<T::Hash, DispatchError> {
		// We don't really care if this fails, since that's only the case if someone else has
		// already noted it.
		let maybe_hash = Self::note_bytes(bytes, None).map(|(_, h)| h);
		// Map to the correct trait error.
		if maybe_hash == Err(DispatchError::from(Error::<T>::TooBig)) {
			Err(DispatchError::Exhausted)
		} else {
			maybe_hash
		}
	}

	fn unnote(hash: &T::Hash) {
		// Should never fail if authorization check is skipped.
		let res = Self::do_unnote_preimage(hash, None);
		debug_assert!(res.is_ok(), "unnote_preimage failed - request outstanding?");
	}
}
