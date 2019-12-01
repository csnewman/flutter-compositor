

use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::Rectangle,
    wayland::{
        compositor::{
            roles::Role, CompositorToken, SubsurfaceRole,
        },
        shell::{
            legacy::{ShellSurface, ShellSurfaceRole},
            xdg::{ToplevelSurface, XdgSurfaceRole},
        },
    },
};

pub enum Kind<R> {
    Xdg(ToplevelSurface<R>),
    Wl(ShellSurface<R>),
}

impl<R> Kind<R>
where
    R: Role<SubsurfaceRole> + Role<XdgSurfaceRole> + Role<ShellSurfaceRole> + 'static,
{
    pub fn alive(&self) -> bool {
        match *self {
            Kind::Xdg(ref t) => t.alive(),
            Kind::Wl(ref t) => t.alive(),
        }
    }
    pub fn get_surface(&self) -> Option<&wl_surface::WlSurface> {
        match *self {
            Kind::Xdg(ref t) => t.get_surface(),
            Kind::Wl(ref t) => t.get_surface(),
        }
    }
}

struct Window<R> {
    location: (i32, i32),
    surface: Rectangle,
    toplevel: Kind<R>,
}

pub struct WindowMap<R> {
    ctoken: CompositorToken<R>,
    windows: Vec<Window<R>>,
}

impl<R> WindowMap<R>
where
    R: Role<SubsurfaceRole> + Role<XdgSurfaceRole> + Role<ShellSurfaceRole> + 'static,
{
    pub fn new(ctoken: CompositorToken<R>) -> WindowMap<R> {
        WindowMap {
            ctoken,
            windows: Vec::new(),
        }
    }

    pub fn insert(&mut self, toplevel: Kind<R>, location: (i32, i32)) {
        let window = Window {
            location,
            surface: Rectangle {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            toplevel,
        };
        self.windows.insert(0, window);
    }

    pub fn with_windows_from_bottom_to_top<Func>(&self, mut f: Func)
    where
        Func: FnMut(&Kind<R>, (i32, i32)),
    {
        for w in self.windows.iter().rev() {
            f(&w.toplevel, w.location)
        }
    }

    pub fn refresh(&mut self) {
        self.windows.retain(|w| w.toplevel.alive());
    }

    pub fn clear(&mut self) {
        self.windows.clear();
    }
}
