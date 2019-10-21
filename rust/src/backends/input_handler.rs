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

        // scancode = keycode + 8
        //

        let xkb_config = XkbConfig::default();

        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            &xkb_config.rules,
            &xkb_config.model,
            &xkb_config.layout,
            &xkb_config.variant,
            xkb_config.options,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(())
        .unwrap();
        let xkb_state = xkb::State::new(&keymap);

        let sym = xkb_state.key_get_one_sym(keycode + 8);

        debug!(
            "key scancode={} state={:?} keycode={}",
            keycode + 8,
            state,
            sym
        );

        //
        //        let mods_changed = guard.key_input(keycode, state);

        //        let compositor_ref = self.compositor.upgrade().unwrap();
        //        let compositor = compositor_ref.get();
        //
        //        let serial = SCOUNTER.next_serial();
        //        let time = Event::time(&evt);
        //        //        let mut action = KeyAction::None;
        //        self.keyboard
        //            .input(keycode, state, serial, time, |modifiers, keysym| {
        //                debug!(
        //                    "keysym {:?} {:?} {}",
        //                    state,
        //                    modifiers,
        //                    ::xkbcommon::xkb::keysym_get_name(keysym)
        //                );
        //                action = process_keyboard_shortcut(modifiers, keysym);
        //                // forward to client only if action == KeyAction::Forward
        //                // both for pressed and released, to avoid inconsistencies
        //                if let KeyAction::Forward = action {
        //                    true
        //                } else {
        //                    false
        //                }
        //            });
        //        if let KeyState::Released = state {
        //            // only process special actions on key press, not release
        //            return;
        //        }
        //        match action {
        //            KeyAction::Quit => {
        //                info!("Quitting.");
        //                self.running.store(false, Ordering::SeqCst);
        //            }
        //            KeyAction::VtSwitch(vt) => {
        //                if let Some(ref mut session) = self.session {
        //                    info!("Trying to switch to vt {}", vt);
        //                    if let Err(err) = session.change_vt(vt) {
        //                        error!("Error switching to vt {}: {}", vt, err);
        //                    }
        //                }
        //            }
        //            KeyAction::Run(cmd) => {
        //                info!("Starting program {}", cmd.clone());
        //                if let Err(e) = Command::new(&cmd).spawn() {
        //                    error!("Failed to start program {} {:?}", cmd, e);
        //                }
        //            }
        //            _ => (),
        //        }
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
//
///// Possible results of a keyboard action
//enum KeyAction {
//    /// Quit the compositor
//    Quit,
//    /// Trigger a vt-switch
//    VtSwitch(i32),
//    /// run a command
//    Run(String),
//    /// Forward the key to the client
//    Forward,
//    /// Do nothing more
//    None,
//}
//
//fn process_keyboard_shortcut(modifiers: &ModifiersState, keysym: Keysym) -> KeyAction {
//    if modifiers.ctrl && modifiers.alt && keysym == xkb::KEY_BackSpace
//        || modifiers.logo && keysym == xkb::KEY_q
//        || modifiers.ctrl && keysym == xkb::KEY_q
//    {
//        // ctrl+alt+backspace = quit
//        // logo + q = quit
//        KeyAction::Quit
//    } else if keysym >= xkb::KEY_XF86Switch_VT_1 && keysym <= xkb::KEY_XF86Switch_VT_12 {
//        // VTSwicth
//        KeyAction::VtSwitch((keysym - xkb::KEY_XF86Switch_VT_1 + 1) as i32)
//    } else if modifiers.logo && keysym == xkb::KEY_Return
//        || modifiers.ctrl && keysym == xkb::KEY_Return {
//        // run terminal
//        KeyAction::Run("weston-terminal".into())
//    } else {
//        KeyAction::Forward
//    }
//}
