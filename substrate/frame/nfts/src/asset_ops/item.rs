use core::marker::PhantomData;

use crate::{types::asset_strategies::*, *, Item as ItemStorage};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		tokens::asset_ops::{common_strategies::*, *},
		EnsureOrigin,
	},
};
use frame_system::ensure_signed;
use sp_runtime::DispatchError;

pub struct Item<PalletInstance>(PhantomData<PalletInstance>);

impl<T: Config<I>, I: 'static> AssetDefinition for Item<Pallet<T, I>> {
	type Id = (T::CollectionId, T::ItemId);
}

impl<T: Config<I>, I: 'static> InspectMetadata<Ownership<T::AccountId>> for Item<Pallet<T, I>> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_ownership: Ownership<T::AccountId>,
	) -> Result<T::AccountId, DispatchError> {
		ItemStorage::<T, I>::get(collection, item)
			.map(|a| a.owner)
			.ok_or(Error::<T, I>::UnknownItem.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Bytes> for Item<Pallet<T, I>> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_bytes: Bytes,
	) -> Result<Vec<u8>, DispatchError> {
		ItemMetadataOf::<T, I>::get(collection, item)
			.map(|m| m.data.into())
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Bytes> for Item<Pallet<T, I>> {
	fn update_metadata(
		(collection, item): &Self::Id,
		_bytes: Bytes,
		update: Option<&[u8]>,
	) -> DispatchResult {
		<Pallet<T, I>>::do_update_item_metadata(
			None,
			*collection,
			*item,
			update.map(|data| <Pallet<T, I>>::construct_metadata(data.to_vec())).transpose()?,
		)
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<WithOrigin<T::RuntimeOrigin, Bytes>>
	for Item<Pallet<T, I>>
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

		<Pallet<T, I>>::do_update_item_metadata(
			maybe_check_origin,
			*collection,
			*item,
			update.map(|data| <Pallet<T, I>>::construct_metadata(data.to_vec())).transpose()?,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Bytes<RegularAttribute<'a>>>
	for Item<Pallet<T, I>>
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
			<Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Bytes<RegularAttribute<'a>>>
	for Item<Pallet<T, I>>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<RegularAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let namespace = AttributeNamespace::CollectionOwner;

		let Bytes(RegularAttribute(attribute)) = bytes;
		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;

		let update =
			update.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec())).transpose()?;

		<Pallet<T, I>>::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute<'a>>>>
	for Item<Pallet<T, I>>
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
		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec())).transpose()?;

		<Pallet<T, I>>::do_update_attribute(
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
	InspectMetadata<Bytes<CustomAttribute<'a, T::AccountId>>> for Item<Pallet<T, I>>
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
			<Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Bytes<CustomAttribute<'a, T::AccountId>>> for Item<Pallet<T, I>>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<CustomAttribute<T::AccountId>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let Bytes(CustomAttribute(account, attribute)) = bytes;

		let namespace = Account::<T, I>::get((account, collection, item))
			.map(|_| AttributeNamespace::ItemOwner)
			.unwrap_or_else(|| AttributeNamespace::Account(account.clone()));

		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec())).transpose()?;

		<Pallet<T, I>>::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<WithOrigin<T::RuntimeOrigin, Bytes<CustomAttribute<'a, T::AccountId>>>>
	for Item<Pallet<T, I>>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, Bytes<CustomAttribute<T::AccountId>>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let WithOrigin(origin, Bytes(CustomAttribute(account, attribute))) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		let namespace = Account::<T, I>::get((account, collection, item))
			.map(|_| AttributeNamespace::ItemOwner)
			.unwrap_or_else(|| AttributeNamespace::Account(account.clone()));

		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec())).transpose()?;

		<Pallet<T, I>>::do_update_attribute(
			maybe_check_origin,
			*collection,
			Some(*item),
			namespace,
			attribute,
			update,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Bytes<SystemAttribute<'a>>>
	for Item<Pallet<T, I>>
{
	fn inspect_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<SystemAttribute>,
	) -> Result<Vec<u8>, DispatchError> {
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;

		Attribute::<T, I>::get((
			collection,
			Some(item),
			namespace,
			<Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Bytes<SystemAttribute<'a>>>
	for Item<Pallet<T, I>>
{
	fn update_metadata(
		(collection, item): &Self::Id,
		bytes: Bytes<SystemAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;

		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec())).transpose()?;

		<Pallet<T, I>>::do_update_attribute(None, *collection, Some(*item), namespace, attribute, update)
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<CanTransfer> for Item<Pallet<T, I>> {
	fn inspect_metadata(
		(collection, item): &Self::Id,
		_can_transfer: CanTransfer,
	) -> Result<bool, DispatchError> {
		use PalletAttributes::TransferDisabled;
		match <Pallet<T, I>>::has_system_attribute(collection, item, TransferDisabled) {
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

impl<T: Config<I>, I: 'static> UpdateMetadata<CanTransfer> for Item<Pallet<T, I>> {
	fn update_metadata(
		id @ (collection, item): &Self::Id,
		_can_transfer: CanTransfer,
		update: bool,
	) -> DispatchResult {
		if update {
			let transfer_disabled =
				<Pallet<T, I>>::has_system_attribute(collection, &item, PalletAttributes::TransferDisabled)?;

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

impl<T: Config<I>, I: 'static>
	Create<Owned<T::AccountId, PredefinedId<(T::CollectionId, T::ItemId)>>> for Item<Pallet<T, I>>
{
	fn create(
		strategy: Owned<T::AccountId, PredefinedId<(T::CollectionId, T::ItemId)>>,
	) -> Result<(T::CollectionId, T::ItemId), DispatchError> {
		let Owned { owner: mint_to, id_assignment, .. } = strategy;
		let (collection, item) = id_assignment.params;

		let item_config = ItemConfig { settings: <Pallet<T, I>>::get_default_item_settings(&collection)? };

		<Pallet<T, I>>::do_mint(collection.clone(), item.clone(), None, mint_to, item_config, |_, _| Ok(()))?;

		Ok((collection, item))
	}
}

impl<T: Config<I>, I: 'static>
	Create<Owned<T::AccountId, PredefinedId<(T::CollectionId, T::ItemId)>, ItemConfig>>
	for Item<Pallet<T, I>>
{
	fn create(
		strategy: Owned<T::AccountId, PredefinedId<(T::CollectionId, T::ItemId)>, ItemConfig>,
	) -> Result<(T::CollectionId, T::ItemId), DispatchError> {
		let Owned { owner: mint_to, id_assignment, config: item_config, .. } = strategy;
		let (collection, item) = id_assignment.params;

		<Pallet<T, I>>::do_mint(collection.clone(), item.clone(), None, mint_to, item_config, |_, _| Ok(()))?;

		Ok((collection, item))
	}
}

impl<T: Config<I>, I: 'static> Transfer<JustDo<T::AccountId>> for Item<Pallet<T, I>> {
	fn transfer((collection, item): &Self::Id, strategy: JustDo<T::AccountId>) -> DispatchResult {
		let JustDo(to) = strategy;

		<Pallet<T, I>>::do_transfer(*collection, *item, to, |_, _| Ok(()))
	}
}

impl<T: Config<I>, I: 'static>
	Transfer<WithOrigin<T::RuntimeOrigin, JustDo<T::AccountId>>> for Item<Pallet<T, I>>
{
	fn transfer(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, JustDo<T::AccountId>>,
	) -> DispatchResult {
		let WithOrigin(origin, JustDo(to)) = strategy;

		let signer = ensure_signed(origin)?;

		<Pallet<T, I>>::do_transfer(*collection, *item, to, |_, details| {
			if details.owner != signer {
				let deadline = details.approvals.get(&signer).ok_or(Error::<T, I>::NoPermission)?;
				if let Some(d) = deadline {
					let block_number = frame_system::Pallet::<T>::block_number();
					ensure!(block_number <= *d, Error::<T, I>::ApprovalExpired);
				}
			}
			Ok(())
		})
	}
}

impl<T: Config<I>, I: 'static> Transfer<FromTo<T::AccountId>> for Item<Pallet<T, I>> {
	fn transfer(
		(collection, item): &Self::Id,
		FromTo(from, to): FromTo<T::AccountId>,
	) -> DispatchResult {
		<Pallet<T, I>>::do_transfer(*collection, *item, to, |_, details| {
			ensure!(details.owner == from, Error::<T, I>::NoPermission);
			Ok(())
		})
	}
}

impl<T: Config<I>, I: 'static> Destroy<JustDo> for Item<Pallet<T, I>> {
	fn destroy((collection, item): &Self::Id, _force_destroy: JustDo) -> DispatchResult {
		<Pallet<T, I>>::do_burn(*collection, *item, |_details| Ok(()))
	}
}

impl<T: Config<I>, I: 'static> Destroy<IfOwnedBy<T::AccountId>> for Item<Pallet<T, I>> {
	fn destroy((collection, item): &Self::Id, strategy: IfOwnedBy<T::AccountId>) -> DispatchResult {
		let IfOwnedBy(account) = strategy;

		<Pallet<T, I>>::do_burn(*collection, *item, |details| {
			ensure!(details.owner == account, Error::<T, I>::NoPermission);

			Ok(())
		})
	}
}

impl<T: Config<I>, I: 'static> Destroy<WithOrigin<T::RuntimeOrigin, JustDo>>
	for Item<Pallet<T, I>>
{
	fn destroy(
		(collection, item): &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, JustDo>,
	) -> DispatchResult {
		let WithOrigin(origin, _force_destroy) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		<Pallet<T, I>>::do_burn(*collection, *item, |details| {
			if let Some(check_origin) = maybe_check_origin {
				ensure!(details.owner == check_origin, Error::<T, I>::NoPermission);
			}

			Ok(())
		})
	}
}