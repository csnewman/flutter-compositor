use std::sync::{Arc, Weak};

use super::super::{
    channel::{ChannelImpl, EventHandler, MethodCallHandler, MethodChannel},
    codec::{standard_codec::CODEC, MethodCall, MethodCodec, Value},
    error::MethodCallError,
};

use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};
use log::error;

pub struct EventChannel {
    name: &'static str,
    compositor: FlutterCompositorWeakRef,
    method_handler: Arc<dyn MethodCallHandler + Send + Sync>,
}

struct EventChannelMethodCallHandler {
    event_handler: Weak<dyn EventHandler + Send + Sync>,
}

impl EventChannel {
    pub fn new(
        name: &'static str,
        handler: Weak<dyn EventHandler + Send + Sync>,
        compositor: FlutterCompositorWeakRef,
    ) -> Self {
        Self {
            name,
            compositor,
            method_handler: Arc::new(EventChannelMethodCallHandler::new(handler)),
        }
    }
}

impl ChannelImpl for EventChannel {
    fn name(&self) -> &'static str {
        &self.name
    }

    fn compositor(&self) -> Option<FlutterCompositorRef> {
        self.compositor.upgrade()
    }
}

impl MethodChannel for EventChannel {
    fn method_handler(&self) -> Option<Arc<dyn MethodCallHandler + Send + Sync>> {
        Some(Arc::clone(&self.method_handler))
    }

    fn codec(&self) -> &'static dyn MethodCodec {
        &CODEC
    }
}

impl EventChannelMethodCallHandler {
    pub fn new(handler: Weak<dyn EventHandler + Send + Sync>) -> Self {
        Self {
            event_handler: handler,
        }
    }
}

impl MethodCallHandler for EventChannelMethodCallHandler {
    fn on_method_call(
        &self,
        call: MethodCall,
        compositor: FlutterCompositorRef,
    ) -> Result<Value, MethodCallError> {
        if let Some(handler) = self.event_handler.upgrade() {
            //            handler.on_listen()
            //            let mut handler = handler.write().unwrap();
            match call.method.as_str() {
                "listen" => handler.on_listen(call.args, compositor),
                "cancel" => handler.on_cancel(compositor),
                _ => Err(MethodCallError::NotImplemented),
            }
        } else {
            Err(MethodCallError::ChannelClosed)
        }
    }
}

method_channel!(EventChannel);
