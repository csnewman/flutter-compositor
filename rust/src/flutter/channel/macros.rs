macro_rules! method_channel {
    ($channel:ty) => {
        impl $crate::flutter::channel::Channel for $channel {
            fn name(&self) -> &'static str {
                ChannelImpl::name(self)
            }

            fn compositor(&self) -> Option<FlutterCompositorRef> {
                ChannelImpl::compositor(self)
            }

            fn handle_platform_message(&self, msg: $crate::flutter::ffi::PlatformMessage) {
                $crate::flutter::channel::MethodChannel::handle_platform_message(self, msg)
            }

            fn try_as_method_channel(
                &self,
            ) -> Option<&dyn $crate::flutter::channel::MethodChannel> {
                Some(self)
            }

            fn try_as_message_channel(
                &self,
            ) -> Option<&dyn $crate::flutter::channel::MessageChannel> {
                None
            }
        }
    };
}

macro_rules! message_channel {
    ($channel:ty) => {
        impl $crate::flutter::channel::Channel for $channel {
            fn name(&self) -> &'static str {
                ChannelImpl::name(self)
            }

            fn compositor(&self) -> Option<FlutterCompositorRef> {
                ChannelImpl::compositor(self)
            }

            fn handle_platform_message(&self, msg: $crate::flutter::ffi::PlatformMessage) {
                $crate::flutter::channel::MessageChannel::handle_platform_message(self, msg)
            }

            fn try_as_method_channel(
                &self,
            ) -> Option<&dyn $crate::flutter::channel::MethodChannel> {
                None
            }

            fn try_as_message_channel(
                &self,
            ) -> Option<&dyn $crate::flutter::channel::MessageChannel> {
                Some(self)
            }
        }
    };
}
