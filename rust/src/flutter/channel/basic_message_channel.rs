use std::sync::{Arc, RwLock, Weak};

use super::super::{
    channel::{ChannelImpl, MessageChannel, MessageHandler},
    codec::MessageCodec,
};

use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};


pub struct BasicMessageChannel {
    name: &'static str,
    compositor: FlutterCompositorWeakRef,
    message_handler: Weak<RwLock<dyn MessageHandler + Send + Sync>>,
    codec: &'static dyn MessageCodec,
}

impl BasicMessageChannel {
    pub fn new(
        name: &'static str,
        message_handler: Weak<RwLock<dyn MessageHandler + Send + Sync>>,
        codec: &'static dyn MessageCodec,
        compositor: FlutterCompositorWeakRef,
    ) -> Self {
        Self {
            name,
            compositor,
            message_handler,
            codec,
        }
    }

    pub fn set_handler(&mut self, message_handler: Weak<RwLock<dyn MessageHandler + Send + Sync>>) {
        self.message_handler = message_handler;
    }
}

impl ChannelImpl for BasicMessageChannel {
    fn name(&self) -> &'static str {
        &self.name
    }

    fn compositor(&self) -> Option<FlutterCompositorRef> {
        self.compositor.upgrade()
    }
}

impl MessageChannel for BasicMessageChannel {
    fn message_handler(&self) -> Option<Arc<RwLock<dyn MessageHandler + Send + Sync>>> {
        self.message_handler.upgrade()
    }

    fn codec(&self) -> &'static dyn MessageCodec {
        self.codec
    }
}

message_channel!(BasicMessageChannel);
