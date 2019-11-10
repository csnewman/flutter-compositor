use log::debug;

use crate::FlutterCompositorWeakRef;
use smithay::wayland::compositor::roles::Role;
use smithay::wayland::compositor::CompositorToken;
use smithay::wayland::seat::CursorImageRole;

use wayland_server::{
    protocol::{wl_seat, wl_surface},
    Display, Global, NewResource,
};

pub struct FlutterSeat {
    compositor: FlutterCompositorWeakRef,
    name: String,
    known_seats: Vec<wl_seat::WlSeat>,
}

impl FlutterSeat {
    pub fn new(compositor: FlutterCompositorWeakRef, name: String) -> Self {
        Self {
            compositor,
            name,
            known_seats: vec![],
        }
    }

    pub fn create<R>(&self, display: &mut Display, token: CompositorToken<R>)
    where
        R: Role<CursorImageRole> + 'static,
    {
        let compositor_weak = self.compositor.clone();
        display.create_global(5, move |new_seat, _version| {
            let seat = implement_seat(compositor_weak.clone(), new_seat, token.clone());

            let compositor_ref = compositor_weak.upgrade().unwrap();
            let compositor = compositor_ref.get();

            let mut seat_ref = compositor.backend.seat.borrow_mut();
            let mut flutter_seat = seat_ref.as_mut().unwrap();

            if seat.as_ref().version() >= 2 {
                seat.name(flutter_seat.name.clone());
            }

            // TODO: Support touch
            let mut caps = wl_seat::Capability::empty();
            caps |= wl_seat::Capability::Pointer;
            caps |= wl_seat::Capability::Keyboard;
            seat.capabilities(caps);

            flutter_seat.known_seats.push(seat);
        });
    }
}

fn implement_seat<R>(
    compositor: FlutterCompositorWeakRef,
    new_seat: NewResource<wl_seat::WlSeat>,
    token: CompositorToken<R>,
) -> wl_seat::WlSeat
where
    R: Role<CursorImageRole> + 'static,
{
    let dest_comp = compositor.clone();
    new_seat.implement_closure(
        move |request, seat| {
            let compositor_weak = seat
                .as_ref()
                .user_data::<FlutterCompositorWeakRef>()
                .unwrap();
            //            let inner = arc.inner.borrow_mut();
            match request {
                wl_seat::Request::GetPointer { id } => {
                    debug!("GetPointer");
                    // let pointer = self::pointer::implement_pointer(id, inner.pointer.as_ref(), token.clone());
                    // if let Some(ref ptr_handle) = inner.pointer {
                    //     ptr_handle.new_pointer(pointer);
                    // }
                }
                wl_seat::Request::GetKeyboard { id } => {
                    debug!("GetKeyboard");

                    // let keyboard = self::keyboard::implement_keyboard(id, inner.keyboard.as_ref());
                    // if let Some(ref kbd_handle) = inner.keyboard {
                    //     kbd_handle.new_kbd(keyboard);
                    // }
                }
                wl_seat::Request::GetTouch { id: _ } => {
                    // TODO: Support touch
                }
                wl_seat::Request::Release => {
                    // Our destructors already handle it
                }
                _ => unreachable!(),
            }
        },
        Some(move |seat: wl_seat::WlSeat| {
            let compositor_ref = dest_comp.upgrade().unwrap();
            let compositor = compositor_ref.get();

            let mut seat_ref = compositor.backend.seat.borrow_mut();
            let mut flutter_seat = seat_ref.as_mut().unwrap();
            flutter_seat
                .known_seats
                .retain(|s| !s.as_ref().equals(&seat.as_ref()));
        }),
        compositor,
    )
}
