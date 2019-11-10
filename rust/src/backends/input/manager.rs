use smithay::backend::input::KeyState;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::sync::{mpsc, Arc, RwLock, Weak};
use std::thread;
use std::time::{Duration, Instant};
use wayland_server::protocol::wl_keyboard::WlKeyboard;
use xkbcommon::xkb;

use crate::flutter::channel::{
    BasicMessageChannel, ChannelRegistry, MessageChannel, MessageHandler,
};
use crate::flutter::codec::{json_codec, Value};
use crate::flutter::error::MessageError;
use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};
use log::{debug, error, info};

use crate::backends::input::glfw::{GLFW_KEY_UNKNOWN, GLFW_MAPPING};
use crate::json_value;

pub struct XkbConfig {
    /// The rules file to use.
    ///
    /// The rules file describes how to interpret the values of the model, layout, variant and
    /// options fields.
    pub rules: String,
    /// The keyboard model by which to interpret keycodes and LEDs.
    pub model: String,
    /// A comma separated list of layouts (languages) to include in the keymap.
    pub layout: String,
    /// A comma separated list of variants, one per layout, which may modify or augment the
    /// respective layout in various ways.
    pub variant: String,
    /// A comma separated list of options, through which the user specifies non-layout related
    /// preferences, like which key combinations are used for switching layouts, or which key is the
    /// Compose key.
    pub options: Option<String>,
}

impl Default for XkbConfig {
    fn default() -> Self {
        Self {
            rules: "".to_string(),
            model: "".to_string(),
            layout: "".to_string(),
            variant: "".to_string(),
            options: None,
        }
    }
}

pub struct InputManager {
    compositor: FlutterCompositorWeakRef,
    known_kbds: Vec<WlKeyboard>,
    keymap: xkb::Keymap,
    state: RefCell<xkb::State>,
    ongoing: RefCell<Option<(u32, mpsc::Sender<()>)>>,
    rate: i32,
    delay: i32,

    ignore_handler: Arc<RwLock<IgnoreHandler>>,
    keyevent_channel: Weak<BasicMessageChannel>,
}

pub const KEYEVENT_CHANNEL_NAME: &str = "flutter/keyevent";

impl InputManager {
    pub fn new(xkb_config: XkbConfig, compositor: FlutterCompositorWeakRef) -> InputManager {
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
        let state = xkb::State::new(&keymap);
        InputManager {
            compositor,
            known_kbds: Vec::new(),
            keymap,
            state: RefCell::new(state),
            ongoing: RefCell::new(None),
            rate: 500,
            delay: 1000,
            ignore_handler: Arc::new(RwLock::new(IgnoreHandler)),
            keyevent_channel: Weak::new(),
        }
    }

    pub fn register_channels(&mut self, registry: &ChannelRegistry) {
        let handler = Arc::downgrade(&self.ignore_handler);

        self.keyevent_channel = registry.register_channel(BasicMessageChannel::new(
            KEYEVENT_CHANNEL_NAME,
            handler,
            &json_codec::CODEC,
            self.compositor.clone(),
        ));
    }

    pub fn key(&self, state: KeyState, code: u32) {
        //        let sym = xkb_state.key_get_one_sym(keycode + 8);
        //        debug!(
        //            "key scancode={} state={:?} keycode={}",
        //            keycode + 8,
        //            state,
        //            sym
        //        );

        match state {
            KeyState::Released => {
                // Check if we are repeating a character
                let current_val = self.ongoing.borrow_mut().take();
                if let Some((current_key, sender)) = current_val {
                    // If its a different key, re-enable repeat
                    if current_key != code {
                        self.ongoing.replace(Some((current_key, sender)));
                    }
                }

                // Send event
                self.key_event(state, code);
            }
            KeyState::Pressed => {
                // Check if we are repeating a character
                let current_val = self.ongoing.borrow_mut().take();
                if let Some((current_key, sender)) = current_val {
                    // If its the same key, ignore event, Fixes smithay bug
                    if current_key == code {
                        self.ongoing.replace(Some((current_key, sender)));
                        return;
                    }
                }

                // replace any previously repeating key
                let (sender, receiver) = mpsc::channel();
                self.ongoing.replace(Some((code, sender)));

                // Send event
                self.key_event(state, code);

                // Check if key can repeat
                if !self.keymap.key_repeats(code + 8) {
                    return;
                }

                // Start thread to send repeat events
                let delay = self.delay as u64;
                let rate = self.rate;
                thread::spawn(move || {
                    // Delay
                    thread::sleep(Duration::from_millis(delay));

                    loop {
                        // Drain channel
                        loop {
                            match receiver.try_recv() {
                                Ok(()) => {}
                                Err(mpsc::TryRecvError::Empty) => break,
                                Err(mpsc::TryRecvError::Disconnected) => return,
                            }
                        }

                        // TODO: Send repeat event
                        //                        self.key_event(state, code, Instant::now());

                        // Rate
                        thread::sleep(Duration::from_secs(1) / rate as u32);
                    }
                });
            }
        }
    }

    fn key_event(&self, keystate: KeyState, rawcode: u32) {
        let mut state = self.state.borrow_mut();

        // Offset the rawcode by 8, as the evdev XKB rules reflect X's
        // broken keycode system, which starts at 8.
        let scancode = rawcode + 8;
        let keycode = if rawcode > GLFW_MAPPING.len() as u32 {
            GLFW_KEY_UNKNOWN
        } else {
            GLFW_MAPPING[rawcode as usize]
        };
        //        let keycode = state.key_get_one_sym(scancode);

        debug!(
            "key event scancode={} state={:?} keycode={}",
            scancode, keystate, keycode,
        );

        let direction = match keystate {
            KeyState::Pressed => xkb::KeyDirection::Down,
            KeyState::Released => xkb::KeyDirection::Up,
        };

        // Handle state
        state.update_key(scancode, direction);

        // TODO: Convert to glfw mods
        let mods = state.serialize_mods(xkb::STATE_MODS_EFFECTIVE);

        if let Some(channel) = self.keyevent_channel.upgrade() {
            let json = json_value!({
                "toolkit": "glfw",
                "keyCode": keycode as i32,
                "scanCode": scancode as i32,
                "modifiers": mods as i32,
                "keymap": "linux",
                "type": if keystate ==  KeyState::Released { "keyup" } else { "keydown" }
            });
            channel.send(&json);
        }
    }
}

struct IgnoreHandler;

impl MessageHandler for IgnoreHandler {
    fn on_message(&mut self, _: Value, _: FlutterCompositorRef) -> Result<Value, MessageError> {
        Ok(Value::Null)
    }
}
