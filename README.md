# Flutter Compositor
flutter-compositor is an experimental library for embedding flutter for wayland compositor creation. This library allows you to write a wayland compositor using dart and flutter (and a small amount of Rust glue logic), with all the standard libraries you are used to.

## Status
**Experimental**

This library is not ready for use. Currently basic features are being implemented, with some core features not implemented, due to a mixture of time constraints and upstream missing features.

## Features
Only a small number of features are currently implemented.
- [x] Flutter rendering
- [x] logind support
- [x] winit support (used for testing)
- [x] tty-udev support
- [ ] Keyboard Input
- [ ] Pointer Input (mouse)
- [ ] Touch Input
- [ ] Flutter channels
- [ ] Dart FFI
- [ ] SHM support
- [ ] Wayland Shell
- [ ] Xdg Shell
- [ ] Data device support (i.e. clipboard & drag and drop)
- [ ] nVidea support (Blocked by [Smitahy#128](https://github.com/Smithay/smithay/issues/128))
- [ ] Multi-monitor support (Blocked by [Flutter#30701](https://github.com/flutter/flutter/issues/30701))
- [ ] Damage tracking (Blocked by [Flutter#33939](https://github.com/flutter/flutter/issues/33939))
- [ ] Performance
