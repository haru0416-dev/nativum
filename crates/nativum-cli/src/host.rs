//! OS window host: winit + softbuffer presenting the software renderer.
//! The engine stays display-free; this crate only maps pixels and input.

use std::num::NonZeroU32;
use std::path::Path;
use std::rc::Rc;

use anyhow::{bail, Context, Result};
use nativum_engine::{load_app_dir, Session, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Open a real window and run until the user closes it (or `--frames` is spent).
pub fn run(path: &Path, dark: bool, frames: Option<u32>) -> Result<()> {
    let loaded = load_app_dir(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let title = loaded.manifest.window.title.clone();
    let width = loaded.manifest.window.width;
    let height = loaded.manifest.window.height;
    let mut session = Session::from_loaded(loaded).map_err(|e| anyhow::anyhow!("{e}"))?;
    if dark {
        session
            .set_appearance(nativum_core::Appearance::Dark)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    println!("nativum run  {title}  {}×{}", width as u32, height as u32);

    let event_loop = EventLoop::new().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = Host {
        session,
        title,
        width,
        height,
        window: None,
        context: None,
        surface: None,
        cursor: PhysicalPosition::new(0.0, 0.0),
        frames_left: frames,
        last_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| anyhow::anyhow!("event loop: {e}"))?;
    if let Some(e) = app.last_error {
        bail!("{e}");
    }
    Ok(())
}

struct Host {
    session: Session,
    title: String,
    width: f32,
    height: f32,
    window: Option<Rc<Window>>,
    context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    cursor: PhysicalPosition<f64>,
    frames_left: Option<u32>,
    last_error: Option<String>,
}

impl Host {
    fn fail(&mut self, event_loop: &ActiveEventLoop, err: impl std::fmt::Display) {
        self.last_error = Some(err.to_string());
        event_loop.exit();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn sync_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let suffix = title_suffix(&self.session);
        if suffix.is_empty() {
            window.set_title(&self.title);
        } else {
            window.set_title(&format!("{} — {suffix}", self.title));
        }
    }

    fn present(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let physical = window.inner_size();
        let Some(pw) = NonZeroU32::new(physical.width) else {
            return;
        };
        let Some(ph) = NonZeroU32::new(physical.height) else {
            return;
        };
        if let Err(e) = surface.resize(pw, ph) {
            self.fail(event_loop, format!("resize buffer: {e}"));
            return;
        }
        let frame = match self.session.frame() {
            Some(f) => f,
            None => {
                self.fail(event_loop, "session has no frame");
                return;
            }
        };
        let src = &frame.surface;
        let mut buf = match surface.buffer_mut() {
            Ok(b) => b,
            Err(e) => {
                self.fail(event_loop, format!("buffer: {e}"));
                return;
            }
        };
        blit_nearest(&mut buf, physical.width, physical.height, src);
        if let Err(e) = buf.present() {
            self.fail(event_loop, format!("present: {e}"));
            return;
        }
        self.sync_title();
        if let Some(left) = self.frames_left.as_mut() {
            *left = left.saturating_sub(1);
            if *left == 0 {
                event_loop.exit();
            }
        }
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(self.width, self.height))
            .with_resizable(true);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                self.fail(event_loop, format!("create window: {e}"));
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.fail(event_loop, format!("softbuffer context: {e}"));
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.fail(event_loop, format!("softbuffer surface: {e}"));
                return;
            }
        };
        window.request_redraw();
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.present(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(window) = &self.window {
                    let logical = size.to_logical::<f32>(window.scale_factor());
                    if let Err(e) = self.session.resize(logical.width, logical.height) {
                        self.fail(event_loop, e);
                        return;
                    }
                }
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(window) = &self.window {
                    let size = window.inner_size().to_logical::<f32>(scale_factor);
                    if let Err(e) = self.session.resize(size.width, size.height) {
                        self.fail(event_loop, e);
                        return;
                    }
                }
                self.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor = position,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let scale = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor())
                    .unwrap_or(1.0);
                let x = (self.cursor.x / scale) as f32;
                let y = (self.cursor.y / scale) as f32;
                match self.session.click(x, y) {
                    Ok(_) => self.request_redraw(),
                    Err(e) => self.fail(event_loop, e),
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if let Some(name) = key_name(&event) {
                    match self.session.key(&name) {
                        Ok(_) => self.request_redraw(),
                        Err(e) => self.fail(event_loop, e),
                    }
                }
            }
            _ => {}
        }
    }
}

fn title_suffix(session: &Session) -> String {
    let model = session.model();
    if let Some(c) = model.get("count") {
        return c.display();
    }
    if let Some(d) = model.get("display") {
        return d.display();
    }
    String::new()
}

fn key_name(event: &winit::event::KeyEvent) -> Option<String> {
    if let Some(text) = &event.text {
        if !text.is_empty() && !text.chars().any(char::is_control) {
            return Some(text.to_string());
        }
    }
    match &event.logical_key {
        Key::Named(NamedKey::Enter) => Some("Enter".to_string()),
        Key::Named(NamedKey::Backspace) => Some("Backspace".to_string()),
        Key::Named(NamedKey::Escape) => Some("Escape".to_string()),
        Key::Named(NamedKey::Space) => Some(" ".to_string()),
        Key::Character(s) => Some(s.to_string()),
        _ => None,
    }
}

fn blit_nearest(dest: &mut [u32], dest_w: u32, dest_h: u32, src: &Surface) {
    if dest_w == 0 || dest_h == 0 || src.width == 0 || src.height == 0 {
        return;
    }
    let src_w = src.width;
    let src_h = src.height;
    for y in 0..dest_h {
        let sy = y.saturating_mul(src_h) / dest_h;
        for x in 0..dest_w {
            let sx = x.saturating_mul(src_w) / dest_w;
            let i = ((sy * src_w + sx) * 4) as usize;
            let r = src.pixels[i] as u32;
            let g = src.pixels[i + 1] as u32;
            let b = src.pixels[i + 2] as u32;
            dest[(y * dest_w + x) as usize] = (r << 16) | (g << 8) | b;
        }
    }
}
