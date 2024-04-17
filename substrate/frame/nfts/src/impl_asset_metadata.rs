use super::*;

use crate::types::metadata_strategies::*;
use frame_support::{
	dispatch::DispatchResult,
	traits::{
		tokens::{
			asset_metadata::{InspectMetadata, MetadataDefinition, UpdateMetadata},
			common_asset_strategies::{CheckOrigin, Primary},
			unique_assets::common_asset_kinds::{Class, Instance},
		},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_runtime::{BoundedVec, DispatchError};

impl<I: 'static, T: Config<I>> MetadataDefinition<Class, Primary> for Pallet<T, I> {
	type Key<'a> = &'a T::CollectionId;
	type Value = BoundedVec<u8, T::StringLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Class, Primary> for Pallet<T, I> {
	fn asset_metadata(
		collection: Self::Key<'_>,
		_primary: Primary,
	) -> Result<Self::Value, DispatchError> {
		CollectionMetadataOf::<T, I>::get(collection)
			.map(|collection_metadata| collection_metadata.data)
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<Class, Primary> for Pallet<T, I> {
	fn update_asset_metadata(
		collection: Self::Key<'_>,
		_primary: Primary,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = None;

		match update {
			Some(data) =>
				Self::do_set_collection_metadata(maybe_check_origin, *collection, data.clone()),
			None => Self::do_clear_collection_metadata(maybe_check_origin, *collection),
		}
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<Class, CheckOrigin<T::RuntimeOrigin, Primary>>
	for Pallet<T, I>
{
	fn update_asset_metadata(
		collection: Self::Key<'_>,
		strategy: CheckOrigin<T::RuntimeOrigin, Primary>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let CheckOrigin(origin, _primary) = strategy;

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

impl<I: 'static, T: Config<I>> MetadataDefinition<Class, RegularAttributes> for Pallet<T, I> {
	type Key<'a> = (&'a T::CollectionId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Class, RegularAttributes> for Pallet<T, I> {
	fn asset_metadata(
		(collection, attribute): Self::Key<'_>,
		_regular_attributes: RegularAttributes,
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

impl<I: 'static, T: Config<I>> MetadataDefinition<Class, SystemAttributes> for Pallet<T, I> {
	type Key<'a> = (&'a T::CollectionId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Class, SystemAttributes> for Pallet<T, I> {
	fn asset_metadata(
		(collection, attribute): Self::Key<'_>,
		_system_attributes: SystemAttributes,
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

impl<I: 'static, T: Config<I>> MetadataDefinition<Instance, Primary> for Pallet<T, I> {
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId);
	type Value = BoundedVec<u8, T::StringLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Instance, Primary> for Pallet<T, I> {
	fn asset_metadata(
		(collection, item): Self::Key<'_>,
		_primary: Primary,
	) -> Result<Self::Value, DispatchError> {
		ItemMetadataOf::<T, I>::get(collection, item)
			.map(|m| m.data)
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<Instance, Primary> for Pallet<T, I> {
	fn update_asset_metadata(
		(collection, item): Self::Key<'_>,
		_primary: Primary,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = None;

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

impl<I: 'static, T: Config<I>> UpdateMetadata<Instance, CheckOrigin<T::RuntimeOrigin, Primary>>
	for Pallet<T, I>
{
	fn update_asset_metadata(
		(collection, item): Self::Key<'_>,
		strategy: CheckOrigin<T::RuntimeOrigin, Primary>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let CheckOrigin(origin, _primary) = strategy;

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

impl<I: 'static, T: Config<I>> MetadataDefinition<Instance, RegularAttributes> for Pallet<T, I> {
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Instance, RegularAttributes> for Pallet<T, I> {
	fn asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
		_regular_attributes: RegularAttributes,
	) -> Result<Self::Value, DispatchError> {
		let namespace = AttributeNamespace::CollectionOwner;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<I: 'static, T: Config<I>> UpdateMetadata<Instance, RegularAttributes> for Pallet<T, I> {
	fn update_asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
		_regular_attributes: RegularAttributes,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = None;
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

impl<I: 'static, T: Config<I>>
	UpdateMetadata<Instance, CheckOrigin<T::RuntimeOrigin, RegularAttributes>> for Pallet<T, I>
{
	fn update_asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
		strategy: CheckOrigin<T::RuntimeOrigin, RegularAttributes>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let CheckOrigin(origin, _regular_attributes) = strategy;

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

impl<I: 'static, T: Config<I>> MetadataDefinition<Instance, SystemAttributes> for Pallet<T, I> {
	type Key<'a> = (&'a T::CollectionId, &'a T::ItemId, &'a [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<I: 'static, T: Config<I>> InspectMetadata<Instance, SystemAttributes> for Pallet<T, I> {
	fn asset_metadata(
		(collection, item, attribute): Self::Key<'_>,
		_system_attributes: SystemAttributes,
	) -> Result<Self::Value, DispatchError> {
		let namespace = AttributeNamespace::Pallet;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, Some(item), namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}
