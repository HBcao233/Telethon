// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pyo3::{PyResult, Python};

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::ops::{ControlFlow, Deref};
use std::{fmt, panic};

use grammers_mtproto::{mtp, transport};
use grammers_session::updates::UpdatesLike;
use grammers_session_pyo3::Session;
use grammers_tl_types::{self as tl, enums};
use tokio::task::AbortHandle;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

use grammers_session_pyo3::PyDcOption;

use crate::ServerAddr;
use crate::configuration::ConnectionParams;
use crate::errors::ReadError;
use crate::{InvocationError, Sender, connect, connect_with_auth};

pub(crate) type Transport = transport::Full;

type InvokeResponse = Vec<u8>;

enum Request {
    Invoke {
        dc_id: i32,
        body: Vec<u8>,
        tx: oneshot::Sender<Result<InvokeResponse, InvocationError>>,
    },
    Disconnect {
        dc_id: i32,
    },
    Quit,
}

struct Rpc {
    body: Vec<u8>,
    tx: oneshot::Sender<Result<InvokeResponse, InvocationError>>,
}

struct ConnectionInfo {
    dc_id: i32,
    rpc_tx: mpsc::UnboundedSender<Rpc>,
    abort_handle: AbortHandle,
}

/// A fat [`SenderPoolHandle`] with additional metadata from its attached [`SenderPoolRunner`].
pub struct SenderPoolFatHandle {
    /// The inner thin handle that self can be derefed into.
    ///
    /// The rest of fields can be dropped if they are no longer needed.
    pub thin: SenderPoolHandle,
    /// The session in use by the attached [`SenderPoolRunner`].
    ///
    /// The runner will read and persist datacenter options in it.
    pub session: Session,
    /// Developer's [Application Identifier](https://core.telegram.org/myapp).
    ///
    /// The [`SenderPoolRunner`] will make use of this value when it needs
    /// to invoke [`tl::functions::InitConnection`] after creating a new connection.
    pub api_id: i32,
}

/// Cheaply cloneable handle to interact with its [`SenderPoolRunner`].
pub struct SenderPoolHandle(mpsc::UnboundedSender<Request>);

/// Builder to configure the runner to drive I/O and linked handles.
pub struct SenderPool {
    /// The single mutable instance responsible for driving I/O.
    ///
    /// Connections are created on-demand, so any errors while the pool
    /// is running can only be retrieved with one of the [`SenderPool::handle`]s.
    pub runner: SenderPoolRunner,
    /// Starting fat handle attached to the [`SenderPool::runner`].
    ///
    /// Handles are the only way to interact with the runner once it's running.
    pub handle: SenderPoolFatHandle,
    /// The single mutable channel through which updates received
    /// from the network by the [`SenderPool::runner`] are delivered.
    ///
    /// Update handling must be processed in a sequential manner,
    /// so this is a separate instance with no way to clone it.
    pub updates: mpsc::UnboundedReceiver<UpdatesLike>,
}

/// Manages and runs a pool of zero or more [`Sender`]s.
///
/// Use [`SenderPool::new`] to create an instance of this type and associated channels.
pub struct SenderPoolRunner {
    session: Session,
    api_id: i32,
    connection_params: ConnectionParams,
    request_rx: mpsc::UnboundedReceiver<Request>,
    updates_tx: mpsc::UnboundedSender<UpdatesLike>,
    connections: Vec<ConnectionInfo>,
    connection_pool: JoinSet<Result<(), ReadError>>,
}

impl Deref for SenderPoolFatHandle {
    type Target = SenderPoolHandle;

    fn deref(&self) -> &Self::Target {
        &self.thin
    }
}

impl SenderPoolHandle {
    /// Communicate with the running [`SenderPoolRunner`] instance
    /// to invoke the serialized request body in the specified datacenter.
    pub async fn invoke_in_dc(
        &self,
        dc_id: i32,
        body: Vec<u8>,
    ) -> Result<InvokeResponse, InvocationError> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send(Request::Invoke { dc_id, body, tx })
            .map_err(|_| InvocationError::Dropped)?;
        rx.await.map_err(|_| InvocationError::Dropped)?
    }

    /// Communicate with the running [`SenderPoolRunner`] instance
    /// to drop any active connections to the given datacenter.
    ///
    /// Has no effect if there was no connection to the datacenter.
    ///
    /// This is useful after datacenter migrations during sign in,
    /// when the old connection is known to not be needed anymore.
    pub fn disconnect_from_dc(&self, dc_id: i32) -> bool {
        self.0.send(Request::Disconnect { dc_id }).is_ok()
    }

    /// Communicate with the running [`SenderPoolRunner`] instance
    /// to drop all active connections and gracefully stop running.
    pub fn quit(&self) -> bool {
        self.0.send(Request::Quit).is_ok()
    }
}

impl SenderPool {
    /// Creates a new sender pool with non-[`ConnectionParams::default`] configuration.
    pub fn new(
        py: Python<'_>,
        session: Session,
        api_id: i32,
        connection_params: ConnectionParams,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();

        Self {
            runner: SenderPoolRunner {
                session: session.clone_ref(py),
                api_id,
                connection_params,
                request_rx,
                updates_tx,
                connections: Vec::new(),
                connection_pool: JoinSet::new(),
            },
            handle: SenderPoolFatHandle {
                thin: SenderPoolHandle(request_tx),
                session,
                api_id,
            },
            updates: updates_rx,
        }
    }
}

impl SenderPoolRunner {
    /// Run the sender pool until [`SenderPoolHandle::quit`] is called or the returned future is dropped.
    ///
    /// Connections will be initiated on-demand whenever the first request to a datacenter is made.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                biased;
                completion = self.connection_pool.join_next(), if !self.connection_pool.is_empty() => {
                    if let Err(err) = completion.unwrap() {
                        if let Ok(reason) = err.try_into_panic() {
                            panic::resume_unwind(reason);
                        }
                    }
                    self.connections
                        .retain(|connection| !connection.abort_handle.is_finished());
                }
                request = self.request_rx.recv() => {
                    let flow = if let Some(request) = request {
                        self.process_request(request).await
                    } else {
                        ControlFlow::Break(())
                    };
                    match flow {
                        ControlFlow::Continue(_) => continue,
                        ControlFlow::Break(_) => break,
                    }
                }
            }
        }

        self.connections.clear(); // drop all channels to cause the `run_sender` loops to stop
        self.connection_pool.join_all().await;
    }

    async fn process_request(&mut self, request: Request) -> ControlFlow<()> {
        match request {
            Request::Invoke { dc_id, body, tx } => {
                let dc_option = match self.session.dc_option(dc_id).await {
                    Ok(dc_option) => dc_option,
                    Err(e) => {
                        let _ = tx.send(Err(InvocationError::PyErr(e)));
                        return ControlFlow::Continue(());
                    }
                };
                let Some(dc_option) = dc_option else {
                    let _ = tx.send(Err(InvocationError::InvalidDc(dc_id)));
                    return ControlFlow::Continue(());
                };

                let connection = match self
                    .connections
                    .iter()
                    .find(|connection| connection.dc_id == dc_id)
                {
                    Some(connection) => connection,
                    None => match self.connect_to_dc(&dc_option).await {
                        Ok(t) => t,
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            return ControlFlow::Continue(());
                        }
                    },
                };
                let _ = connection.rpc_tx.send(Rpc { body, tx });
                ControlFlow::Continue(())
            }
            Request::Disconnect { dc_id } => {
                self.connections.retain(|connection| {
                    if connection.dc_id == dc_id {
                        connection.abort_handle.abort();
                        false
                    } else {
                        true
                    }
                });
                ControlFlow::Continue(())
            }
            Request::Quit => ControlFlow::Break(()),
        }
    }

    async fn connect_to_dc(
        &mut self,
        dc_option: &PyDcOption,
    ) -> Result<&ConnectionInfo, InvocationError> {
        let sender = self.connect_sender(&dc_option).await?;

        let mut dc_option = dc_option.clone();

        dc_option.auth_key = Some(sender.auth_key());
        let dc_id = dc_option.id;
        self.session
            .set_dc_option(dc_option)
            .await
            .map_err(InvocationError::PyErr)?;

        let (rpc_tx, rpc_rx) = mpsc::unbounded_channel();
        let home_dc_id = self
            .session
            .home_dc_id()
            .await
            .map_err(InvocationError::PyErr)?;

        let abort_handle = self.connection_pool.spawn(run_sender(
            sender,
            rpc_rx,
            self.updates_tx.clone(),
            dc_id == home_dc_id,
        ));
        self.connections.push(ConnectionInfo {
            dc_id,
            rpc_tx,
            abort_handle,
        });
        Ok(self.connections.last().unwrap())
    }

    async fn connect_sender(
        &mut self,
        dc_option: &PyDcOption,
    ) -> Result<Sender<transport::Full, mtp::Encrypted>, InvocationError> {
        let transport = transport::Full::new;

        let addr = || {
            if let Some(proxy) = self.connection_params.proxy_url.clone() {
                ServerAddr::Proxied {
                    address: dc_option.ipv4.clone().into(),
                    proxy,
                }
            } else {
                ServerAddr::Tcp {
                    address: dc_option.ipv4.clone().into(),
                }
            }
        };

        let init_connection = tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id: self.api_id,
                device_model: self.connection_params.device_model.clone(),
                system_version: self.connection_params.system_version.clone(),
                app_version: self.connection_params.app_version.clone(),
                system_lang_code: self.connection_params.system_lang_code.clone(),
                lang_pack: "".into(),
                lang_code: self.connection_params.lang_code.clone(),
                proxy: None,
                params: None,
                query: tl::functions::help::GetConfig {},
            },
        };

        let mut sender = if let Some(auth_key) = dc_option.auth_key {
            connect_with_auth(transport(), addr(), auth_key).await?
        } else {
            connect(transport(), addr()).await?
        };

        let enums::Config::Config(remote_config) = match sender.invoke(&init_connection).await {
            Ok(config) => config,
            Err(InvocationError::Transport(transport::Error::BadStatus { status: 404 })) => {
                sender = connect(transport(), addr()).await?;
                sender.invoke(&init_connection).await?
            }
            Err(e) => return Err(e),
        };

        self.update_config(remote_config)
            .await
            .map_err(|e| InvocationError::PyErr(e))?;

        Ok(sender)
    }

    async fn update_config(&mut self, config: tl::types::Config) -> PyResult<()> {
        for option in config
            .dc_options
            .iter()
            .map(|tl::enums::DcOption::Option(option)| option)
            .filter(|option| !option.media_only && !option.tcpo_only && option.r#static)
        {
            let mut dc_option =
                self.session
                    .dc_option(option.id)
                    .await?
                    .unwrap_or_else(|| PyDcOption {
                        id: option.id,
                        ipv4: SocketAddrV4::new(Ipv4Addr::from_bits(0), 0).into(),
                        ipv6: SocketAddrV6::new(Ipv6Addr::from_bits(0), 0, 0, 0).into(),
                        auth_key: None,
                    });
            if option.ipv6 {
                dc_option.ipv6 = SocketAddrV6::new(
                    option
                        .ip_address
                        .parse()
                        .expect("Telegram to return a valid IPv6 address"),
                    option.port as _,
                    0,
                    0,
                )
                .into();
            } else {
                dc_option.ipv4 = SocketAddrV4::new(
                    option
                        .ip_address
                        .parse()
                        .expect("Telegram to return a valid IPv4 address"),
                    option.port as _,
                )
                .into();
                if dc_option.ipv6.ip().to_bits() == 0 {
                    dc_option.ipv6 = SocketAddrV6::new(
                        dc_option.ipv4.ip().to_ipv6_mapped(),
                        dc_option.ipv4.port(),
                        0,
                        0,
                    )
                    .into()
                }
            }
        }
        Ok(())
    }
}

async fn run_sender(
    mut sender: Sender<Transport, grammers_mtproto::mtp::Encrypted>,
    mut rpc_rx: mpsc::UnboundedReceiver<Rpc>,
    updates: mpsc::UnboundedSender<UpdatesLike>,
    home_sender: bool,
) -> Result<(), ReadError> {
    loop {
        tokio::select! {
            step = sender.step() => match step {
                Ok(all_new_updates) => all_new_updates.into_iter().for_each(|new_updates| {
                    let _ = updates.send(new_updates);
                }),
                Err(err) => {
                    if home_sender {
                        let _ = updates.send(UpdatesLike::ConnectionClosed);
                    }
                    break Err(err)
                },
            },
            rpc = rpc_rx.recv() => match rpc {
                Some(rpc) => sender.enqueue_body(rpc.body, rpc.tx),
                None => break Ok(()),
            },
        }
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invoke { dc_id, body, tx } => f
                .debug_struct("Invoke")
                .field("dc_id", dc_id)
                .field(
                    "request",
                    &body[..4]
                        .try_into()
                        .map(|constructor_id| tl::name_for_id(u32::from_le_bytes(constructor_id)))
                        .unwrap_or("?"),
                )
                .field("tx", tx)
                .finish(),
            Self::Disconnect { dc_id } => {
                f.debug_struct("Disconnect").field("dc_id", dc_id).finish()
            }
            Self::Quit => write!(f, "Quit"),
        }
    }
}
