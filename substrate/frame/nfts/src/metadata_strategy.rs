use core::marker::PhantomData;

pub struct Collection<Strategy>(PhantomData<Strategy>);
pub struct Item<Strategy>(PhantomData<Strategy>);

pub struct GenericMetadata;

pub struct RegularAttributes;

pub struct SystemAttributes;

pub struct CustomAttributes;
