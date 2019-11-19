//! Plugins use MethodChannel to interop with flutter/dart.
//! It contains two implementations StandardMethodChannel using binary encoding
//! and JsonMethodChannel using json encoding.

pub use self::{
    basic_message_channel::BasicMessageChannel, event_channel::EventChannel,
    json_method_channel::JsonMethodChannel, registry::ChannelRegistry,
    standard_method_channel::StandardMethodChannel,
};
use super::{
    codec::{MessageCodec, MethodCall, MethodCallResult, MethodCodec, Value},
    error::{MessageError, MethodCallError},
    ffi::{PlatformMessage, PlatformMessageResponseHandle},
};

use std::{
    borrow::Cow,
    sync::{Arc, RwLock, Weak},
};

use crate::FlutterCompositorRef;
use log::{error, trace};
use std::borrow::Borrow;
use tokio::prelude::Future;

use log::{debug, info};

#[macro_use]
mod macros;

mod basic_message_channel;
mod event_channel;
mod json_method_channel;
mod registry;
mod standard_method_channel;

trait ChannelImpl {
    fn name(&self) -> &'static str;
    fn compositor(&self) -> Option<FlutterCompositorRef>;
}

pub trait Channel {
    fn name(&self) -> &'static str;
    fn compositor(&self) -> Option<FlutterCompositorRef>;
    fn handle_platform_message(&self, msg: PlatformMessage);
    fn try_as_method_channel(&self) -> Option<&dyn MethodChannel>;
    fn try_as_message_channel(&self) -> Option<&dyn MessageChannel>;

    /// When flutter call a method using MethodChannel,
    /// it can wait for rust response using await syntax.
    /// This method send a response to flutter. This is a low level method.
    fn send_response(&self, response_handle: PlatformMessageResponseHandle, buf: &[u8]) {
        if let Some(compositor_ref) = self.compositor() {
            compositor_ref
                .get()
                .engine
                .send_platform_message_response(response_handle, buf);
        } else {
            error!("Channel {} was not initialized", self.name());
        }
    }

    /// Send a platform message over this channel. This is a low level method.
    fn send_platform_message(&self, message: PlatformMessage) {
        if let Some(compositor_ref) = self.compositor() {
            compositor_ref.get().engine.send_platform_message(message);
        } else {
            error!("Channel {} was not initialized", self.name());
        }
    }

    /// Send a buffer. This is a low level method.
    fn send_buffer(&self, buf: &[u8]) {
        self.send_platform_message(PlatformMessage {
            channel: Cow::Borrowed(self.name()),
            message: &buf,
            response_handle: None,
        });
    }
}

pub trait MethodChannel: Channel {
    fn method_handler(&self) -> Option<Arc<dyn MethodCallHandler + Send + Sync>>;
    fn codec(&self) -> &'static dyn MethodCodec;

    /// Handle incoming message received on this channel
    fn handle_platform_message(&self, mut msg: PlatformMessage) {
        debug_assert_eq!(msg.channel, self.name());
        if let Some(handler) = self.method_handler() {
            if let Some(compositor_ref) = self.compositor() {
                let call = self.codec().decode_method_call(msg.message).unwrap();
                let channel = self.name().to_string();
                trace!(
                    "on channel {}, got method call {} with args {:?}",
                    channel,
                    call.method,
                    call.args
                );
                let mut response_handle = msg.response_handle.take();
                let codec = self.codec();

                compositor_ref.clone().get().task_executor.spawn(
                    tokio::prelude::future::ok(()).map(move |_| {
                        let method = call.method.clone();

                        let result = handler.on_method_call(call, compositor_ref.clone());
                        let response = match result {
                            Ok(value) => MethodCallResult::Ok(value),
                            Err(error) => {
                                error!("error in method call {}#{}: {}", channel, method, error);
                                error.into()
                            }
                        };

                        compositor_ref.get().main_thread_sender.send(
                            crate::MainThreadCallback::ChannelFn((
                                channel,
                                Box::new(move |channel| {
                                    let buf = codec.encode_method_call_response(&response);
                                    if let Some(handle) = response_handle.take() {
                                        channel.send_response(handle, &buf);
                                    }
                                }),
                            )),
                        );
                    }),
                );
            }
        }
    }

    /// Invoke a flutter method using this channel
    fn invoke_method(&self, method_call: MethodCall) {
        let buf = self.codec().encode_method_call(&method_call);
        self.send_buffer(&buf);
    }

    /// When flutter listen to a stream of events using EventChannel.
    /// This method send back a success event.
    /// It can be call multiple times to simulate stream.
    fn send_success_event(&self, data: &Value) {
        let buf = self.codec().encode_success_envelope(data);
        self.send_buffer(&buf);
    }

    /// When flutter listen to a stream of events using EventChannel.
    /// This method send back a error event.
    /// It can be call multiple times to simulate stream.
    fn send_error_event(&self, code: &str, message: &str, data: &Value) {
        let buf = self.codec().encode_error_envelope(code, message, data);
        self.send_buffer(&buf);
    }
}

pub trait MessageChannel: Channel {
    fn message_handler(&self) -> Option<Arc<RwLock<dyn MessageHandler + Send + Sync>>>;
    fn codec(&self) -> &'static dyn MessageCodec;

    /// Handle incoming message received on this channel
    fn handle_platform_message(&self, mut msg: PlatformMessage) {
        debug_assert_eq!(msg.channel, self.name());
        if let Some(handler) = self.message_handler() {
            if let Some(compositor_ref) = self.compositor() {
                let message = self.codec().decode_message(msg.message).unwrap();
                let channel = self.name().to_string();
                trace!("on channel {}, got message {:?}", channel, message);
                let mut response_handle = msg.response_handle.take();
                let codec = self.codec();

                compositor_ref.clone().get().task_executor.spawn(
                    tokio::prelude::future::ok(()).map(move |_| {
                        let mut handler = handler.write().unwrap();
                        let result = handler.on_message(message, compositor_ref.clone());
                        let response = match result {
                            Ok(value) => value,
                            Err(error) => {
                                error!(
                                    "error in message handler on channel {}: {}",
                                    channel, error
                                );
                                Value::Null
                            }
                        };
                        if response_handle.is_some() {
                            compositor_ref.get().main_thread_sender.send(
                                crate::MainThreadCallback::ChannelFn((
                                    channel,
                                    Box::new(move |channel| {
                                        let buf = codec.encode_message(&response);
                                        if let Some(handle) = response_handle.take() {
                                            channel.send_response(handle, &buf);
                                        }
                                    }),
                                )),
                            );
                        }
                    }),
                );
            }
        }
    }

    /// Send a plain value on this channel.
    fn send(&self, value: &Value) {
        let buf = self.codec().encode_message(value);
        self.send_buffer(&buf);
    }
}

pub trait MessageHandler {
    fn on_message(
        &mut self,
        msg: Value,
        compositor: FlutterCompositorRef,
    ) -> Result<Value, MessageError>;
}

pub trait MethodCallHandler {
    fn on_method_call(
        &self,
        call: MethodCall,
        compositor: FlutterCompositorRef,
    ) -> Result<Value, MethodCallError>;
}

pub trait EventHandler {
    fn on_listen(
        &self,
        args: Value,
        compositor: FlutterCompositorRef,
    ) -> Result<Value, MethodCallError>;
    fn on_cancel(&self, compositor: FlutterCompositorRef) -> Result<Value, MethodCallError>;
}
