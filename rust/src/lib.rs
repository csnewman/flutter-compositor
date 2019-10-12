#[macro_use(define_roles)]
extern crate smithay;

extern crate parking_lot;

extern crate rand;

use log::{debug, error, info};

use wayland_server::calloop::EventLoop;
use wayland_server::Display;

pub mod backends;
mod flutter;

mod renderer;

mod window_map;

mod shell;

pub mod input_handler;

use crate::backends::CompositorBackend;
use crate::flutter::FlutterEngine;
use backends::udev;
use backends::winit;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use tokio::runtime::{Runtime, TaskExecutor};

pub struct FlutterCompositorRef {
    inner: Arc<ReentrantMutex<FlutterCompositor>>,
}

impl FlutterCompositorRef {
    fn get(&self) -> ReentrantMutexGuard<FlutterCompositor> {
        self.inner.lock()
    }

    pub(crate) unsafe fn get_from_mutex_ptr(
        ptr: *const ReentrantMutex<FlutterCompositor>,
    ) -> ReentrantMutexGuard<'static, FlutterCompositor> {
        ptr.as_ref().unwrap().lock()
    }

    pub(crate) fn to_mutex_ptr(self) -> *const ReentrantMutex<FlutterCompositor> {
        Arc::into_raw(self.inner)
    }

    pub fn downgrade(&self) -> FlutterCompositorWeakRef {
        FlutterCompositorWeakRef {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl Clone for FlutterCompositorRef {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

unsafe impl Send for FlutterCompositorRef {}
unsafe impl Sync for FlutterCompositorRef {}

pub struct FlutterCompositorWeakRef {
    inner: Weak<ReentrantMutex<FlutterCompositor>>,
}

impl FlutterCompositorWeakRef {
    pub fn upgrade(&self) -> Option<FlutterCompositorRef> {
        match self.inner.upgrade() {
            None => None,
            Some(val) => Some(FlutterCompositorRef { inner: val }),
        }
    }
}

impl Default for FlutterCompositorWeakRef {
    fn default() -> Self {
        Self { inner: Weak::new() }
    }
}

impl Clone for FlutterCompositorWeakRef {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

unsafe impl Send for FlutterCompositorWeakRef {}
unsafe impl Sync for FlutterCompositorWeakRef {}

pub struct FlutterCompositor {
    backend: CompositorBackend,
    runtime: Runtime,
    task_executor: TaskExecutor,
    engine: FlutterEngine,
}

impl FlutterCompositor {
    pub fn new(backend: CompositorBackend) -> FlutterCompositorRef {
        let runtime = Runtime::new().unwrap();

        let compositor_ref = FlutterCompositorRef {
            inner: Arc::new(ReentrantMutex::new(Self {
                backend,
                task_executor: runtime.executor(),
                runtime,
                engine: FlutterEngine::new(),
            })),
        };

        {
            let weak = compositor_ref.downgrade();
            let compositor = compositor_ref.get();
            compositor.engine.set_compositor(weak);
        }

        compositor_ref
    }
}

/*

new compositor




    backend: Box<GLGraphicsBackend>,
    egl_display: Rc<RefCell<Option<EGLDisplay>>>,
    display: egl_util::WrappedDisplay,
    resource_context: egl_util::WrappedContext,

    wm_context: egl_util::WrappedContext,



backend: Box<GLGraphicsBackend>, egl_display: Rc<RefCell<Option<EGLDisplay>>>

let dims = backend.get_framebuffer_dimensions();

        unsafe {
            match backend.make_current() {
                Ok(_) => {}
                Err(err) => {
                    panic!("Failed to make backend current");
                }
            }
        }

        let wrapped_display = unsafe { egl_util::WrappedDisplay::new() };

        let resource_context = unsafe { egl_util::create_extra_context() };

        unsafe { backend.make_current(); }

        let wm_context = unsafe { egl_util::create_extra_context() };

        unsafe {
            wrapped_display.release_context();

            if backend.is_current() {
                panic!("Failed to release graphics backend");
            }
        }





    new engine




    pub channel_registry: RefCell<ChannelRegistry>,
    pub test_handler: Arc<TestHandler>,
    pub test_channel: RefCell<Weak<EventChannel>>,


        let instance_ref = FlutterInstanceRef(Arc::new(ReentrantMutex::new(FlutterInstance {

            channel_registry: RefCell::new(ChannelRegistry::new()),
            test_handler: Arc::new(TestHandler),
            test_channel: RefCell::new(Weak::new()),
        })));

        info!("Setting up channel registry");

        {
            let instance = instance_ref.get();

            let handler = Arc::downgrade(&instance.test_handler);

            let mut registry = instance.channel_registry.borrow_mut();
            registry.init(instance_ref.downgrade());
            let channel = registry.register_channel(EventChannel::new("test/stream", handler, instance_ref.downgrade()));

            instance_ref.get().test_channel.replace(channel);

//            channel.upgrade().as_ref().unwrap().send_success_event(&Value::String("Hello world".to_string()));
        }



*/

impl FlutterCompositorRef {
    pub fn start(&self) {
        let weak = self.downgrade();

        let event_loop = {
            let mut compositor = self.get();

            compositor.backend.init(weak.clone());

            FlutterEngine::run(&mut compositor);

            compositor.backend.run();

            info!("pre get_framebuffer_size");

            let dims = compositor.backend.get_framebuffer_size();
            info!("dims={:?}", dims);
            compositor.engine.send_window_metrics_event(
                dims.0 as i32,
                dims.1 as i32,
                dims.1 as f64 / 1080.0,
            );

            info!("post get_framebuffer_size");

            compositor.backend.event_loop.clone()
        };

        info!("Starting loop");

        let running = Arc::new(AtomicBool::new(true));

        let mut val = 0;

        while running.load(Ordering::SeqCst) {
            {
                let compositor_ref = weak.upgrade().unwrap();
                let compositor = compositor_ref.get();
                compositor.backend.update();
            }

            //            drawer.get().test(&*window_map.borrow(), compositor_token);

            unsafe {
                flutter_engine_sys::__FlutterEngineFlushPendingTasksNow();
            }

            if RefCell::borrow_mut(event_loop.borrow())
                .as_mut()
                .unwrap()
                .dispatch(Some(::std::time::Duration::from_millis(16)), &mut ())
                .is_err()
            {
                running.store(false, Ordering::SeqCst);
            }

            val += 1;
            if val > 60 * 10 {
                running.store(false, Ordering::SeqCst);
            }

            //            else {
            //                window_map.borrow_mut().refresh();
            //            }
        }
    }
}
