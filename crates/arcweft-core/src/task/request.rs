//! One host request grammar over live or inert payloads.

use super::{HostTaskRequest, HttpFetchRequest, HttpRespondRequest, NamedHostArg, WasmCallRequest};

impl<P> HostTaskRequest<P> {
    pub(crate) fn payloads(&self) -> impl Iterator<Item = &P> {
        let (body, args, named): (Option<&P>, &[P], &[NamedHostArg<P>]) = match self {
            Self::HttpFetch(r) => (r.body.as_ref(), &[], &[]),
            Self::HttpRespond(r) => (r.body.as_ref(), &[], &[]),
            Self::WasmCall(r) => (None, &r.args, &[]),
            Self::Custom {
                args, named_args, ..
            } => (None, args, named_args),
            Self::FileReadText(_)
            | Self::FileReadBytes(_)
            | Self::FileWriteText(_)
            | Self::FileWriteBytes(_)
            | Self::ProcessRun(_)
            | Self::AssetLoad(_)
            | Self::ShaderCompile(_)
            | Self::AudioDecode(_)
            | Self::TtsSynthesis(_)
            | Self::SystemInfo(_) => (None, &[], &[]),
        };
        body.into_iter()
            .chain(args)
            .chain(named.iter().map(|arg| &arg.value))
    }

    /// Called only after the complete descriptor/value batch is admitted.
    pub(super) fn map_admitted<Q, E>(
        &self,
        mut payload: impl FnMut(&P) -> Result<Q, E>,
    ) -> Result<HostTaskRequest<Q>, E> {
        Ok(match self {
            Self::FileReadText(r) => HostTaskRequest::FileReadText(r.clone()),
            Self::FileReadBytes(r) => HostTaskRequest::FileReadBytes(r.clone()),
            Self::FileWriteText(r) => HostTaskRequest::FileWriteText(r.clone()),
            Self::FileWriteBytes(r) => HostTaskRequest::FileWriteBytes(r.clone()),
            Self::HttpFetch(r) => HostTaskRequest::HttpFetch(HttpFetchRequest {
                url: r.url.clone(),
                method: r.method.clone(),
                headers: r.headers.clone(),
                body: r.body.as_ref().map(&mut payload).transpose()?,
            }),
            Self::HttpRespond(r) => HostTaskRequest::HttpRespond(HttpRespondRequest {
                request_id: r.request_id.clone(),
                status: r.status,
                headers: r.headers.clone(),
                body: r.body.as_ref().map(&mut payload).transpose()?,
            }),
            Self::ProcessRun(r) => HostTaskRequest::ProcessRun(r.clone()),
            Self::AssetLoad(r) => HostTaskRequest::AssetLoad(r.clone()),
            Self::ShaderCompile(r) => HostTaskRequest::ShaderCompile(r.clone()),
            Self::AudioDecode(r) => HostTaskRequest::AudioDecode(r.clone()),
            Self::TtsSynthesis(r) => HostTaskRequest::TtsSynthesis(r.clone()),
            Self::WasmCall(r) => HostTaskRequest::WasmCall(WasmCallRequest {
                module: r.module.clone(),
                function: r.function.clone(),
                args: r.args.iter().map(&mut payload).collect::<Result<_, _>>()?,
            }),
            Self::SystemInfo(r) => HostTaskRequest::SystemInfo(r.clone()),
            Self::Custom {
                capability,
                operation,
                args,
                named_args,
                manifest_contract,
            } => HostTaskRequest::Custom {
                capability: capability.clone(),
                operation: operation.clone(),
                args: args.iter().map(&mut payload).collect::<Result<_, _>>()?,
                named_args: named_args
                    .iter()
                    .map(|arg| {
                        Ok(NamedHostArg {
                            name: arg.name.clone(),
                            value: payload(&arg.value)?,
                        })
                    })
                    .collect::<Result<_, E>>()?,
                manifest_contract: *manifest_contract,
            },
        })
    }

    /// Consuming restore moves metadata and constructs only admitted payloads.
    pub(super) fn into_mapped_admitted<Q, E>(
        self,
        mut payload: impl FnMut(P) -> Result<Q, E>,
    ) -> Result<HostTaskRequest<Q>, E> {
        Ok(match self {
            Self::FileReadText(r) => HostTaskRequest::FileReadText(r),
            Self::FileReadBytes(r) => HostTaskRequest::FileReadBytes(r),
            Self::FileWriteText(r) => HostTaskRequest::FileWriteText(r),
            Self::FileWriteBytes(r) => HostTaskRequest::FileWriteBytes(r),
            Self::HttpFetch(r) => HostTaskRequest::HttpFetch(HttpFetchRequest {
                url: r.url,
                method: r.method,
                headers: r.headers,
                body: r.body.map(&mut payload).transpose()?,
            }),
            Self::HttpRespond(r) => HostTaskRequest::HttpRespond(HttpRespondRequest {
                request_id: r.request_id,
                status: r.status,
                headers: r.headers,
                body: r.body.map(&mut payload).transpose()?,
            }),
            Self::ProcessRun(r) => HostTaskRequest::ProcessRun(r),
            Self::AssetLoad(r) => HostTaskRequest::AssetLoad(r),
            Self::ShaderCompile(r) => HostTaskRequest::ShaderCompile(r),
            Self::AudioDecode(r) => HostTaskRequest::AudioDecode(r),
            Self::TtsSynthesis(r) => HostTaskRequest::TtsSynthesis(r),
            Self::WasmCall(r) => HostTaskRequest::WasmCall(WasmCallRequest {
                module: r.module,
                function: r.function,
                args: r
                    .args
                    .into_iter()
                    .map(&mut payload)
                    .collect::<Result<_, _>>()?,
            }),
            Self::SystemInfo(r) => HostTaskRequest::SystemInfo(r),
            Self::Custom {
                capability,
                operation,
                args,
                named_args,
                manifest_contract,
            } => HostTaskRequest::Custom {
                capability,
                operation,
                args: args
                    .into_iter()
                    .map(&mut payload)
                    .collect::<Result<_, _>>()?,
                named_args: named_args
                    .into_iter()
                    .map(|arg| {
                        Ok(NamedHostArg {
                            name: arg.name,
                            value: payload(arg.value)?,
                        })
                    })
                    .collect::<Result<_, E>>()?,
                manifest_contract,
            },
        })
    }
}
