//! Horizon-Lite: главный цикл с меню и миром из 10 блоков.

use horizon_client::{assets, gfx, mesher};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    SelectWorld,
    Playing,
}

#[derive(Clone, Copy)]
struct Button {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &'static str,
    action: Action,
    tex_key: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    OpenSelectWorld,
    PlayWorld,
    BackToMenu,
    Quit,
}

impl Button {
    fn hit(&self, mx: f32, my: f32) -> bool {
        mx >= self.x && mx <= self.x + self.w && my >= self.y && my <= self.y + self.h
    }
}

#[derive(Default)]
struct Camera {
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
}

impl Camera {
    fn new(pos: [f32; 3], yaw: f32, pitch: f32) -> Self {
        Self { pos, yaw, pitch }
    }
    fn forward(&self) -> [f32; 3] {
        let y = self.yaw.to_radians();
        let p = self.pitch.to_radians();
        [y.sin() * p.cos(), p.sin(), y.cos() * p.cos()]
    }
    fn right(&self) -> [f32; 3] {
        let y = self.yaw.to_radians();
        [y.cos(), 0.0, -y.sin()]
    }
    fn move_forward(&mut self, dist: f32) {
        let f = self.forward();
        for i in 0..3 { self.pos[i] += f[i] * dist; }
    }
    fn move_right(&mut self, dist: f32) {
        let r = self.right();
        for i in 0..3 { self.pos[i] += r[i] * dist; }
    }
    fn move_up(&mut self, dist: f32) { self.pos[1] += dist; }
    fn rotate(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx as f32 * 0.15;
        self.pitch = (self.pitch - dy as f32 * 0.15).clamp(-89.0, 89.0);
    }
}

struct Input {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    grabbed: bool,
    last_cursor: Option<PhysicalPosition<f64>>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            forward: false, back: false, left: false, right: false,
            up: false, down: false,
            grabbed: false, last_cursor: None,
        }
    }
}

fn world_blocks(assets: &assets::Assets) -> Vec<([f32; 3], u32)> {
    let names = [
        "stone", "grass_side", "dirt", "log_oak", "bricks",
        "coal_ore", "diamond_block", "gold_block", "iron_block", "sand",
    ];
    let mut v = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let layer = assets.block_layer(name).unwrap_or(0);
        let x = (i % 5) as f32 * 1.2;
        let z = (i / 5) as f32 * 1.5;
        v.push(([x, 0.0, z], layer));
    }
    v
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<gfx::Renderer>,
    assets: Option<Arc<assets::Assets>>,
    camera: Camera,
    input: Input,
    screen: Screen,
    last_frame: std::time::Instant,
    menu_time: f32,
    cursor: (f32, f32),
}

impl App {
    fn buttons(&self) -> Vec<Button> {
        let (w, h) = match &self.window {
            Some(win) => {
                let s = win.inner_size();
                (s.width as f32, s.height as f32)
            }
            None => (1280.0, 720.0),
        };
        let bw = 320.0;
        let bh = 40.0;
        let bx = w / 2.0 - bw / 2.0;
        let by = h / 2.0;
        let (cx, cy) = self.cursor;
        let tex_for = |yy: f32| {
            if cx >= bx && cx <= bx + bw && cy >= yy && cy <= yy + bh {
                "button_hover"
            } else {
                "button_normal"
            }
        };
        match self.screen {
            Screen::MainMenu => vec![
                Button { x: bx, y: by, w: bw, h: bh, label: "Singleplayer",
                         action: Action::OpenSelectWorld, tex_key: tex_for(by) },
                Button { x: bx, y: by + 60.0, w: bw, h: bh, label: "Quit Game",
                         action: Action::Quit, tex_key: tex_for(by + 60.0) },
            ],
            Screen::SelectWorld => vec![
                Button { x: bx, y: by, w: bw, h: bh, label: "World 10 Blocks",
                         action: Action::PlayWorld, tex_key: tex_for(by) },
                Button { x: bx, y: by + 60.0, w: bw, h: bh, label: "Back",
                         action: Action::BackToMenu, tex_key: tex_for(by + 60.0) },
            ],
            Screen::Playing => Vec::new(),
        }
    }

    fn apply_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::OpenSelectWorld => self.screen = Screen::SelectWorld,
            Action::BackToMenu => self.screen = Screen::MainMenu,
            Action::PlayWorld => {
                self.screen = Screen::Playing;
                self.camera = Camera::new([5.5, 3.0, 8.0], 180.0, 20.0);
                if let Some(w) = &self.window {
                    let _ = w.set_cursor_grab(CursorGrabMode::Locked);
                    w.set_cursor_visible(false);
                }
                self.input = Input::default();
                self.input.grabbed = true;
            }
            Action::Quit => event_loop.exit(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        println!("[Main] Запуск Horizon-Lite...");

        let root = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let assets = match assets::Assets::load(&root) {
            Ok(a) => Arc::new(a),
            Err(e) => {
                eprintln!("[Main] ошибка загрузки ассетов: {}", e);
                event_loop.exit();
                return;
            }
        };

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Horizon-Lite (Minecraft 1.0)")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0)),
                )
                .expect("не удалось создать окно"),
        );
        self.window = Some(window.clone());

        match gfx::build_renderer(
            event_loop,
            window,
            &assets.block_pixels,
            assets.block_size,
            assets.block_layers,
            &assets.font_pixels,
            assets.font_w,
            assets.font_h,
            &assets.gui.pixels,
            assets.gui.width,
            assets.gui.height,
        ) {
            Ok(r) => {
                self.renderer = Some(r);
                self.assets = Some(assets);
                println!("[Main] Рендерер готов.");
            }
            Err(e) => {
                eprintln!("[Main] ошибка рендерера: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if self.screen == Screen::Playing && self.input.grabbed {
                    if let Some(last) = self.input.last_cursor {
                        let dx = position.x - last.x;
                        let dy = position.y - last.y;
                        self.camera.rotate(dx, dy);
                    }
                    self.input.last_cursor = Some(position);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                let code = match event.physical_key {
                    PhysicalKey::Code(c) => c,
                    _ => return,
                };
                if code == KeyCode::Escape && pressed {
                    match self.screen {
                        Screen::Playing => {
                            self.screen = Screen::SelectWorld;
                            self.input.grabbed = false;
                            if let Some(w) = &self.window {
                                let _ = w.set_cursor_grab(CursorGrabMode::None);
                                w.set_cursor_visible(true);
                            }
                            self.input = Input::default();
                        }
                        Screen::SelectWorld => self.screen = Screen::MainMenu,
                        Screen::MainMenu => event_loop.exit(),
                    }
                    return;
                }
                if self.screen == Screen::Playing {
                    match code {
                        KeyCode::KeyW => self.input.forward = pressed,
                        KeyCode::KeyS => self.input.back = pressed,
                        KeyCode::KeyA => self.input.left = pressed,
                        KeyCode::KeyD => self.input.right = pressed,
                        KeyCode::Space => self.input.up = pressed,
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => self.input.down = pressed,
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && self.screen != Screen::Playing
                {
                    let (mx, my) = self.cursor;
                    let action = self
                        .buttons()
                        .into_iter()
                        .find(|b| b.hit(mx, my))
                        .map(|b| b.action);
                    if let Some(a) = action {
                        self.apply_action(a, event_loop);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;
                self.menu_time += dt;

                if self.screen == Screen::Playing {
                    let speed = 8.0 * dt;
                    if self.input.forward { self.camera.move_forward(speed); }
                    if self.input.back    { self.camera.move_forward(-speed); }
                    if self.input.right   { self.camera.move_right(speed); }
                    if self.input.left    { self.camera.move_right(-speed); }
                    if self.input.up      { self.camera.move_up(speed); }
                    if self.input.down    { self.camera.move_up(-speed); }
                }

                let buttons = self.buttons();

                if let Some(r) = &mut self.renderer {
                    let (scr_w, scr_h) = (r.config.width, r.config.height);

                    let (cube, cam_pos, cam_yaw, cam_pitch) = match self.screen {
                        Screen::Playing => {
                            let assets = self.assets.as_ref().unwrap();
                            let cube = mesher::Mesher::mesh_blocks(&world_blocks(assets));
                            (cube, self.camera.pos, self.camera.yaw, self.camera.pitch)
                        }
                        _ => {
                            let cube = mesher::Mesher::mesh_cube([-0.5, -0.5, -0.5], 0);
                            let angle = self.menu_time * 20.0;
                            let rad = angle.to_radians();
                            let cam_pos = [3.0 * rad.sin(), 2.0, 3.0 * rad.cos()];
                            (cube, cam_pos, angle, 20.0)
                        }
                    };

                    r.set_camera(cam_pos, cam_yaw, cam_pitch);

                    let mut rects: Vec<gfx::Rect2D> = Vec::new();
                    if self.screen != Screen::Playing {
                        rects.push(gfx::Rect2D {
                            x: 0.0, y: 0.0,
                            w: scr_w as f32, h: scr_h as f32,
                            color: [0.0, 0.0, 0.0, 0.45],
                        });
                    }

                    let mut sprites: Vec<gfx::SpriteCall> = Vec::new();
                    if let Some(assets) = &self.assets {
                        for b in &buttons {
                            if let Some(uv) = assets.gui.rects.get(b.tex_key) {
                                sprites.push(gfx::SpriteCall {
                                    x: b.x, y: b.y, w: b.w, h: b.h,
                                    uv: *uv,
                                });
                            }
                        }
                        if self.screen == Screen::Playing {
                            if let Some(uv) = assets.gui.rects.get("hotbar") {
                                let hw = 182.0;
                                let hh = 22.0;
                                sprites.push(gfx::SpriteCall {
                                    x: scr_w as f32 / 2.0 - hw / 2.0,
                                    y: scr_h as f32 - hh - 2.0,
                                    w: hw, h: hh, uv: *uv,
                                });
                            }
                            if let Some(uv) = assets.gui.rects.get("hotbar_selection") {
                                let hw = 182.0;
                                let hh = 22.0;
                                sprites.push(gfx::SpriteCall {
                                    x: scr_w as f32 / 2.0 - hw / 2.0,
                                    y: scr_h as f32 - hh - 2.0,
                                    w: 22.0, h: hh, uv: *uv,
                                });
                            }
                        }
                    }

                    let cx = scr_w as f32 / 2.0;
                    let cy = scr_h as f32 / 2.0;
                    let mut texts: Vec<(&str, f32, f32, f32)> = Vec::new();
                    match self.screen {
                        Screen::MainMenu => {
                            texts.push(("Horizon-Lite", cx - 100.0, cy - 100.0, 3.0));
                            for b in &buttons {
                                texts.push((b.label, b.x + 20.0, b.y + 12.0, 2.0));
                            }
                        }
                        Screen::SelectWorld => {
                            texts.push(("Select World", cx - 90.0, cy - 100.0, 3.0));
                            for b in &buttons {
                                texts.push((b.label, b.x + 20.0, b.y + 12.0, 2.0));
                            }
                        }
                        Screen::Playing => {
                            texts.push(("WASD move, Esc back", 10.0, 10.0, 2.0));
                        }
                    }

                    let _ = r.render_frame(&cube, &rects, &sprites, &texts);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.screen == Screen::Playing && self.input.grabbed {
                self.camera.rotate(delta.0, delta.1);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("не удалось EventLoop");
    let mut app = App {
        window: None,
        renderer: None,
        assets: None,
        camera: Camera::new([5.5, 3.0, 8.0], 180.0, 20.0),
        input: Input::default(),
        screen: Screen::MainMenu,
        last_frame: std::time::Instant::now(),
        menu_time: 0.0,
        cursor: (0.0, 0.0),
    };
    event_loop.run_app(&mut app).expect("не удалось запустить");
}