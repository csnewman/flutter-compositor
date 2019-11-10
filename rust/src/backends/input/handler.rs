use std::{
    cell::RefCell,
    process::Command,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use smithay::backend::session::{auto::AutoSession, Session};
use smithay::{
    backend::input::{
        self, Event, InputBackend, InputHandler, KeyState, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
    },
    reexports::wayland_server::protocol::wl_pointer,
    wayland::{
        seat::{AxisFrame, KeyboardHandle, ModifiersState, PointerHandle},
        SERIAL_COUNTER as SCOUNTER,
    },
};

use xkbcommon::xkb;
pub use xkbcommon::xkb::{keysyms, Keysym};

use crate::FlutterCompositorWeakRef;
use log::{debug, error, info, trace, warn};
use smithay::wayland::seat::XkbConfig;

use crate::smithay::backend::input::TouchDownEvent;
use std::borrow::Borrow;

pub struct FlutterInputHandler {
    compositor: FlutterCompositorWeakRef,
}

impl FlutterInputHandler {
    pub fn new(compositor: FlutterCompositorWeakRef) -> Self {
        Self { compositor }
    }
}

impl<B: InputBackend> InputHandler<B> for FlutterInputHandler {
    fn on_seat_created(&mut self, _: &input::Seat) {
        // currently we just create a single static one
    }

    fn on_seat_destroyed(&mut self, _: &input::Seat) {
        // currently we just create a single static one
    }

    fn on_seat_changed(&mut self, _: &input::Seat) {
        // currently we just create a single static one
    }

    fn on_keyboard_key(&mut self, _: &input::Seat, evt: B::KeyboardKeyEvent) {
        let keycode = evt.key_code();
        let state = evt.state();
        //        let time = Event::time(&evt);

        let compositor_ref = self.compositor.upgrade().unwrap();
        let compositor = compositor_ref.get();

        let manager_ref = compositor.backend.input.borrow();
        let manager = manager_ref.as_ref().unwrap();
        manager.key(state, keycode);
    }

    fn on_pointer_move(&mut self, _: &input::Seat, evt: B::PointerMotionEvent) {
        // TODO: Implement mouse support
    }

    fn on_pointer_move_absolute(&mut self, _: &input::Seat, evt: B::PointerMotionAbsoluteEvent) {
        // TODO: Implement mouse support
    }

    fn on_pointer_button(&mut self, _: &input::Seat, evt: B::PointerButtonEvent) {
        // TODO: Implement mouse support
    }

    fn on_pointer_axis(&mut self, _: &input::Seat, evt: B::PointerAxisEvent) {
        // TODO: Implement mouse support
    }

    fn on_touch_down(&mut self, _: &input::Seat, evt: B::TouchDownEvent) {
        info!(
            "TOUCH DOWN {:?} >> {:?}",
            evt.position(),
            evt.position_transformed((1920, 1080))
        );

        // TODO: Implement touch support
    }
    fn on_touch_motion(&mut self, _: &input::Seat, _: B::TouchMotionEvent) {
        // TODO: Implement touch support
    }
    fn on_touch_up(&mut self, _: &input::Seat, _: B::TouchUpEvent) {
        // TODO: Implement touch support
    }
    fn on_touch_cancel(&mut self, _: &input::Seat, _: B::TouchCancelEvent) {
        // TODO: Implement touch support
    }
    fn on_touch_frame(&mut self, _: &input::Seat, _: B::TouchFrameEvent) {
        // TODO: Implement touch support
    }
    fn on_input_config_changed(&mut self, _: &mut B::InputConfig) {
        // TODO: Implement touch support
    }
}
