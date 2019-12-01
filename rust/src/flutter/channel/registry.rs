use super::super::ffi::PlatformMessage;
use super::Channel;

use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Weak},
};

use crate::FlutterCompositorWeakRef;
use log::{trace, warn};
use std::cell::RefCell;

pub struct ChannelRegistry {
    compositor: RefCell<FlutterCompositorWeakRef>,
    channels: RefCell<HashMap<String, Arc<dyn Channel>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            compositor: RefCell::new(Default::default()),
            channels: RefCell::new(HashMap::new()),
        }
    }

    pub fn set_compositor(&self, compositor: FlutterCompositorWeakRef) {
        self.compositor.replace(compositor);
    }

    pub fn with_channel<F>(&self, channel_name: &str, mut f: F)
    where
        F: FnMut(&dyn Channel),
    {
        if let Some(channel) = self.channels.borrow().get(channel_name) {
            f(&**channel);
        }
    }

    pub fn handle(&self, mut message: PlatformMessage) {
        if let Some(channel) = self.channels.borrow().get(message.channel.deref()) {
            trace!("Processing message from channel: {}", message.channel);
            channel.handle_platform_message(message);
        } else {
            warn!(
                "No plugin registered to handle messages from channel: {}",
                &message.channel
            );
            if let Some(handle) = message.response_handle.take() {
                let compositor_ref = self.compositor.borrow().upgrade().unwrap();
                let compositor = compositor_ref.get();
                compositor
                    .engine
                    .send_platform_message_response(handle, &[]);
            }
        }
    }

    pub fn register_channel<C>(&self, channel: C) -> Weak<C>
    where
        C: Channel + 'static,
    {
        let name = channel.name().to_owned();
        let arc = Arc::new(channel);
        let weak = Arc::downgrade(&arc);
        self.channels.borrow_mut().insert(name, arc);
        weak
    }
}
