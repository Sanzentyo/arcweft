//! Bounded signature-request admission, cancellation, and immutable request carriers.

pub(crate) mod control;
pub(crate) mod executor;
pub(crate) mod registry;
pub(crate) mod signature;

pub(crate) use control::{
    RequestControl, RequestGateState, SignatureCancellationReason, SignatureRequestBinding,
};
pub(crate) use executor::{RequestRuntimeError, SignatureRequestRuntime};
pub(crate) use registry::{ActiveRequest, RequestAdmissionError, RequestRegistry};

#[cfg(test)]
pub(crate) fn with_test_request_registry<T>(run: impl FnOnce(&RequestRegistry) -> T) -> T {
    struct RegistryGuard(std::sync::Arc<RequestRegistry>);

    impl Drop for RegistryGuard {
        fn drop(&mut self) {
            self.0.shutdown();
        }
    }

    let registry =
        RegistryGuard(RequestRegistry::try_new().expect("test signature request registry starts"));
    run(registry.0.as_ref())
}
