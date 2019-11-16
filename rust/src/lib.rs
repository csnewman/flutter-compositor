#[macro_use(define_roles)]
extern crate smithay;

extern crate parking_lot;

extern crate rand;

use log::{debug, error, info};

use wayland_server::calloop::EventLoop;
use wayland_server::Display;

pub mod backends;
pub mod flutter;

mod renderer;

mod window_map;

mod shell;

use crate::backends::CompositorBackend;
use crate::flutter::channel::Channel;
use crate::flutter::FlutterEngine;
use backends::udev;
use backends::winit;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Weak};
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

pub(crate) type MainThreadChannelFn = (String, Box<dyn FnMut(&dyn Channel) + Send>);

pub(crate) enum MainThreadCallback {
    ChannelFn(MainThreadChannelFn),
}

pub struct FlutterCompositor {
    backend: CompositorBackend,
    runtime: Runtime,
    task_executor: TaskExecutor,
    main_thread_sender: Sender<MainThreadCallback>,
    main_thread_receiver: Receiver<MainThreadCallback>,
    engine: FlutterEngine,
}

impl FlutterCompositor {
    pub fn new(backend: CompositorBackend) -> FlutterCompositorRef {
        let runtime = Runtime::new().unwrap();

        let (main_tx, main_rx) = mpsc::channel();

        let compositor_ref = FlutterCompositorRef {
            inner: Arc::new(ReentrantMutex::new(Self {
                backend,
                task_executor: runtime.executor(),
                main_thread_sender: main_tx,
                main_thread_receiver: main_rx,
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

impl FlutterCompositorRef {
    pub fn register_channel<C>(&self, mut channel: C) -> Weak<C>
    where
        C: Channel + 'static,
    {
        self.get().engine.channel_registry.register_channel(channel)
    }

    pub fn start(&self) {
        let weak = self.downgrade();

        let event_loop = {
            let mut compositor = self.get();

            compositor.backend.init(weak.clone());

            {
                let mut input_ref = compositor.backend.input.borrow_mut();
                input_ref
                    .as_mut()
                    .unwrap()
                    .register_channels(&compositor.engine.channel_registry);
            }

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

                // Process callbacks
                let callbacks: Vec<MainThreadCallback> = compositor.main_thread_receiver.try_iter().collect();
                for cb in callbacks {
                    match cb {
                        MainThreadCallback::ChannelFn((name, mut f)) => {
                            compositor.engine.channel_registry.with_channel(&name, |channel| {
                                f(channel);
                            });
                        },
                    }
                }
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
