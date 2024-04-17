use super::*;
use crate::types::unique_assets_strategies::*;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		tokens::{
			common_asset_strategies::{CheckOrigin, ForceTo, FromTo, NewOwnedChildAssetWithId},
			unique_assets::{
				common_asset_kinds::{Class, Instance},
				Create, Identification, Transfer,
			},
		},
		EnsureOrigin,
	},
};
use sp_core::Get;
use sp_runtime::DispatchError;

impl<T: Config<I>, I: 'static> Identification<Class> for Pallet<T, I> {
	type Id = T::CollectionId;
}

impl<T: Config<I>, I: 'static> Identification<Instance> for Pallet<T, I> {
	type Id = (T::CollectionId, T::ItemId);
}

impl<'a, T: Config<I>, I: 'static> Create<ConfiguredCollection<'a, T, I>> for Pallet<T, I> {
	fn create(strategy: ConfiguredCollection<'a, T, I>) -> Result<T::CollectionId, DispatchError> {
		let ConfiguredCollection { owner, admin, config } = strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		Self::do_create_collection(
			collection,
			owner.clone(),
			admin.clone(),
			*config,
			T::CollectionDeposit::get(),
			Event::Created { collection, creator: owner.clone(), owner: admin.clone() },
		)?;

		Self::set_next_collection_id(collection);

		Ok(collection)
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<CheckOrigin<T::RuntimeOrigin, ConfiguredCollection<'a, T, I>>> for Pallet<T, I>
{
	fn create(
		strategy: CheckOrigin<T::RuntimeOrigin, ConfiguredCollection<'a, T, I>>,
	) -> Result<T::CollectionId, DispatchError> {
		let CheckOrigin(origin, ConfiguredCollection { owner, admin, config }) = strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		let maybe_check_signer =
			T::ForceOrigin::try_origin(origin).map(|_| None).or_else(|origin| {
				T::CreateOrigin::ensure_origin(origin, &collection)
					.map(Some)
					.map_err(DispatchError::from)
			})?;

		if let Some(signer) = maybe_check_signer {
			ensure!(signer == *owner, <Error<T, I>>::NoPermission);

			// DepositRequired can be disabled by calling the with `ForceOrigin` only
			ensure!(
				!config.has_disabled_setting(CollectionSetting::DepositRequired),
				Error::<T, I>::WrongSetting
			);
		}

		Self::do_create_collection(
			collection,
			owner.clone(),
			admin.clone(),
			*config,
			T::CollectionDeposit::get(),
			Event::Created { collection, creator: owner.clone(), owner: admin.clone() },
		)?;

		Self::set_next_collection_id(collection);

		Ok(collection)
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<NewOwnedChildAssetWithId<'a, Instance, T::CollectionId, T::ItemId, T::AccountId>>
	for Pallet<T, I>
{
	fn create(
		strategy: NewOwnedChildAssetWithId<'a, Instance, T::CollectionId, T::ItemId, T::AccountId>,
	) -> DispatchResult {
		let NewOwnedChildAssetWithId {
			parent_asset_id: collection, id: item, owner: mint_to, ..
		} = strategy;

		let item_config = ItemConfig { settings: Self::get_default_item_settings(collection)? };

		Self::do_mint(*collection, *item, None, mint_to.clone(), item_config, |_, _| Ok(()))
	}
}

impl<'a, T: Config<I>, I: 'static> Transfer<Instance, FromTo<'a, T::AccountId>> for Pallet<T, I> {
	fn transfer(
		(collection, item): &Self::Id,
		FromTo(from, to): FromTo<'_, T::AccountId>,
	) -> DispatchResult {
		Self::do_transfer(*collection, *item, to.clone(), |_, details| {
			if details.owner != *from {
				let deadline = details.approvals.get(from).ok_or(Error::<T, I>::NoPermission)?;
				if let Some(d) = deadline {
					let block_number = frame_system::Pallet::<T>::block_number();
					ensure!(block_number <= *d, Error::<T, I>::ApprovalExpired);
				}
			}
			Ok(())
		})
	}
}

impl<'a, T: Config<I>, I: 'static> Transfer<Instance, ForceTo<'a, T::AccountId>> for Pallet<T, I> {
	fn transfer(
		(collection, item): &Self::Id,
		ForceTo(to): ForceTo<'_, T::AccountId>,
	) -> DispatchResult {
		Self::do_transfer(*collection, *item, to.clone(), |_, _| Ok(()))
	}
}
