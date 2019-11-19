use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::flutter::channel::{
    BasicMessageChannel, ChannelRegistry, JsonMethodChannel, MessageHandler, MethodCallHandler,
    MethodChannel,
};
use crate::flutter::codec::{json_codec, MethodCall};
use crate::flutter::error::{MessageError, MethodCallError};
use crate::flutter::{
    codec::{value::from_value, Value},
    utils::{OwnedStringUtils, StringUtils},
};
use crate::{FlutterCompositorRef, FlutterCompositorWeakRef};
use std::sync::{Arc, RwLock, Weak};

use log::{debug, error, info};

const TEXTINPUT_CHANNEL_NAME: &str = "flutter/textinput";

pub struct TextInputManager {
    compositor: FlutterCompositorWeakRef,
    channel: Weak<JsonMethodChannel>,
    handler: Arc<TextInputHandler>,
    client_id: Option<i64>,
    pub editing_state: Option<TextEditingState>,
}

impl TextInputManager {
    pub fn new() -> Self {
        Self {
            compositor: Default::default(),
            channel: Weak::new(),
            handler: Arc::new(TextInputHandler),
            client_id: None,
            editing_state: None,
        }
    }

    pub fn set_compositor(&mut self, compositor: FlutterCompositorWeakRef) {
        self.compositor = compositor;
    }

    pub fn register_channels(&mut self, registry: &ChannelRegistry) {
        let handler = Arc::downgrade(&self.handler);

        self.channel = registry.register_channel(JsonMethodChannel::new(
            TEXTINPUT_CHANNEL_NAME,
            handler,
            self.compositor.clone(),
        ));
    }

    pub fn with_state(&mut self, cbk: impl FnOnce(&mut TextEditingState)) {
        if let Some(state) = &mut self.editing_state {
            cbk(state);
        }
    }

    pub fn notify_changes(&mut self) {
        let client_id = self.client_id;
        if let Some(state) = &mut self.editing_state {
            if let Some(channel) = self.channel.upgrade() {
                channel.invoke_method(MethodCall {
                    method: String::from("TextInputClient.updateEditingState"),
                    args: json_value!([client_id, state]),
                });
            }
        };
    }
}

struct TextInputHandler;

impl MethodCallHandler for TextInputHandler {
    fn on_method_call(
        &self,
        call: MethodCall,
        compositor: FlutterCompositorRef,
    ) -> Result<Value, MethodCallError> {
        let compositor = compositor.get();
        let mut textinput = compositor.engine.text_input.borrow_mut();

        match call.method.as_str() {
            "TextInput.setClient" => {
                let args: SetClientArgs = from_value(&call.args)?;
                textinput.client_id = Some(args.0);
                Ok(Value::Null)
            }
            "TextInput.clearClient" => {
                textinput.client_id = None;
                textinput.editing_state.take();
                Ok(Value::Null)
            }
            "TextInput.setEditingState" => {
                let state: TextEditingState = from_value(&call.args)?;
                textinput.editing_state.replace(state);
                Ok(Value::Null)
            }
            "TextInput.show" => Ok(Value::Null),
            "TextInput.hide" => Ok(Value::Null),
            _ => Err(MethodCallError::NotImplemented),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SetClientArgs(i64, SetClientArgsText);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetClientArgsText {
    autocorrect: bool,
    input_action: String,
    obscure_text: bool,
    keyboard_appearance: String,
    action_label: Option<String>,
    text_capitalization: String,
    input_type: SetClientArgsInputType,
}

#[derive(Serialize, Deserialize)]
struct SetClientArgsInputType {
    signed: Option<bool>,
    name: String,
    decimal: Option<bool>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TextEditingState {
    composing_base: i64,
    composing_extent: i64,
    selection_affinity: String,
    selection_base: i64,
    selection_extent: i64,
    selection_is_directional: bool,
    text: String,
}

enum Direction {
    Left,
    Right,
}

impl TextEditingState {
    pub fn from(v: Value) -> Option<Self> {
        from_value(&v).ok()
    }

    fn get_selection_range(&self) -> Range<usize> {
        if self.selection_base <= self.selection_extent {
            self.selection_base as usize..self.selection_extent as usize
        } else {
            self.selection_extent as usize..self.selection_base as usize
        }
    }

    pub fn move_to(&mut self, p: usize) {
        self.selection_base = p as i64;
        self.selection_extent = self.selection_base;
        self.selection_is_directional = false;
    }

    pub fn select_to(&mut self, p: usize) {
        self.selection_extent = p as i64;
        self.selection_is_directional = true;
    }

    fn select_or_move_to(&mut self, p: usize, select: bool) {
        if select {
            self.select_to(p)
        } else {
            self.move_to(p)
        }
    }

    pub fn select_all(&mut self) {
        self.selection_base = 0;
        self.move_to_end(true);
    }

    pub fn delete_selected(&mut self) -> bool {
        let range = self.get_selection_range();
        if range.start != range.end {
            self.move_to(range.start);
            self.text.remove_chars(range);
            true
        } else {
            false
        }
    }

    pub fn add_characters(&mut self, c: &str) {
        self.delete_selected();
        let index = self
            .text
            .byte_index_of_char(self.selection_extent as usize)
            .unwrap_or_else(|| self.text.len());
        self.text.insert_str(index, c);
        self.move_to(self.selection_extent as usize + c.char_count());
    }

    pub fn backspace(&mut self) {
        if !self.delete_selected() && self.selection_base > 0 {
            if let Some(index) = self
                .text
                .byte_index_of_char(self.selection_base as usize - 1)
            {
                self.text.remove(index);
                self.move_to(self.selection_base as usize - 1);
            }
        }
    }

    pub fn delete(&mut self) {
        if !self.delete_selected() && (self.selection_base as usize) < self.text.char_count() {
            if let Some(index) = self.text.byte_index_of_char(self.selection_base as usize) {
                self.text.remove(index);
            }
        }
    }

    pub fn move_left(&mut self, by_word: bool, select: bool) {
        let selection = self.get_selection_range();

        let current_pos = if select {
            self.selection_extent as usize
        } else if self.selection_base != self.selection_extent {
            selection.start + 1
        } else {
            selection.start
        };
        let next_pos = if by_word {
            self.get_next_word_boundary(current_pos, Direction::Left)
        } else {
            (current_pos as i64 - 1).max(0) as usize
        };
        self.select_or_move_to(next_pos, select);
    }

    pub fn move_right(&mut self, by_word: bool, select: bool) {
        let selection = self.get_selection_range();

        let current_pos = if select {
            self.selection_extent as usize
        } else if self.selection_base != self.selection_extent {
            selection.end - 1
        } else {
            selection.end
        };
        let next_pos = if by_word {
            self.get_next_word_boundary(current_pos, Direction::Right)
        } else {
            (current_pos + 1).min(self.text.char_count())
        };
        self.select_or_move_to(next_pos, select);
    }

    pub fn move_to_beginning(&mut self, select: bool) {
        self.select_or_move_to(0, select);
    }

    pub fn move_to_end(&mut self, select: bool) {
        self.select_or_move_to(self.text.char_count(), select);
    }
    pub fn move_up(&mut self, select: bool) {
        let selection = self.get_selection_range();

        let p = self.get_next_line_pos(selection.start, false);
        self.select_or_move_to(p, select);
    }
    pub fn move_down(&mut self, select: bool) {
        let selection = self.get_selection_range();

        let p = self.get_next_line_pos(selection.end, true);
        self.select_or_move_to(p, select);
    }

    pub fn get_selected_text(&self) -> &str {
        if let Some(range) = self.text.byte_range_of_chars(self.get_selection_range()) {
            &self.text[range]
        } else {
            ""
        }
    }

    /// Naive implementation, since rust does not know font metrics.
    /// It's hard to predict column position when caret jumps across lines.
    /// Official android implementation does not have a solution so far:
    /// https://github.com/flutter/engine/blob/395937380c26c7f7e3e0d781d111667daad2c47d/shell/platform/android/io/flutter/plugin/editing/InputConnectionAdaptor.java
    fn get_next_line_pos(&self, start: usize, forward: bool) -> usize {
        let v: Vec<char> = self.text.chars().collect();
        if forward {
            // search forward
            let max = self.text.char_count();
            if start >= max {
                return max;
            }
            let s = &v[start + 1..];
            s.iter().position(|&c| c == '\n').map_or(max, |n| {
                // end of line pos
                start + n + 1
            })
        } else {
            // search backward
            if start < 1 {
                return 0;
            }
            let s = &v[..start - 1];
            let len = s.iter().count();
            s.iter().rposition(|&c| c == '\n').map_or(0, |n| {
                // start of line pos
                start - len + n
            })
        }
    }

    fn get_next_word_boundary(&self, start: usize, direction: Direction) -> usize {
        match direction {
            Direction::Right => {
                let max = self.text.char_count();
                if start >= max {
                    return max;
                }
                let start = start + 1;
                self.text
                    .chars()
                    .skip(start)
                    .position(|c| !c.is_alphanumeric())
                    .map_or(max, |n| start + n)
            }
            Direction::Left => {
                if start == 0 {
                    return 0;
                }
                let len = self.text.char_count();
                let start = start - 1;
                self.text
                    .chars()
                    .rev()
                    .skip(len - start)
                    .position(|c| !c.is_alphanumeric())
                    .map_or(0, |n| start - n)
            }
        }
    }
}
