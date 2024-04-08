use crate::dispatch::{DispatchResult, Parameter};
use sp_runtime::DispatchError;

/// Trait for providing types for identifying NFT-like asset classes and instances.
pub trait Identification {
    /// Type for identifying a class (an identifier for an independent collection of
    /// items).
    type ClassId: Parameter;

    /// Type for identifying an instance.
    type InstanceId: Parameter;
}

pub trait Ownership<AccountId>: Identification {
    fn class_owner(class: &Self::ClassId) -> Result<AccountId, DispatchError>;

    fn instance_owner(class: &Self::ClassId, instance: &Self::InstanceId) -> Result<AccountId, DispatchError>;
}

pub trait MintInstanceInto<InstanceData, AccountId>: Identification {
    fn mint_instance_into(
        class: &Self::ClassId,
        data: &InstanceData,
        into: &AccountId,
    ) -> Result<Self::InstanceId, DispatchError>;
}

pub trait MintInstanceWithIdInto<InstanceData>: Identification {
    fn mint_instance_with_id_into(
        class: &Self::ClassId,
        instance: &Self::InstanceId,
        data: &InstanceData,
    ) -> DispatchResult;
}

pub trait TransferInstanceFromTo<AccountId>: Identification {
    fn transfer_instance_from_to(
        class: &Self::ClassId,
        instance: &Self::InstanceId,
        from: &AccountId,
        to: &AccountId,
    ) -> DispatchResult;
}

pub trait TransferInstanceTo<AccountId>: Identification {
    fn transfer_instance_to(
        class: &Self::ClassId,
        instance: &Self::InstanceId,
        to: &AccountId,
    ) -> DispatchResult;
}

pub trait BurnInstanceFrom<AccountId>: Identification {
    fn burn_instance_from(
        class: &Self::ClassId,
        instance: &Self::InstanceId,
        from: &AccountId,
    ) -> DispatchResult;
}

pub trait BurnInstance<AccountId>: Identification {
    fn burn_instance(
        class: &Self::ClassId,
        instance: &Self::InstanceId,
    ) -> DispatchResult;
}
