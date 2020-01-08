use std::marker::PhantomData;
use std::time::Duration;

use enet_sys::{
    enet_peer_disconnect, enet_peer_disconnect_later, enet_peer_disconnect_now, enet_peer_receive,
    enet_peer_reset, enet_peer_send, ENetPeer, _ENetPeerState,
    _ENetPeerState_ENET_PEER_STATE_ACKNOWLEDGING_CONNECT,
    _ENetPeerState_ENET_PEER_STATE_ACKNOWLEDGING_DISCONNECT,
    _ENetPeerState_ENET_PEER_STATE_CONNECTED, _ENetPeerState_ENET_PEER_STATE_CONNECTING,
    _ENetPeerState_ENET_PEER_STATE_CONNECTION_PENDING,
    _ENetPeerState_ENET_PEER_STATE_CONNECTION_SUCCEEDED,
    _ENetPeerState_ENET_PEER_STATE_DISCONNECTED, _ENetPeerState_ENET_PEER_STATE_DISCONNECTING,
    _ENetPeerState_ENET_PEER_STATE_DISCONNECT_LATER, _ENetPeerState_ENET_PEER_STATE_ZOMBIE,
};

use crate::{Address, Error, Packet};

/// This struct represents an endpoint in an ENet-connection.
///
/// The lifetime of these instances is not really clear from the ENet documentation.
/// Therefore, `Peer`s are always borrowed, and can not really be stored anywhere.
///
/// ENet allows the association of arbitrary data with each peer.
/// The type of this associated data is chosen through `T`.
#[repr(transparent)]
pub struct Peer<T> {
    inner: ENetPeer,
    _data: PhantomData<T>,
}

impl<'a, T> Peer<T>
where
    T: 'a,
{
    pub(crate) fn new(inner: &'a ENetPeer) -> &'a Peer<T> {
        unsafe { &*(inner as *const _ as *const Peer<T>) }
    }

    pub(crate) fn new_mut(inner: &'a mut ENetPeer) -> &'a mut Peer<T> {
        unsafe { &mut *(inner as *mut _ as *mut Peer<T>) }
    }

    /// Returns the address of this `Peer`.
    pub fn address(&self) -> Address {
        Address::from_enet_address(&self.inner.address)
    }

    /// Returns the amout of channels allocated for this `Peer`.
    pub fn channel_count(&self) -> usize {
        self.inner.channelCount
    }

    /// Returns a reference to the data associated with this `Peer`, if set.
    pub fn data(&self) -> Option<&T> {
        unsafe {
            let raw_data = self.inner.data as *const T;

            if raw_data.is_null() {
                None
            } else {
                Some(&(*raw_data))
            }
        }
    }

    /// Returns a mutable reference to the data associated with this `Peer`, if set.
    pub fn data_mut(&mut self) -> Option<&mut T> {
        unsafe {
            let raw_data = self.inner.data as *mut T;

            if raw_data.is_null() {
                None
            } else {
                Some(&mut (*raw_data))
            }
        }
    }

    /// Sets or clears the data associated with this `Peer`, replacing existing data.
    pub fn set_data(&mut self, data: Option<T>) {
        unsafe {
            let raw_data = self.inner.data as *mut T;

            if !raw_data.is_null() {
                // free old data
                let _: Box<T> = Box::from_raw(raw_data);
            }

            let new_data = match data {
                Some(data) => Box::into_raw(Box::new(data)) as *mut _,
                None => std::ptr::null_mut(),
            };

            self.inner.data = new_data;
        }
    }

    /// Returns the downstream bandwidth of this `Peer` in bytes/second.
    pub fn incoming_bandwidth(&self) -> u32 {
        self.inner.incomingBandwidth
    }

    /// Returns the upstream bandwidth of this `Peer` in bytes/second.
    pub fn outgoing_bandwidth(&self) -> u32 {
        self.inner.outgoingBandwidth
    }

    /// Returns the mean round trip time between sending a reliable packet and receiving its acknowledgement.
    pub fn mean_rtt(&self) -> Duration {
        Duration::from_millis(self.inner.roundTripTime as u64)
    }

    /// Forcefully disconnects this `Peer`.
    ///
    /// The foreign host represented by the peer is not notified of the disconnection and will timeout on its connection to the local host.
    pub fn reset(&mut self) {
        unsafe {
            enet_peer_reset(&mut self.inner as *mut _);
        }
    }

    /// Returns the state this `Peer` is in.
    pub fn state(&self) -> PeerState {
        PeerState::from_sys_state(unsafe { self.inner.state })
    }

    /// Queues a packet to be sent.
    ///
    /// Actual sending will happen during `Host::service`.
    pub fn send_packet(&mut self, packet: Packet, channel_id: u8) -> Result<(), Error> {
        let res =
            unsafe { enet_peer_send(&mut self.inner as *mut _, channel_id, packet.into_inner()) };

        match res {
            r if r > 0 => panic!("unexpected res: {}", r),
            0 => Ok(()),
            r if r < 0 => Err(Error(r)),
            _ => panic!("unreachable"),
        }
    }

    /// Disconnects from this peer.
    ///
    /// A `Disconnect` event will be returned by `Host::service` once the disconnection is complete.
    pub fn disconnect(&mut self, user_data: u32) {
        unsafe {
            enet_peer_disconnect(&mut self.inner as *mut _, user_data);
        }
    }

    /// Disconnects from this peer immediately.
    ///
    /// No `Disconnect` event will be created. No disconnect notification for the foreign peer is guaranteed, and this `Peer` is immediately reset on return from this method.
    pub fn disconnect_now(&mut self, user_data: u32) {
        unsafe {
            enet_peer_disconnect_now(&mut self.inner as *mut _, user_data);
        }
    }

    /// Disconnects from this peer after all outgoing packets have been sent.
    ///
    /// A `Disconnect` event will be returned by `Host::service` once the disconnection is complete.
    pub fn disconnect_later(&mut self, user_data: u32) {
        unsafe {
            enet_peer_disconnect_later(&mut self.inner as *mut _, user_data);
        }
    }
}

/// Describes the state a `Peer` is in.
///
/// The states should be self-explanatory, ENet doesn't explain them more either.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum PeerState {
    Disconnected,
    Connected,
    Connecting,
    AcknowledgingConnect,
    ConnectionPending,
    ConnectionSucceeded,
    DisconnectLater,
    Disconnecting,
    AcknowledgingDisconnect,
    Zombie,
}

impl PeerState {
    fn from_sys_state(enet_sys_state: _ENetPeerState) -> PeerState {
        #[allow(non_upper_case_globals)]
        match enet_sys_state {
            _ENetPeerState_ENET_PEER_STATE_DISCONNECTED => PeerState::Disconnected,
            _ENetPeerState_ENET_PEER_STATE_CONNECTING => PeerState::Connecting,
            _ENetPeerState_ENET_PEER_STATE_ACKNOWLEDGING_CONNECT => PeerState::AcknowledgingConnect,
            _ENetPeerState_ENET_PEER_STATE_CONNECTION_PENDING => PeerState::ConnectionPending,
            _ENetPeerState_ENET_PEER_STATE_CONNECTION_SUCCEEDED => PeerState::ConnectionSucceeded,
            _ENetPeerState_ENET_PEER_STATE_CONNECTED => PeerState::Connected,
            _ENetPeerState_ENET_PEER_STATE_DISCONNECT_LATER => PeerState::DisconnectLater,
            _ENetPeerState_ENET_PEER_STATE_DISCONNECTING => PeerState::Disconnecting,
            _ENetPeerState_ENET_PEER_STATE_ACKNOWLEDGING_DISCONNECT => {
                PeerState::AcknowledgingDisconnect
            }
            _ENetPeerState_ENET_PEER_STATE_ZOMBIE => PeerState::Zombie,
            val => panic!("unexpected peer state: {}", val),
        }
    }
}
