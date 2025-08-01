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

//! # Multisig pallet
//! A pallet for doing multisig dispatch.
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! This pallet contains functionality for multi-signature dispatch, a (potentially) stateful
//! operation, allowing multiple signed
//! origins (accounts) to coordinate and dispatch a call from a well-known origin, derivable
//! deterministically from the set of account IDs and the threshold number of accounts from the
//! set that must approve it. In the case that the threshold is just one then this is a stateless
//! operation. This is useful for multisig wallets where cryptographic threshold signatures are
//! not available or desired.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `as_multi` - Approve and if possible dispatch a call from a composite origin formed from a
//!   number of signed origins.
//! * `approve_as_multi` - Approve a call from a composite origin.
//! * `cancel_as_multi` - Cancel a call from a composite origin.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
pub mod migrations;
mod tests;
pub mod weights;

extern crate alloc;
use alloc::{boxed::Box, vec, vec::Vec};
use frame::{
	prelude::*,
	traits::{Currency, ReservableCurrency},
};
use frame_system::RawOrigin;
pub use weights::WeightInfo;

/// Re-export all pallet items.
pub use pallet::*;

/// The log target of this pallet.
pub const LOG_TARGET: &'static str = "runtime::multisig";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			concat!("[{:?}] ✍️ ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type BlockNumberFor<T> =
	<<T as Config>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;

/// A global extrinsic index, formed as the extrinsic index within a block, together with that
/// block's height. This allows a transaction in which a multisig operation of a particular
/// composite was created to be uniquely identified.
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Encode,
	Decode,
	DecodeWithMemTracking,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct Timepoint<BlockNumber> {
	/// The height of the chain at the point in time.
	pub height: BlockNumber,
	/// The index of the extrinsic at the point in time.
	pub index: u32,
}

/// An open multisig operation.
#[derive(
	Clone,
	Eq,
	PartialEq,
	Encode,
	Decode,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(MaxApprovals))]
pub struct Multisig<BlockNumber, Balance, AccountId, MaxApprovals>
where
	MaxApprovals: Get<u32>,
{
	/// The extrinsic when the multisig operation was opened.
	pub when: Timepoint<BlockNumber>,
	/// The amount held in reserve of the `depositor`, to be returned once the operation ends.
	pub deposit: Balance,
	/// The account who opened it (i.e. the first to approve it).
	pub depositor: AccountId,
	/// The approvals achieved so far, including the depositor. Always sorted.
	pub approvals: BoundedVec<AccountId, MaxApprovals>,
}

type CallHash = [u8; 32];

enum CallOrHash<T: Config> {
	Call(<T as Config>::RuntimeCall),
	Hash([u8; 32]),
}

#[frame::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The overarching call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;

		/// The currency mechanism.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// The base amount of currency needed to reserve for creating a multisig execution or to
		/// store a dispatch call for later.
		///
		/// This is held for an additional storage item whose value size is
		/// `4 + sizeof((BlockNumber, Balance, AccountId))` bytes and whose key size is
		/// `32 + sizeof(AccountId)` bytes.
		#[pallet::constant]
		type DepositBase: Get<BalanceOf<Self>>;

		/// The amount of currency needed per unit threshold when creating a multisig execution.
		///
		/// This is held for adding 32 bytes more into a pre-existing storage value.
		#[pallet::constant]
		type DepositFactor: Get<BalanceOf<Self>>;

		/// The maximum amount of signatories allowed in the multisig.
		#[pallet::constant]
		type MaxSignatories: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: weights::WeightInfo;

		/// Query the current block number.
		///
		/// Must return monotonically increasing values when called from consecutive blocks.
		/// Can be configured to return either:
		/// - the local block number of the runtime via `frame_system::Pallet`
		/// - a remote block number, eg from the relay chain through `RelaychainDataProvider`
		/// - an arbitrary value through a custom implementation of the trait
		///
		/// There is currently no migration provided to "hot-swap" block number providers and it may
		/// result in undefined behavior when doing so. Parachains are therefore best off setting
		/// this to their local block number provider if they have the pallet already deployed.
		///
		/// Suggested values:
		/// - Solo- and Relay-chains: `frame_system::Pallet`
		/// - Parachains that may produce blocks sparingly or only when needed (on-demand):
		///   - already have the pallet deployed: `frame_system::Pallet`
		///   - are freshly deploying this pallet: `RelaychainDataProvider`
		/// - Parachains with a reliably block production rate (PLO or bulk-coretime):
		///   - already have the pallet deployed: `frame_system::Pallet`
		///   - are freshly deploying this pallet: no strong recommendation. Both local and remote
		///     providers can be used. Relay provider can be a bit better in cases where the
		///     parachain is lagging its block production to avoid clock skew.
		type BlockNumberProvider: BlockNumberProvider;
	}

	/// The in-code storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// The set of open multisig operations.
	#[pallet::storage]
	pub type Multisigs<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Blake2_128Concat,
		[u8; 32],
		Multisig<BlockNumberFor<T>, BalanceOf<T>, T::AccountId, T::MaxSignatories>,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Threshold must be 2 or greater.
		MinimumThreshold,
		/// Call is already approved by this signatory.
		AlreadyApproved,
		/// Call doesn't need any (more) approvals.
		NoApprovalsNeeded,
		/// There are too few signatories in the list.
		TooFewSignatories,
		/// There are too many signatories in the list.
		TooManySignatories,
		/// The signatories were provided out of order; they should be ordered.
		SignatoriesOutOfOrder,
		/// The sender was contained in the other signatories; it shouldn't be.
		SenderInSignatories,
		/// Multisig operation not found in storage.
		NotFound,
		/// Only the account that originally created the multisig is able to cancel it or update
		/// its deposits.
		NotOwner,
		/// No timepoint was given, yet the multisig operation is already underway.
		NoTimepoint,
		/// A different timepoint was given to the multisig operation that is underway.
		WrongTimepoint,
		/// A timepoint was given, yet no multisig operation is underway.
		UnexpectedTimepoint,
		/// The maximum weight information provided was too low.
		MaxWeightTooLow,
		/// The data to be stored is already stored.
		AlreadyStored,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new multisig operation has begun.
		NewMultisig { approving: T::AccountId, multisig: T::AccountId, call_hash: CallHash },
		/// A multisig operation has been approved by someone.
		MultisigApproval {
			approving: T::AccountId,
			timepoint: Timepoint<BlockNumberFor<T>>,
			multisig: T::AccountId,
			call_hash: CallHash,
		},
		/// A multisig operation has been executed.
		MultisigExecuted {
			approving: T::AccountId,
			timepoint: Timepoint<BlockNumberFor<T>>,
			multisig: T::AccountId,
			call_hash: CallHash,
			result: DispatchResult,
		},
		/// A multisig operation has been cancelled.
		MultisigCancelled {
			cancelling: T::AccountId,
			timepoint: Timepoint<BlockNumberFor<T>>,
			multisig: T::AccountId,
			call_hash: CallHash,
		},
		/// The deposit for a multisig operation has been updated/poked.
		DepositPoked {
			who: T::AccountId,
			call_hash: CallHash,
			old_deposit: BalanceOf<T>,
			new_deposit: BalanceOf<T>,
		},
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<frame_system::pallet_prelude::BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Immediately dispatch a multi-signature call using a single approval from the caller.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `other_signatories`: The accounts (other than the sender) who are part of the
		/// multi-signature, but do not participate in the approval process.
		/// - `call`: The call to be executed.
		///
		/// Result is equivalent to the dispatched result.
		///
		/// ## Complexity
		/// O(Z + C) where Z is the length of the call and C its execution weight.
		#[pallet::call_index(0)]
		#[pallet::weight({
			let dispatch_info = call.get_dispatch_info();
			(
				T::WeightInfo::as_multi_threshold_1(call.using_encoded(|c| c.len() as u32))
					// AccountData for inner call origin accountdata.
					.saturating_add(T::DbWeight::get().reads_writes(1, 1))
					.saturating_add(dispatch_info.call_weight),
				dispatch_info.class,
			)
		})]
		pub fn as_multi_threshold_1(
			origin: OriginFor<T>,
			other_signatories: Vec<T::AccountId>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let max_sigs = T::MaxSignatories::get() as usize;
			ensure!(!other_signatories.is_empty(), Error::<T>::TooFewSignatories);
			let other_signatories_len = other_signatories.len();
			ensure!(other_signatories_len < max_sigs, Error::<T>::TooManySignatories);
			let signatories = Self::ensure_sorted_and_insert(other_signatories, who.clone())?;

			let id = Self::multi_account_id(&signatories, 1);

			let (call_len, call_hash) = call.using_encoded(|c| (c.len(), blake2_256(&c)));
			let result = call.dispatch(RawOrigin::Signed(id.clone()).into());

			Self::deposit_event(Event::MultisigExecuted {
				approving: who,
				timepoint: Self::timepoint(),
				multisig: id,
				call_hash,
				result: result.map(|_| ()).map_err(|e| e.error),
			});

			result
				.map(|post_dispatch_info| {
					post_dispatch_info
						.actual_weight
						.map(|actual_weight| {
							T::WeightInfo::as_multi_threshold_1(call_len as u32)
								.saturating_add(actual_weight)
						})
						.into()
				})
				.map_err(|err| match err.post_info.actual_weight {
					Some(actual_weight) => {
						let weight_used = T::WeightInfo::as_multi_threshold_1(call_len as u32)
							.saturating_add(actual_weight);
						let post_info = Some(weight_used).into();
						DispatchErrorWithPostInfo { post_info, error: err.error }
					},
					None => err,
				})
		}

		/// Register approval for a dispatch to be made from a deterministic composite account if
		/// approved by a total of `threshold - 1` of `other_signatories`.
		///
		/// If there are enough, then dispatch the call.
		///
		/// Payment: `DepositBase` will be reserved if this is the first approval, plus
		/// `threshold` times `DepositFactor`. It is returned once this dispatch happens or
		/// is cancelled.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `threshold`: The total number of approvals for this dispatch before it is executed.
		/// - `other_signatories`: The accounts (other than the sender) who can approve this
		/// dispatch. May not be empty.
		/// - `maybe_timepoint`: If this is the first approval, then this must be `None`. If it is
		/// not the first approval, then it must be `Some`, with the timepoint (block number and
		/// transaction index) of the first approval transaction.
		/// - `call`: The call to be executed.
		///
		/// NOTE: Unless this is the final approval, you will generally want to use
		/// `approve_as_multi` instead, since it only requires a hash of the call.
		///
		/// Result is equivalent to the dispatched result if `threshold` is exactly `1`. Otherwise
		/// on success, result is `Ok` and the result from the interior call, if it was executed,
		/// may be found in the deposited `MultisigExecuted` event.
		///
		/// ## Complexity
		/// - `O(S + Z + Call)`.
		/// - Up to one balance-reserve or unreserve operation.
		/// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
		///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
		/// - One call encode & hash, both of complexity `O(Z)` where `Z` is tx-len.
		/// - One encode & hash, both of complexity `O(S)`.
		/// - Up to one binary search and insert (`O(logS + S)`).
		/// - I/O: 1 read `O(S)`, up to 1 mutate `O(S)`. Up to one remove.
		/// - One event.
		/// - The weight of the `call`.
		/// - Storage: inserts one item, value size bounded by `MaxSignatories`, with a deposit
		///   taken for its lifetime of `DepositBase + threshold * DepositFactor`.
		#[pallet::call_index(1)]
		#[pallet::weight({
			let s = other_signatories.len() as u32;
			let z = call.using_encoded(|d| d.len()) as u32;

			T::WeightInfo::as_multi_create(s, z)
			.max(T::WeightInfo::as_multi_approve(s, z))
			.max(T::WeightInfo::as_multi_complete(s, z))
			.saturating_add(*max_weight)
		})]
		pub fn as_multi(
			origin: OriginFor<T>,
			threshold: u16,
			other_signatories: Vec<T::AccountId>,
			maybe_timepoint: Option<Timepoint<BlockNumberFor<T>>>,
			call: Box<<T as Config>::RuntimeCall>,
			max_weight: Weight,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::operate(
				who,
				threshold,
				other_signatories,
				maybe_timepoint,
				CallOrHash::Call(*call),
				max_weight,
			)
		}

		/// Register approval for a dispatch to be made from a deterministic composite account if
		/// approved by a total of `threshold - 1` of `other_signatories`.
		///
		/// Payment: `DepositBase` will be reserved if this is the first approval, plus
		/// `threshold` times `DepositFactor`. It is returned once this dispatch happens or
		/// is cancelled.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `threshold`: The total number of approvals for this dispatch before it is executed.
		/// - `other_signatories`: The accounts (other than the sender) who can approve this
		/// dispatch. May not be empty.
		/// - `maybe_timepoint`: If this is the first approval, then this must be `None`. If it is
		/// not the first approval, then it must be `Some`, with the timepoint (block number and
		/// transaction index) of the first approval transaction.
		/// - `call_hash`: The hash of the call to be executed.
		///
		/// NOTE: If this is the final approval, you will want to use `as_multi` instead.
		///
		/// ## Complexity
		/// - `O(S)`.
		/// - Up to one balance-reserve or unreserve operation.
		/// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
		///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
		/// - One encode & hash, both of complexity `O(S)`.
		/// - Up to one binary search and insert (`O(logS + S)`).
		/// - I/O: 1 read `O(S)`, up to 1 mutate `O(S)`. Up to one remove.
		/// - One event.
		/// - Storage: inserts one item, value size bounded by `MaxSignatories`, with a deposit
		///   taken for its lifetime of `DepositBase + threshold * DepositFactor`.
		#[pallet::call_index(2)]
		#[pallet::weight({
			let s = other_signatories.len() as u32;

			T::WeightInfo::approve_as_multi_create(s)
				.max(T::WeightInfo::approve_as_multi_approve(s))
				.saturating_add(*max_weight)
		})]
		pub fn approve_as_multi(
			origin: OriginFor<T>,
			threshold: u16,
			other_signatories: Vec<T::AccountId>,
			maybe_timepoint: Option<Timepoint<BlockNumberFor<T>>>,
			call_hash: [u8; 32],
			max_weight: Weight,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::operate(
				who,
				threshold,
				other_signatories,
				maybe_timepoint,
				CallOrHash::Hash(call_hash),
				max_weight,
			)
		}

		/// Cancel a pre-existing, on-going multisig transaction. Any deposit reserved previously
		/// for this operation will be unreserved on success.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `threshold`: The total number of approvals for this dispatch before it is executed.
		/// - `other_signatories`: The accounts (other than the sender) who can approve this
		/// dispatch. May not be empty.
		/// - `timepoint`: The timepoint (block number and transaction index) of the first approval
		/// transaction for this dispatch.
		/// - `call_hash`: The hash of the call to be executed.
		///
		/// ## Complexity
		/// - `O(S)`.
		/// - Up to one balance-reserve or unreserve operation.
		/// - One passthrough operation, one insert, both `O(S)` where `S` is the number of
		///   signatories. `S` is capped by `MaxSignatories`, with weight being proportional.
		/// - One encode & hash, both of complexity `O(S)`.
		/// - One event.
		/// - I/O: 1 read `O(S)`, one remove.
		/// - Storage: removes one item.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::cancel_as_multi(other_signatories.len() as u32))]
		pub fn cancel_as_multi(
			origin: OriginFor<T>,
			threshold: u16,
			other_signatories: Vec<T::AccountId>,
			timepoint: Timepoint<BlockNumberFor<T>>,
			call_hash: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(threshold >= 2, Error::<T>::MinimumThreshold);
			let max_sigs = T::MaxSignatories::get() as usize;
			ensure!(!other_signatories.is_empty(), Error::<T>::TooFewSignatories);
			ensure!(other_signatories.len() < max_sigs, Error::<T>::TooManySignatories);
			let signatories = Self::ensure_sorted_and_insert(other_signatories, who.clone())?;

			let id = Self::multi_account_id(&signatories, threshold);

			let m = <Multisigs<T>>::get(&id, call_hash).ok_or(Error::<T>::NotFound)?;
			ensure!(m.when == timepoint, Error::<T>::WrongTimepoint);
			ensure!(m.depositor == who, Error::<T>::NotOwner);

			let err_amount = T::Currency::unreserve(&m.depositor, m.deposit);
			debug_assert!(err_amount.is_zero());
			<Multisigs<T>>::remove(&id, &call_hash);

			Self::deposit_event(Event::MultisigCancelled {
				cancelling: who,
				timepoint,
				multisig: id,
				call_hash,
			});
			Ok(())
		}

		/// Poke the deposit reserved for an existing multisig operation.
		///
		/// The dispatch origin for this call must be _Signed_ and must be the original depositor of
		/// the multisig operation.
		///
		/// The transaction fee is waived if the deposit amount has changed.
		///
		/// - `threshold`: The total number of approvals needed for this multisig.
		/// - `other_signatories`: The accounts (other than the sender) who are part of the
		///   multisig.
		/// - `call_hash`: The hash of the call this deposit is reserved for.
		///
		/// Emits `DepositPoked` if successful.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::poke_deposit(other_signatories.len() as u32))]
		pub fn poke_deposit(
			origin: OriginFor<T>,
			threshold: u16,
			other_signatories: Vec<T::AccountId>,
			call_hash: [u8; 32],
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(threshold >= 2, Error::<T>::MinimumThreshold);
			let max_sigs = T::MaxSignatories::get() as usize;
			ensure!(!other_signatories.is_empty(), Error::<T>::TooFewSignatories);
			ensure!(other_signatories.len() < max_sigs, Error::<T>::TooManySignatories);
			// Get the multisig account ID
			let signatories = Self::ensure_sorted_and_insert(other_signatories, who.clone())?;
			let id = Self::multi_account_id(&signatories, threshold);

			Multisigs::<T>::try_mutate(
				&id,
				call_hash,
				|maybe_multisig| -> DispatchResultWithPostInfo {
					let mut multisig = maybe_multisig.take().ok_or(Error::<T>::NotFound)?;
					ensure!(multisig.depositor == who, Error::<T>::NotOwner);

					// Calculate the new deposit
					let new_deposit = Self::deposit(threshold);
					let old_deposit = multisig.deposit;

					if new_deposit == old_deposit {
						*maybe_multisig = Some(multisig);
						return Ok(Pays::Yes.into());
					}

					// Update the reserved amount
					if new_deposit > old_deposit {
						let extra = new_deposit.saturating_sub(old_deposit);
						T::Currency::reserve(&who, extra)?;
					} else {
						let excess = old_deposit.saturating_sub(new_deposit);
						let remaining_unreserved = T::Currency::unreserve(&who, excess);
						if !remaining_unreserved.is_zero() {
							defensive!(
								"Failed to unreserve for full amount for multisig. (Call Hash, Requested, Actual): ",
								(call_hash, excess, excess.saturating_sub(remaining_unreserved))
							);
						}
					}

					// Update storage
					multisig.deposit = new_deposit;
					*maybe_multisig = Some(multisig);

					// Emit event
					Self::deposit_event(Event::DepositPoked {
						who: who.clone(),
						call_hash,
						old_deposit,
						new_deposit,
					});

					Ok(Pays::No.into())
				},
			)
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Derive a multi-account ID from the sorted list of accounts and the threshold that are
	/// required.
	///
	/// NOTE: `who` must be sorted. If it is not, then you'll get the wrong answer.
	pub fn multi_account_id(who: &[T::AccountId], threshold: u16) -> T::AccountId {
		let entropy = (b"modlpy/utilisuba", who, threshold).using_encoded(blake2_256);
		Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
			.expect("infinite length input; no invalid inputs for type; qed")
	}

	fn operate(
		who: T::AccountId,
		threshold: u16,
		other_signatories: Vec<T::AccountId>,
		maybe_timepoint: Option<Timepoint<BlockNumberFor<T>>>,
		call_or_hash: CallOrHash<T>,
		max_weight: Weight,
	) -> DispatchResultWithPostInfo {
		ensure!(threshold >= 2, Error::<T>::MinimumThreshold);
		let max_sigs = T::MaxSignatories::get() as usize;
		ensure!(!other_signatories.is_empty(), Error::<T>::TooFewSignatories);
		let other_signatories_len = other_signatories.len();
		ensure!(other_signatories_len < max_sigs, Error::<T>::TooManySignatories);
		let signatories = Self::ensure_sorted_and_insert(other_signatories, who.clone())?;

		let id = Self::multi_account_id(&signatories, threshold);

		// Threshold > 1; this means it's a multi-step operation. We extract the `call_hash`.
		let (call_hash, call_len, maybe_call) = match call_or_hash {
			CallOrHash::Call(call) => {
				let (call_hash, call_len) = call.using_encoded(|d| (blake2_256(d), d.len()));
				(call_hash, call_len, Some(call))
			},
			CallOrHash::Hash(h) => (h, 0, None),
		};

		// Branch on whether the operation has already started or not.
		if let Some(mut m) = <Multisigs<T>>::get(&id, call_hash) {
			// Yes; ensure that the timepoint exists and agrees.
			let timepoint = maybe_timepoint.ok_or(Error::<T>::NoTimepoint)?;
			ensure!(m.when == timepoint, Error::<T>::WrongTimepoint);

			// Ensure that either we have not yet signed or that it is at threshold.
			let mut approvals = m.approvals.len() as u16;
			// We only bother with the approval if we're below threshold.
			let maybe_pos = m.approvals.binary_search(&who).err().filter(|_| approvals < threshold);
			// Bump approvals if not yet voted and the vote is needed.
			if maybe_pos.is_some() {
				approvals += 1;
			}

			// We only bother fetching/decoding call if we know that we're ready to execute.
			if let Some(call) = maybe_call.filter(|_| approvals >= threshold) {
				// verify weight
				ensure!(
					call.get_dispatch_info().call_weight.all_lte(max_weight),
					Error::<T>::MaxWeightTooLow
				);

				// Clean up storage before executing call to avoid an possibility of reentrancy
				// attack.
				<Multisigs<T>>::remove(&id, call_hash);
				T::Currency::unreserve(&m.depositor, m.deposit);

				let result = call.dispatch(RawOrigin::Signed(id.clone()).into());
				Self::deposit_event(Event::MultisigExecuted {
					approving: who,
					timepoint,
					multisig: id,
					call_hash,
					result: result.map(|_| ()).map_err(|e| e.error),
				});
				Ok(get_result_weight(result)
					.map(|actual_weight| {
						T::WeightInfo::as_multi_complete(
							other_signatories_len as u32,
							call_len as u32,
						)
						.saturating_add(actual_weight)
					})
					.into())
			} else {
				// We cannot dispatch the call now; either it isn't available, or it is, but we
				// don't have threshold approvals even with our signature.

				if let Some(pos) = maybe_pos {
					// Record approval.
					m.approvals
						.try_insert(pos, who.clone())
						.map_err(|_| Error::<T>::TooManySignatories)?;
					<Multisigs<T>>::insert(&id, call_hash, m);
					Self::deposit_event(Event::MultisigApproval {
						approving: who,
						timepoint,
						multisig: id,
						call_hash,
					});
				} else {
					// If we already approved and didn't store the Call, then this was useless and
					// we report an error.
					Err(Error::<T>::AlreadyApproved)?
				}

				let final_weight =
					T::WeightInfo::as_multi_approve(other_signatories_len as u32, call_len as u32);
				// Call is not made, so the actual weight does not include call
				Ok(Some(final_weight).into())
			}
		} else {
			// Not yet started; there should be no timepoint given.
			ensure!(maybe_timepoint.is_none(), Error::<T>::UnexpectedTimepoint);

			// Just start the operation by recording it in storage.
			let deposit = Self::deposit(threshold);

			T::Currency::reserve(&who, deposit)?;

			let initial_approvals =
				vec![who.clone()].try_into().map_err(|_| Error::<T>::TooManySignatories)?;

			<Multisigs<T>>::insert(
				&id,
				call_hash,
				Multisig {
					when: Self::timepoint(),
					deposit,
					depositor: who.clone(),
					approvals: initial_approvals,
				},
			);
			Self::deposit_event(Event::NewMultisig { approving: who, multisig: id, call_hash });

			let final_weight =
				T::WeightInfo::as_multi_create(other_signatories_len as u32, call_len as u32);
			// Call is not made, so the actual weight does not include call
			Ok(Some(final_weight).into())
		}
	}

	/// The current `Timepoint`.
	pub fn timepoint() -> Timepoint<BlockNumberFor<T>> {
		Timepoint {
			height: T::BlockNumberProvider::current_block_number(),
			index: <frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
		}
	}

	/// Check that signatories is sorted and doesn't contain sender, then insert sender.
	fn ensure_sorted_and_insert(
		other_signatories: Vec<T::AccountId>,
		who: T::AccountId,
	) -> Result<Vec<T::AccountId>, DispatchError> {
		let mut signatories = other_signatories;
		let mut maybe_last = None;
		let mut index = 0;
		for item in signatories.iter() {
			if let Some(last) = maybe_last {
				ensure!(last < item, Error::<T>::SignatoriesOutOfOrder);
			}
			if item <= &who {
				ensure!(item != &who, Error::<T>::SenderInSignatories);
				index += 1;
			}
			maybe_last = Some(item);
		}
		signatories.insert(index, who);
		Ok(signatories)
	}

	/// Calculate the deposit for a multisig operation.
	///
	/// The deposit is calculated as `DepositBase + DepositFactor * threshold`.
	pub fn deposit(threshold: u16) -> BalanceOf<T> {
		T::DepositBase::get() + T::DepositFactor::get() * threshold.into()
	}
}

/// Return the weight of a dispatch call result as an `Option`.
///
/// Will return the weight regardless of what the state of the result is.
fn get_result_weight(result: DispatchResultWithPostInfo) -> Option<Weight> {
	match result {
		Ok(post_info) => post_info.actual_weight,
		Err(err) => err.post_info.actual_weight,
	}
}
