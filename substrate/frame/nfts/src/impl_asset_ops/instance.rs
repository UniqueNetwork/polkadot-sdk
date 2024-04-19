use crate::{types::asset_strategies::*, *};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		asset_ops::{common_asset_kinds::Instance, common_strategies::*, *},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_runtime::DispatchError;

impl<T: Config<I>, I: 'static> AssetDefinition<Instance> for Pallet<T, I> {
	type Id = (T::CollectionId, T::ItemId);
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

impl<T: Config<I>, I: 'static> Destroy<Instance, ForceDestroy> for Pallet<T, I> {
	fn destroy((collection, item): &Self::Id, _force_destroy: ForceDestroy) -> DispatchResult {
		Self::do_burn(*collection, *item, |_details| Ok(()))
	}
}

impl<'a, T: Config<I>, I: 'static> Destroy<Instance, IfOwnedBy<'a, T::AccountId>> for Pallet<T, I> {
	fn destroy(
		(collection, item): &Self::Id,
		strategy: IfOwnedBy<'a, T::AccountId>,
	) -> DispatchResult {
		let IfOwnedBy(account) = strategy;

		Self::do_burn(*collection, *item, |details| {
			ensure!(details.owner == *account, Error::<T, I>::NoPermission);

			Ok(())
		})
	}
}

impl<T: Config<I>, I: 'static> Destroy<Instance, CheckOrigin<T::RuntimeOrigin, ForceDestroy>>
	for Pallet<T, I>
{
	fn destroy(
		(collection, item): &Self::Id,
		strategy: CheckOrigin<T::RuntimeOrigin, ForceDestroy>,
	) -> DispatchResult {
		let CheckOrigin(origin, _force_destroy) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		Self::do_burn(*collection, *item, |details| {
			if let Some(check_origin) = maybe_check_origin {
				ensure!(details.owner == check_origin, Error::<T, I>::NoPermission);
			}

			Ok(())
		})
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Instance, Ownership<T::AccountId>> for Pallet<T, I> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_ownership: Ownership<T::AccountId>,
	) -> Result<T::AccountId, DispatchError> {
		Item::<T, I>::get(collection, item)
			.map(|a| a.owner)
			.ok_or(Error::<T, I>::UnknownItem.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Instance, Bytes> for Pallet<T, I> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_bytes: Bytes,
	) -> Result<Vec<u8>, DispatchError> {
		ItemMetadataOf::<T, I>::get(collection, item)
			.map(|m| m.data.into())
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Instance, Bytes> for Pallet<T, I> {
	fn update_metadata(
		(collection, item): &Self::Id,
		_bytes: Bytes,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_check_origin = None;

		match update {
			Some(data) => Self::do_set_item_metadata(
				maybe_check_origin,
				*collection,
				*item,
				Self::construct_metadata(data.to_vec())?,
				None,
			),
			None => Self::do_clear_item_metadata(maybe_check_origin, *collection, *item),
		}
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Instance, CheckOrigin<T::RuntimeOrigin, Bytes>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: CheckOrigin<T::RuntimeOrigin, Bytes>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let CheckOrigin(origin, _bytes) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		match update {
			Some(data) => Self::do_set_item_metadata(
				maybe_check_origin,
				*collection,
				*item,
				Self::construct_metadata(data.to_vec())?,
				None,
			),
			None => Self::do_clear_item_metadata(maybe_check_origin, *collection, *item),
		}
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Instance, Bytes<RegularAttribute<'a>>>
	for Pallet<T, I>
{
	fn inspect_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<RegularAttribute>,
	) -> Result<Vec<u8>, DispatchError> {
		let namespace = AttributeNamespace::CollectionOwner;

		let Bytes(RegularAttribute(attribute)) = bytes;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0.into())
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Instance, Bytes<RegularAttribute<'a>>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<RegularAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_check_origin = None;

		let Bytes(RegularAttribute(attribute)) = bytes;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;

		let update = update
			.map(|value| Self::construct_attribute_value(value.to_vec()))
			.transpose()?;

		match update {
			Some(value) => Self::do_force_set_attribute(
				None,
				*collection,
				Some(*item),
				AttributeNamespace::Pallet,
				attribute,
				value,
			),
			None => Self::do_clear_attribute(
				maybe_check_origin,
				*collection,
				Some(*item),
				AttributeNamespace::Pallet,
				attribute,
			),
		}
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Instance, CheckOrigin<T::RuntimeOrigin, Bytes<RegularAttribute<'a>>>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: CheckOrigin<T::RuntimeOrigin, Bytes<RegularAttribute>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let CheckOrigin(origin, Bytes(RegularAttribute(attribute))) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update = update
			.map(|value| Self::construct_attribute_value(value.to_vec()))
			.transpose()?;

		match (maybe_check_origin, update) {
			(Some(who), Some(value)) => {
				let collection_owner =
					Self::collection_owner(*collection).ok_or(Error::<T, I>::UnknownCollection)?;

				Self::do_set_attribute(
					who,
					*collection,
					Some(*item),
					AttributeNamespace::CollectionOwner,
					attribute,
					value,
					collection_owner,
				)
			},
			(None, Some(value)) => Self::do_force_set_attribute(
				None,
				*collection,
				Some(*item),
				AttributeNamespace::Pallet,
				attribute,
				value,
			),
			(maybe_check_origin, None) => Self::do_clear_attribute(
				maybe_check_origin,
				*collection,
				Some(*item),
				AttributeNamespace::Pallet,
				attribute,
			),
		}
	}
}

impl<'a, T: Config<I>, I: 'static>
	InspectMetadata<Instance, Bytes<CustomAttribute<'a, T::AccountId>>> for Pallet<T, I>
{
	fn inspect_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<CustomAttribute<T::AccountId>>,
	) -> Result<Vec<u8>, DispatchError> {
		let Bytes(CustomAttribute(account, attribute)) = bytes;

		let namespace = Account::<T, I>::get((account, collection, item))
			.map(|_| AttributeNamespace::ItemOwner)
			.unwrap_or_else(|| AttributeNamespace::Account(account.clone()));

		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0.into())
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Instance, Bytes<SystemAttribute<'a>>>
	for Pallet<T, I>
{
	fn inspect_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<SystemAttribute<'a>>,
	) -> Result<Vec<u8>, DispatchError> {
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0.into())
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Instance, CanTransfer> for Pallet<T, I> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_can_transfer: CanTransfer,
	) -> Result<bool, DispatchError> {
		use PalletAttributes::TransferDisabled;
		match Self::has_system_attribute(collection, item, TransferDisabled) {
			Ok(transfer_disabled) if transfer_disabled => return Ok(false),
			_ => (),
		}
		match (
			CollectionConfigOf::<T, I>::get(collection),
			ItemConfigOf::<T, I>::get(collection, item),
		) {
			(Some(cc), Some(ic))
				if cc.is_setting_enabled(CollectionSetting::TransferableItems) &&
					ic.is_setting_enabled(ItemSetting::Transferable) =>
				Ok(true),
			_ => Ok(false),
		}
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Instance, CanTransfer> for Pallet<T, I> {
	fn update_metadata(
		id @ (collection, item): &Self::Id,
		_can_transfer: CanTransfer,
		update: bool,
	) -> DispatchResult {
		if update {
			let transfer_disabled =
				Self::has_system_attribute(collection, &item, PalletAttributes::TransferDisabled)?;

			// Can't lock the item twice
			if transfer_disabled {
				return Err(Error::<T, I>::ItemLocked.into())
			}
		}

		<Self as UpdateMetadata<Instance, _>>::update_metadata(
			id,
			Bytes(RegularAttribute(
				&PalletAttributes::<T::CollectionId>::TransferDisabled.encode(),
			)),
			update.then_some(&[]),
		)
	}
}
