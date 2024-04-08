use super::*;

use crate::metadata_strategy::{self as strategy, *};
use frame_support::{
	dispatch::DispatchResult,
	traits::{
		tokens::asset_metadata::{InspectMetadata, MetadataDefinition, UpdateMetadata},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_runtime::{BoundedVec, DispatchError};

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Collection<GenericMetadata>>
	for Pallet<T, I>
{
	type Key<'a> = &'a T::CollectionId;
	type Value = BoundedVec<u8, T::StringLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Collection<GenericMetadata>>
	for Pallet<T, I>
{
	fn asset_metadata(collection: Self::Key<'_>) -> Result<Self::Value, DispatchError> {
		CollectionMetadataOf::<T, I>::get(collection)
			.map(|collection_metadata| collection_metadata.data)
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<I: 'static, T: Config<I>>
	UpdateMetadata<T::RuntimeOrigin, strategy::Collection<GenericMetadata>> for Pallet<T, I>
{
	fn update_asset_metadata(
		origin: T::RuntimeOrigin,
		collection: Self::Key<'_>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		match update {
			Some(data) =>
				Self::do_set_collection_metadata(maybe_check_origin, *collection, data.clone()),
			None => Self::do_clear_collection_metadata(maybe_check_origin, *collection),
		}
	}
}

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Collection<RegularAttributes>>
	for Pallet<T, I>
{
	type Key<'a> = (&'a T::CollectionId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Collection<RegularAttributes>>
	for Pallet<T, I>
{
	fn asset_metadata(
		(collection, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let attribute =
			BoundedSlice::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((
			collection,
			Option::<T::ItemId>::None,
			AttributeNamespace::CollectionOwner,
			attribute,
		))
		.map(|a| a.0)
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Collection<SystemAttributes>>
	for Pallet<T, I>
{
	type Key<'a> = (&'a T::CollectionId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Collection<SystemAttributes>>
	for Pallet<T, I>
{
	fn asset_metadata(
		(collection, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let item: Option<T::ItemId> = None;
		let namespace = AttributeNamespace::Pallet;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, item, namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Item<GenericMetadata>>
	for Pallet<T, I>
{
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId);
	type Value = BoundedVec<u8, T::StringLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Item<GenericMetadata>> for Pallet<T, I> {
	fn asset_metadata((collection, item): Self::Key<'_>) -> Result<Self::Value, DispatchError> {
		ItemMetadataOf::<T, I>::get(collection, item)
			.map(|m| m.data)
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<T::RuntimeOrigin, strategy::Item<GenericMetadata>>
	for Pallet<T, I>
{
	fn update_asset_metadata(
		origin: T::RuntimeOrigin,
		(collection, item): Self::Key<'_>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		match update {
			Some(data) => Self::do_set_item_metadata(
				maybe_check_origin,
				*collection,
				*item,
				data.clone(),
				None,
			),
			None => Self::do_clear_item_metadata(maybe_check_origin, *collection, *item),
		}
	}
}

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Item<RegularAttributes>>
	for Pallet<T, I>
{
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Item<RegularAttributes>> for Pallet<T, I> {
	fn asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let namespace = AttributeNamespace::CollectionOwner;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<T::RuntimeOrigin, strategy::Item<RegularAttributes>>
	for Pallet<T, I>
{
	fn update_asset_metadata(
		origin: T::RuntimeOrigin,
		(collection, item, attribute): Self::Key<'_>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
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

impl<I: 'static, T: Config<I>> MetadataDefinition<strategy::Item<SystemAttributes>>
	for Pallet<T, I>
{
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<strategy::Item<SystemAttributes>> for Pallet<T, I> {
	fn asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let namespace = AttributeNamespace::Pallet;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}
