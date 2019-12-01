use std::sync::{Arc, Weak};

use super::super::{
    channel::{ChannelImpl, MethodCallHandler, MethodChannel},
    codec::{standard_codec::CODEC, MethodCodec},
};

use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};


pub struct StandardMethodChannel {
    name: &'static str,
    compositor: FlutterCompositorWeakRef,
    method_handler: Weak<dyn MethodCallHandler + Send + Sync>,
}

impl StandardMethodChannel {
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

impl ChannelImpl for StandardMethodChannel {
    fn name(&self) -> &'static str {
        &self.name
    }

    fn compositor(&self) -> Option<FlutterCompositorRef> {
        self.compositor.upgrade()
    }
}

impl MethodChannel for StandardMethodChannel {
    fn method_handler(&self) -> Option<Arc<dyn MethodCallHandler + Send + Sync>> {
        self.method_handler.upgrade()
    }

    fn codec(&self) -> &'static dyn MethodCodec {
        &CODEC
    }
}

method_channel!(StandardMethodChannel);
