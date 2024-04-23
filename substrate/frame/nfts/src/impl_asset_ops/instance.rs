use crate::{types::asset_strategies::*, *};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		tokens::asset_ops::{common_asset_kinds::Instance, common_strategies::*, *},
		EnsureOrigin,
	},
};
use frame_system::ensure_signed;
use sp_runtime::DispatchError;

impl<T: Config<I>, I: 'static> AssetDefinition<Instance> for Pallet<T, I> {
	type Id = (T::CollectionId, T::ItemId);
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
		Self::do_update_item_metadata(
			None,
			*collection,
			*item,
			update.map(|data| Self::construct_metadata(data.to_vec())).transpose()?,
		)
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Instance, WithOrigin<T::RuntimeOrigin, Bytes>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, Bytes>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let WithOrigin(origin, _bytes) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		Self::do_update_item_metadata(
			maybe_check_origin,
			*collection,
			*item,
			update.map(|data| Self::construct_metadata(data.to_vec())).transpose()?,
		)
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

		Attribute::<T, I>::get((
			collection,
			Some(item),
			namespace,
			Self::construct_attribute_key(attribute.to_vec())?,
		))
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
		let namespace = AttributeNamespace::CollectionOwner;

		let Bytes(RegularAttribute(attribute)) = bytes;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;

		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Instance, WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute<'a>>>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let namespace = AttributeNamespace::CollectionOwner;

		let WithOrigin(origin, Bytes(RegularAttribute(attribute))) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(
			maybe_check_origin,
			*collection,
			Some(*item),
			namespace,
			attribute,
			update,
		)
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

		Attribute::<T, I>::get((
			collection,
			Some(item),
			namespace,
			Self::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Instance, Bytes<CustomAttribute<'a, T::AccountId>>> for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<CustomAttribute<'a, T::AccountId>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let Bytes(CustomAttribute(account, attribute)) = bytes;

		let namespace = Account::<T, I>::get((account, collection, item))
			.map(|_| AttributeNamespace::ItemOwner)
			.unwrap_or_else(|| AttributeNamespace::Account(account.clone()));

		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Instance, WithOrigin<T::RuntimeOrigin, Bytes<CustomAttribute<'a, T::AccountId>>>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, Bytes<CustomAttribute<'a, T::AccountId>>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let WithOrigin(origin, Bytes(CustomAttribute(account, attribute))) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		let namespace = Account::<T, I>::get((account, collection, item))
			.map(|_| AttributeNamespace::ItemOwner)
			.unwrap_or_else(|| AttributeNamespace::Account(account.clone()));

		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(
			maybe_check_origin,
			*collection,
			Some(*item),
			namespace,
			attribute,
			update,
		)
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

		Attribute::<T, I>::get((
			collection,
			Some(item),
			namespace,
			Self::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Instance, Bytes<SystemAttribute<'a>>>
	for Pallet<T, I>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<SystemAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;

		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
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

		Self::update_metadata(
			id,
			Bytes(RegularAttribute(
				&PalletAttributes::<T::CollectionId>::TransferDisabled.encode(),
			)),
			update.then_some(&[]),
		)
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<Instance, Owned<'a, PredefinedId<'a, (T::CollectionId, T::ItemId)>, T::AccountId>>
	for Pallet<T, I>
{
	fn create(strategy: Owned<PredefinedId<Self::Id>, T::AccountId>) -> DispatchResult {
		let Owned { id_assignment: PredefinedId((collection, item)), owner: mint_to, .. } =
			strategy;

		let item_config = ItemConfig { settings: Self::get_default_item_settings(collection)? };

		Self::do_mint(*collection, *item, None, mint_to.clone(), item_config, |_, _| Ok(()))
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<
		Instance,
		Owned<'a, PredefinedId<'a, (T::CollectionId, T::ItemId)>, T::AccountId, ItemConfig>,
	> for Pallet<T, I>
{
	fn create(strategy: Owned<PredefinedId<Self::Id>, T::AccountId, ItemConfig>) -> DispatchResult {
		let Owned {
			id_assignment: PredefinedId((collection, item)),
			owner: mint_to,
			config: item_config,
			..
		} = strategy;

		Self::do_mint(*collection, *item, None, mint_to.clone(), *item_config, |_, _| Ok(()))
	}
}

impl<'a, T: Config<I>, I: 'static> Transfer<Instance, FromTo<'a, T::AccountId>> for Pallet<T, I> {
	fn transfer(
		(collection, item): &Self::Id,
		FromTo(from, to): FromTo<T::AccountId>,
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
		ForceTo(to): ForceTo<T::AccountId>,
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

impl<T: Config<I>, I: 'static> Destroy<Instance, WithOrigin<T::RuntimeOrigin, ForceDestroy>>
	for Pallet<T, I>
{
	fn destroy(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, ForceDestroy>,
	) -> DispatchResult {
		let WithOrigin(origin, _force_destroy) = strategy;

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
