use arcweft_runtime_plan::assertion_identity::{
    AssertionConditionIndex, AssertionPresentation, RuntimeAssertionFault,
    RuntimeAssertionFaultIdentity, RuntimeAssertionInventory, RuntimeAssertionMode,
    RuntimeAssertionSite,
};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

macro_rules! require_serde {
    ($($identity:ty),+ $(,)?) => {
        $(
            requires_serialize::<$identity>();
            requires_deserialize::<$identity>();
        )+
    };
}

fn main() {
    require_serde!(
        RuntimeAssertionMode,
        AssertionConditionIndex,
        AssertionPresentation,
        RuntimeAssertionSite,
        RuntimeAssertionInventory,
        RuntimeAssertionFaultIdentity,
        RuntimeAssertionFault,
    );
}
