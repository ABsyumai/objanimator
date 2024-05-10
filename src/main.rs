use std::cell::Cell;
use std::collections::VecDeque;
use std::mem;
use std::os::raw::c_void;
use std::sync::Arc;
use std::time::{Duration, Instant};

use c_str_macro::c_str;
use cgmath::perspective;
use cgmath::prelude::SquareMatrix;
use gl::types::{GLfloat, GLsizei, GLsizeiptr};
use imgui::im_str;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use cacher::Cacher;
use util::SliceAs;

mod image_manager;
mod shader;
mod vertex;

use image_manager::Texture;
use shader::Shader;
use vertex::Vertex;

#[allow(dead_code)]
type Point3 = cgmath::Point3<f32>;
#[allow(dead_code)]
type Vector3 = cgmath::Vector3<f32>;
#[allow(dead_code)]
type Matrix4 = cgmath::Matrix4<f32>;

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;
const FLOAT_NUM: usize = 8;

fn new_vertex(buf: &[f32]) -> Vertex {
    Vertex::new(
        (buf.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
        buf.as_ptr() as *const c_void,
        gl::DYNAMIC_DRAW,
        vec![gl::FLOAT, gl::FLOAT, gl::FLOAT],
        vec![3, 3, 2],
        (FLOAT_NUM * mem::size_of::<GLfloat>()) as GLsizei,
        (buf.len() / FLOAT_NUM) as i32,
    )
}

/// イベントループの1ループごとにsetを呼び出して，fpsを測定
/// 1秒前までにsetされた回数をカウント
#[derive(Debug, Default)]
struct FPSBencher {
    v: VecDeque<Instant>,
}
impl FPSBencher {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn fps(&self) -> f32 {
        self.v.len() as _
    }
    pub fn set(&mut self) {
        let now = Instant::now();
        while !self.v.is_empty() {
            if now - self.v[0] > Duration::from_secs(1) {
                self.v.pop_front();
            } else {
                break;
            }
        }
        self.v.push_back(now);
    }
}
#[derive(Debug, Default)]
struct SuccessCounter {
    acc: usize,
    mem: Vec<bool>,
    index: usize,
}

impl SuccessCounter {
    pub fn new(len: usize) -> SuccessCounter {
        Self {
            mem: vec![false; len],
            ..Default::default()
        }
    }
    pub fn set(&mut self, success: bool) {
        let old = self.mem[self.index];
        self.mem[self.index] = success;
        match (old, success) {
            (true, false) => self.acc -= 1,
            (false, true) => self.acc += 1,
            _ => (),
        }
        self.index = (self.index + 1) % self.mem.len();
    }
    pub fn get(&self) -> f32 {
        self.acc as _
    }
    pub fn len(&self) -> f32 {
        self.mem.len() as _
    }
}

/// 前回のスリープから正確に時間を刻む
#[derive(Debug)]
struct Sleeper {
    inst: Cell<Instant>,
}
impl Sleeper {
    pub fn new() -> Self {
        Self {
            inst: Cell::new(Instant::now()),
        }
    }
    pub fn sleep(&self, time: Duration) {
        let prg = self.inst.get().elapsed();
        if time > prg {
            std::thread::sleep(time - prg);
        }
        self.reset();
    }
    pub fn reset(&self) {
        self.inst.set(Instant::now());
    }
}

///コマンドライン引数を解析してイベントループに渡す
fn main() {
    let mut args = std::env::args();
    args.next();
    let v = args.next().expect("require argment vertex_path");
    let t = args.next().expect("require argment texture_path");
    dbg!((&v, &t));
    let vertex_path: Vec<_> = v.split("{}").collect();
    let texture_path: Vec<_> = t.split("{}").collect();
    let start: usize = args
        .next()
        .expect("require argment start")
        .parse()
        .expect("failed to parse start");
    let last: usize = args
        .next()
        .expect("require argment last")
        .parse()
        .expect("failed to parse last");
    truth_main(
        (start..=last)
            .map(|i| format!("{}{}{}", vertex_path[0], i, vertex_path[1]))
            .collect(),
        (start..=last)
            .map(|i| format!("{}{}{}", texture_path[0], i, texture_path[1]))
            .collect(),
    );
}
///ファイルをすべて読み込んだ時のメモリ量測定用
#[allow(dead_code)]
fn bad_main(vertexes: Vec<String>, textures: Vec<String>) {
    use std::io::Read;
    let mut v = vec![];
    v.push((
        vertexes
            .iter()
            .map(|p| {
                let mut f = std::fs::File::open(p).unwrap();
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).unwrap();
                eprint!("\rread {}", p);
                buf
            })
            .collect::<Vec<_>>(),
        textures
            .iter()
            .map(|p| {
                let mut f = std::fs::File::open(p).unwrap();
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).unwrap();
                buf
            })
            .collect::<Vec<_>>(),
    ));
    std::thread::sleep(Duration::from_secs(10));
}
///イベントループの実装
fn truth_main(vertexes: Vec<String>, textures: Vec<String>) {
    dbg!(&vertexes[..4]);
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    {
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 1);
        let (major, minor) = gl_attr.context_version();
        println!("OK: init OpenGL: version={}.{}", major, minor);
    }

    let window = video_subsystem
        .window("Obj Animator", WINDOW_WIDTH, WINDOW_HEIGHT)
        .opengl()
        .position_centered()
        .maximized()
        .resizable()
        .build()
        .unwrap();

    let _gl_context = window.gl_create_context().unwrap();
    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as _);

    let shader = Shader::new("shader/shader.vs", "shader/shader.fs");

    // init imgui
    let mut imgui_context = imgui::Context::create();
    imgui_context.set_ini_filename(None);

    // init imgui sdl2
    let mut imgui_sdl2_context = imgui_sdl2::ImguiSdl2::new(&mut imgui_context, &window);
    let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui_context, |s| {
        video_subsystem.gl_get_proc_address(s) as _
    });
    let vertexes = Arc::new(vertexes);
    let textures = Arc::new(textures);
    let mut vertex_cache = Cacher::new(5, Arc::clone(&vertexes), |x| x, |b, _| b);
    let mut texture_cache = Cacher::new(
        5,
        Arc::clone(&textures),
        |x| {
            // dbg!("decoding");
            let ret = image::load_from_memory(x.as_ref()).expect("");
            // dbg!("decoded");
            ret
        },
        |b, _| b,
    );
    let mut vertex = loop {
        if let Some(x) = vertex_cache.get(0) {
            break new_vertex(unsafe { x.as_ref().as_ref().slice_as().unwrap() });
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let mut texture = loop {
        if let Some(x) = texture_cache.get(0) {
            break Texture::new(&x, true);
        }
        std::thread::sleep(Duration::from_millis(1));
    };

    let mut depth_test: bool = true;
    let mut blend: bool = true;
    let mut wireframe: bool = false;
    let mut culling: bool = true;
    let mut eye = Point3::new(2.0, 2.0, 2.0);
    let mut center = Point3::new(0.0, 0.0, 0.0);
    let mut up = Vector3::new(0.0, 0.0, 1.0);
    let mut file_index = 1;
    let step = 1;
    let len = vertexes.len();
    let mut max_fps = 30;
    let sleeper = Sleeper::new();
    let mut success_counter = SuccessCounter::new(60);
    let mut fps_bencher = FPSBencher::new();

    let mut event_pump = sdl_context.event_pump().unwrap();
    dbg!("start loop");
    'running: loop {
        for event in event_pump.poll_iter() {
            imgui_sdl2_context.handle_event(&mut imgui_context, &event);
            if imgui_sdl2_context.ignore_event(&event) {
                continue;
            }

            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        unsafe {
            if depth_test {
                gl::Enable(gl::DEPTH_TEST);
            } else {
                gl::Disable(gl::DEPTH_TEST);
            }

            if blend {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            } else {
                gl::Disable(gl::BLEND);
            }

            if wireframe {
                gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
            } else {
                gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
            }

            if culling {
                gl::Enable(gl::CULL_FACE);
            } else {
                gl::Disable(gl::CULL_FACE);
            }

            gl::Viewport(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);

            // clear screen
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            // init matrice for model, view and projection
            let model_matrix = Matrix4::identity();
            let view_matrix = Matrix4::look_at(eye, center, up);
            let projection_matrix: Matrix4 = perspective(
                cgmath::Deg(45.0f32),
                WINDOW_WIDTH as f32 / WINDOW_HEIGHT as f32,
                0.1,
                100.0,
            );

            // shader use matrices
            shader.use_program();
            shader.set_mat4(c_str!("uModel"), &model_matrix);
            shader.set_mat4(c_str!("uView"), &view_matrix);
            shader.set_mat4(c_str!("uProjection"), &projection_matrix);
            shader.set_vec3(c_str!("uViewPosition"), eye.x, eye.y, eye.z);
        }
        let nowi = file_index;
        match (vertex_cache.get(file_index), texture_cache.get(file_index)) {
            (Some(v), Some(t)) => {
                vertex = new_vertex(unsafe {
                    let x = v.as_ref().as_ref().slice_as().unwrap();
                    x
                });
                texture = Texture::new(&t, true);
                file_index += step;
                file_index %= len;
                success_counter.set(true);
            }
            _ => success_counter.set(false),
        }
        texture.using(|| {
            vertex.draw();
        });
        imgui_sdl2_context.prepare_frame(
            imgui_context.io_mut(),
            &window,
            &event_pump.mouse_state(),
        );
        //ui
        let ui = imgui_context.frame();
        imgui::Window::new(im_str!("Information"))
            .size([300.0, 300.0], imgui::Condition::FirstUseEver)
            .position([10.0, 10.0], imgui::Condition::FirstUseEver)
            .build(&ui, || {
                ui.text(im_str!("Obj Animator ver 1.0"));
                ui.separator();
                ui.text(im_str!("FPS: {}", fps_bencher.fps()));
                ui.text(im_str!(
                    "MODEL FPS: {:.2}",
                    fps_bencher.fps() * success_counter.get() / success_counter.len()
                ));
                imgui::Slider::new(im_str!("max fps"), 1..=120).build(&ui, &mut max_fps);
                ui.separator();
                ui.text(im_str!("show: {}", vertexes[nowi]));
                ui.text(im_str!("index: {}/{}", nowi, len - 1));
                ui.text(im_str!(
                    "hit rate: {}/{}",
                    success_counter.get(),
                    success_counter.len()
                ));
                ui.separator();
                let display_size = ui.io().display_size;
                ui.text(format!(
                    "Display Size: ({:.1}, {:.1})",
                    display_size[0], display_size[1]
                ));
                let mouse_pos = ui.io().mouse_pos;
                ui.text(format!(
                    "Mouse Position: ({:.1}, {:.1})",
                    mouse_pos[0], mouse_pos[1]
                ));

                ui.separator();

                ui.checkbox(im_str!("Depth Test"), &mut depth_test);
                ui.checkbox(im_str!("Blend"), &mut blend);
                ui.checkbox(im_str!("Wireframe"), &mut wireframe);
                ui.checkbox(im_str!("Culling"), &mut culling);

                ui.separator();

                #[rustfmt::skip]
                imgui::Slider::new(im_str!("eye X"), -5.0..=5.0)
                    .build(&ui, &mut eye.x);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("eye Y"), -5.0..=5.0)
                    .build(&ui, &mut eye.y);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("eye Z"), -5.0..=5.0)
                    .build(&ui, &mut eye.z);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("center X"), -5.0..=5.0)
                    .build(&ui, &mut center.x);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("center Y"), -5.0..=5.0)
                    .build(&ui, &mut center.y);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("center Z"), -5.0..=5.0)
                    .build(&ui, &mut center.z);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("up X"), -5.0..=5.0)
                    .build(&ui, &mut up.x);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("up Y"), -5.0..=5.0)
                    .build(&ui, &mut up.y);
                #[rustfmt::skip]
                imgui::Slider::new(im_str!("up Z"), -5.0..=5.0)
                    .build(&ui, &mut up.z);
            });
        imgui_sdl2_context.prepare_render(&ui, &window);
        renderer.render(ui);
        //end ui
        fps_bencher.set();
        window.gl_swap_window();
        sleeper.sleep(Duration::new(0, 1_000_000_000u32 / max_fps));
    }
}
