use std::sync::{Arc, RwLock, Weak};

use super::super::{
    channel::{ChannelImpl, MethodCallHandler, MethodChannel},
    codec::{json_codec::CODEC, MethodCodec},
};

use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};
use log::error;

pub struct JsonMethodChannel {
    name: &'static str,
    compositor: FlutterCompositorWeakRef,
    method_handler: Weak<dyn MethodCallHandler + Send + Sync>,
}

impl JsonMethodChannel {
    pub fn new(
        name: &'static str,
        method_handler: Weak<dyn MethodCallHandler + Send + Sync>,
        compositor: FlutterCompositorWeakRef,
    ) -> Self {
        Self {
            name,
            compositor,
            method_handler,
        }
    }

    pub fn set_handler(&mut self, method_handler: Weak<dyn MethodCallHandler + Send + Sync>) {
        self.method_handler = method_handler;
    }
}

impl ChannelImpl for JsonMethodChannel {
    fn name(&self) -> &'static str {
        &self.name
    }

    fn compositor(&self) -> Option<FlutterCompositorRef> {
        self.compositor.upgrade()
    }
}

impl MethodChannel for JsonMethodChannel {
    fn method_handler(&self) -> Option<Arc<dyn MethodCallHandler + Send + Sync>> {
        self.method_handler.upgrade()
    }

    fn codec(&self) -> &'static dyn MethodCodec {
        &CODEC
    }
}

method_channel!(JsonMethodChannel);
