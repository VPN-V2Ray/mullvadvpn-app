#![allow(missing_docs)]

use self::config::Config;
use super::{TunnelEvent, TunnelMetadata};
use crate::routing;
use std::{io, path::Path, sync::mpsc};

pub mod config;
mod ping_monitor;
pub mod wireguard_go;

pub use self::wireguard_go::WgGoTunnel;

// amount of seconds to run `ping` until it returns.
const PING_TIMEOUT: u16 = 7;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can happen in the Wireguard tunnel monitor.
#[derive(err_derive::Error, Debug)]
pub enum Error {
    /// Failed to setup a tunnel device.
    #[error(display = "Failed to create tunnel device")]
    SetupTunnelDeviceError(#[error(cause)] crate::network_interface::Error),

    /// Failed to setup wireguard tunnel.
    #[error(display = "Failed to start wireguard tunnel - {}", status)]
    StartWireguardError { status: i32 },

    /// Failed to tear down wireguard tunnel.
    #[error(display = "Failed to stop wireguard tunnel - {}", status)]
    StopWireguardError { status: i32 },

    /// Failed to set up routing.
    #[error(display = "Failed to setup routing")]
    SetupRoutingError(#[error(cause)] crate::routing::Error),

    /// Failed to move or craete a log file.
    #[error(display = "Failed to setup a logging file")]
    PrepareLogFileError(#[error(cause)] io::Error),

    /// Invalid tunnel interface name.
    #[error(display = "Invalid tunnel interface name")]
    InterfaceNameError(#[error(cause)] std::ffi::NulError),

    /// Pinging timed out.
    #[error(display = "Ping timed out")]
    PingTimeoutError,
}

/// Spawns and monitors a wireguard tunnel
pub struct WireguardMonitor {
    /// Tunnel implementation
    tunnel: Box<dyn Tunnel>,
    /// Route manager
    router: routing::RouteManager,
    /// Callback to signal tunnel events
    event_callback: Box<dyn Fn(TunnelEvent) + Send + Sync + 'static>,
    close_msg_sender: mpsc::Sender<CloseMsg>,
    close_msg_receiver: mpsc::Receiver<CloseMsg>,
}

impl WireguardMonitor {
    pub fn start<F: Fn(TunnelEvent) + Send + Sync + Clone + 'static>(
        config: &Config,
        log_path: Option<&Path>,
        on_event: F,
    ) -> Result<WireguardMonitor> {
        let tunnel = Box::new(WgGoTunnel::start_tunnel(&config, log_path)?);
        let router = routing::RouteManager::new().map_err(Error::SetupRoutingError)?;
        let event_callback = Box::new(on_event.clone());
        let (close_msg_sender, close_msg_receiver) = mpsc::channel();
        let mut monitor = WireguardMonitor {
            tunnel,
            router,
            event_callback,
            close_msg_sender,
            close_msg_receiver,
        };
        monitor.setup_routing(&config)?;

        let metadata = monitor.tunnel_metadata(&config);
        let iface_name = monitor.tunnel.get_interface_name().to_string();
        let gateway = config.ipv4_gateway.into();
        let close_sender = monitor.close_msg_sender.clone();

        ::std::thread::spawn(move || {
            match ping_monitor::ping(gateway, PING_TIMEOUT, &iface_name, true) {
                Ok(()) => {
                    (on_event)(TunnelEvent::Up(metadata));
                }
                Err(e) => {
                    log::error!("First ping to gateway failed - {}", e);
                    let _ = close_sender.send(CloseMsg::PingErr);
                }
            };

            if let Err(e) = ping_monitor::monitor_ping(gateway, PING_TIMEOUT, &iface_name) {
                log::trace!("Ping monitor failed - {}", e);
            }
            let _ = close_sender.send(CloseMsg::PingErr);
        });


        Ok(monitor)
    }

    pub fn close_handle(&self) -> CloseHandle {
        CloseHandle {
            chan: self.close_msg_sender.clone(),
        }
    }

    pub fn wait(mut self) -> Result<()> {
        let wait_result = match self.close_msg_receiver.recv() {
            Ok(CloseMsg::PingErr) => Err(Error::PingTimeoutError),
            Ok(CloseMsg::Stop) => Ok(()),
            Err(_) => Ok(()),
        };

        // Clear routes manually - otherwise there will be some log spam since the tunnel device
        // can be removed before the routes are cleared, which automatically clears some of the
        // routes that were set.
        if let Err(e) = self.router.delete_routes() {
            log::error!("Failed to remove a route from the routing table - {}", e);
        }

        if let Err(e) = self.tunnel.stop() {
            log::error!("Failed to stop tunnel - {}", e);
        }
        (self.event_callback)(TunnelEvent::Down);
        wait_result
    }

    fn setup_routing(&mut self, config: &Config) -> Result<()> {
        let iface_name = self.tunnel.get_interface_name();
        let mut routes: Vec<_> = config
            .peers
            .iter()
            .flat_map(|peer| peer.allowed_ips.iter())
            .cloned()
            .map(|allowed_ip| {
                routing::Route::new(allowed_ip, routing::NetNode::Device(iface_name.to_string()))
            })
            .collect();

        // To survive network roaming, we should listen for new routes and reapply them
        // here - probably would need RouteManager be extended. Or maybe RouteManager can deal
        // with it on it's own
        let default_node = self
            .router
            .get_default_route_node()
            .map_err(Error::SetupRoutingError)?;

        // route endpoints with specific routes
        for peer in config.peers.iter() {
            let default_route = routing::Route::new(
                peer.endpoint.ip().into(),
                routing::NetNode::Address(default_node),
            );
            routes.push(default_route);
        }

        let required_routes = routing::RequiredRoutes { routes };

        self.router
            .add_routes(required_routes)
            .map_err(Error::SetupRoutingError)
    }

    fn tunnel_metadata(&self, config: &Config) -> TunnelMetadata {
        let interface_name = self.tunnel.get_interface_name();
        TunnelMetadata {
            interface: interface_name.to_string(),
            ips: config.tunnel.addresses.clone(),
            ipv4_gateway: config.ipv4_gateway,
            ipv6_gateway: config.ipv6_gateway,
        }
    }
}

enum CloseMsg {
    Stop,
    PingErr,
}

#[derive(Clone, Debug)]
pub struct CloseHandle {
    chan: mpsc::Sender<CloseMsg>,
}


impl CloseHandle {
    pub fn close(&mut self) {
        if let Err(e) = self.chan.send(CloseMsg::Stop) {
            log::trace!("Failed to send close message to wireguard tunnel - {}", e);
        }
    }
}

pub trait Tunnel: Send {
    fn get_interface_name(&self) -> &str;
    fn stop(self: Box<Self>) -> Result<()>;
}
