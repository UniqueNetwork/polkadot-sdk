use crate::dispatch::DispatchResult;
use sp_runtime::DispatchError;

pub trait MetadataDefinition<Strategy> {
    type Key<'a>;
    type Value;
}

pub trait InspectMetadata<Strategy>: MetadataDefinition<Strategy> {
    fn asset_metadata(key: Self::Key<'_>) -> Result<Self::Value, DispatchError>;
}

pub trait UpdateMetadata<RuntimeOrigin, Strategy>: MetadataDefinition<Strategy> {
    fn update_asset_metadata(
        origin: RuntimeOrigin,
        key: Self::Key<'_>,
        update: Option<&Self::Value>
    ) -> DispatchResult;
}
