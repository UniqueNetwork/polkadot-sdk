use core::marker::PhantomData;
use frame_support::traits::{
	tokens::asset_ops::{
		common_asset_kinds::{Class, Instance},
		common_strategies::{DeriveIdFrom, FromTo, IfOwnedBy, Owned, PredefinedId},
		AssetDefinition, Create, Destroy, Transfer,
	},
	Get,
};
use xcm::latest::prelude::*;
use xcm_executor::traits::{ConvertLocation, Error as MatchError, MatchesInstance, TransactAsset};

const LOG_TARGET: &str = "xcm::unique_instances";

pub struct TransferableInstanceAdapter<
	AccountId,
	AccountIdConverter,
	Matcher,
	InstanceTransfer,
	StashLocation,
>(PhantomData<(AccountId, AccountIdConverter, Matcher, InstanceTransfer, StashLocation)>);

impl<
		AccountId,
		AccountIdConverter: ConvertLocation<AccountId>,
		Matcher: MatchesInstance<InstanceTransfer::Id>,
		InstanceTransfer: for<'a> Transfer<Instance, FromTo<'a, AccountId>>,
		StashLocation: Get<Location>,
	> TransactAsset
	for TransferableInstanceAdapter<
		AccountId,
		AccountIdConverter,
		Matcher,
		InstanceTransfer,
		StashLocation,
	>
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what,
			&StashLocation::get(),
			who,
		)
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what,
			who,
			&StashLocation::get(),
		)?;

		Ok(what.clone().into())
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what, from, to,
		)?;

		Ok(what.clone().into())
	}
}

pub struct RecreateableInstanceAdapter<AccountId, AccountIdConverter, Matcher, InstanceOps>(
	PhantomData<(AccountId, AccountIdConverter, Matcher, InstanceOps)>,
);

impl<AccountId, AccountIdConverter, Matcher, InstanceOps> TransactAsset
	for RecreateableInstanceAdapter<AccountId, AccountIdConverter, Matcher, InstanceOps>
where
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<InstanceOps::Id>,
	for<'a> InstanceOps: Create<Instance, Owned<'a, PredefinedId<'a, InstanceOps::Id>, AccountId>>
		+ Transfer<Instance, FromTo<'a, AccountId>>
		+ Destroy<Instance, IfOwnedBy<'a, AccountId>>,
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		let instance_id = Matcher::matches_instance(what)?;
		let who = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		InstanceOps::create(Owned::new(PredefinedId(&instance_id), &who))
			.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);
		let instance_id = Matcher::matches_instance(what)?;
		let who = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		InstanceOps::destroy(&instance_id, IfOwnedBy(&who))
			.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

		Ok(what.clone().into())
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceOps>(what, from, to)?;

		Ok(what.clone().into())
	}
}

fn transfer_instance<
	AccountId,
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<InstanceTransfer::Id>,
	InstanceTransfer: for<'a> Transfer<Instance, FromTo<'a, AccountId>>,
>(
	what: &Asset,
	from: &Location,
	to: &Location,
) -> XcmResult {
	let instance_id = Matcher::matches_instance(what)?;
	let from =
		AccountIdConverter::convert_location(from).ok_or(MatchError::AccountIdConversionFailed)?;
	let to =
		AccountIdConverter::convert_location(to).ok_or(MatchError::AccountIdConversionFailed)?;

	InstanceTransfer::transfer(&instance_id, FromTo(&from, &to))
		.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
}

pub enum DerivativeStatus<ClassId, InstanceId> {
	DepositableIn(ClassId),
	Exists(InstanceId),
}

pub struct BackedDerivativeInstanceAdapter<
	AccountId,
	AccountIdConverter,
	Matcher,
	ClassDef,
	InstanceOps,
	StashLocation,
>(PhantomData<(AccountId, AccountIdConverter, Matcher, ClassDef, InstanceOps, StashLocation)>);

impl<AccountId, AccountIdConverter, Matcher, ClassDef, InstanceOps, StashLocation> TransactAsset
	for BackedDerivativeInstanceAdapter<
		AccountId,
		AccountIdConverter,
		Matcher,
		ClassDef,
		InstanceOps,
		StashLocation,
	> where
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<DerivativeStatus<ClassDef::Id, InstanceOps::Id>>,
	ClassDef: AssetDefinition<Class>,
	for<'a> InstanceOps: AssetDefinition<Instance>
		+ Create<Instance, Owned<'a, DeriveIdFrom<'a, ClassDef::Id, InstanceOps::Id>, AccountId>>
		+ Transfer<Instance, FromTo<'a, AccountId>>,
	StashLocation: Get<Location>,
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let to = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		let result = match derivative_status {
			DerivativeStatus::DepositableIn(class_id) =>
				InstanceOps::create(Owned::new(DeriveIdFrom::parent_id(&class_id), &to))
					.map(|_id| ()),
			DerivativeStatus::Exists(instance_id) => {
				let from = AccountIdConverter::convert_location(&StashLocation::get())
					.ok_or(MatchError::AccountIdConversionFailed)?;

				InstanceOps::transfer(&instance_id, FromTo(&from, &to))
			},
		};

		result.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let from = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		if let DerivativeStatus::Exists(instance_id) = derivative_status {
			let to = AccountIdConverter::convert_location(&StashLocation::get())
				.ok_or(MatchError::AccountIdConversionFailed)?;

			InstanceOps::transfer(&instance_id, FromTo(&from, &to))
				.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

			Ok(what.clone().into())
		} else {
			Err(XcmError::NotWithdrawable)
		}
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let from = AccountIdConverter::convert_location(from)
			.ok_or(MatchError::AccountIdConversionFailed)?;
		let to = AccountIdConverter::convert_location(to)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		if let DerivativeStatus::Exists(instance_id) = derivative_status {
			InstanceOps::transfer(&instance_id, FromTo(&from, &to))
				.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

			Ok(what.clone().into())
		} else {
			Err(XcmError::NotWithdrawable)
		}
	}
}
