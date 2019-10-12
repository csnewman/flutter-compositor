use super::super::ffi::PlatformMessage;
use super::Channel;

use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Weak},
};

use crate::FlutterCompositorWeakRef;
use log::{info, trace, warn};
use std::cell::RefCell;

pub struct ChannelRegistry {
    compositor: RefCell<FlutterCompositorWeakRef>,
    channels: HashMap<String, Arc<dyn Channel>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            compositor: RefCell::new(Default::default()),
            channels: HashMap::new(),
        }
    }

    pub fn set_compositor(&self, compositor: FlutterCompositorWeakRef) {
        self.compositor.replace(compositor);
    }

    pub fn with_channel<F>(&self, channel_name: &'static str, mut f: F)
    where
        F: FnMut(&Channel),
    {
        if let Some(channel) = self.channels.get(channel_name) {
            f(&**channel);
        }
    }

    pub fn handle(&mut self, mut message: PlatformMessage) {
        if let Some(channel) = self.channels.get(message.channel.deref()) {
            trace!("Processing message from channel: {}", message.channel);
            channel.handle_platform_message(message);
        } else {
            warn!(
                "No plugin registered to handle messages from channel: {}",
                &message.channel
            );
            if let Some(handle) = message.response_handle.take() {
                let compositor_ref = self.compositor.borrow().upgrade().unwrap();
                //                compositor_ref.get().engine.borrow().as_ref().unwrap().send_platform_message_response(handle, &[]);
            }
        }
    }

    pub fn register_channel<C>(&mut self, mut channel: C) -> Weak<C>
    where
        C: Channel + 'static,
    {
        let name = channel.name().to_owned();
        let arc = Arc::new(channel);
        let weak = Arc::downgrade(&arc);
        self.channels.insert(name, arc);
        weak
    }
}
