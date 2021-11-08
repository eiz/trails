use anyhow::Result;
use d3d11::{
    Dx11ComputeShader, Dx11ConstantBuffer, Dx11Device, Dx11RWStructuredBuffer, Dx11Texture2D,
};
use rand::{prelude::StdRng, Rng, SeedableRng};
use std::{cmp, f32::consts::PI, ptr, time::Instant};
use structopt::StructOpt;
use winapi::{
    shared::dxgiformat::DXGI_FORMAT_R16G16B16A16_FLOAT,
    um::{synchapi::WaitForSingleObject, winbase::INFINITE},
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    platform::{
        run_return::EventLoopExtRunReturn,
        windows::{EventLoopExtWindows, WindowExtWindows},
    },
    window::{Fullscreen, WindowBuilder},
};

use crate::d3d11::Dx11SwapChain;

mod d3d11;
mod encoder;
mod shaders {
    pub const SLIME_ADVANCE_AGENTS_CS: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shader/slime.advance_agents.cso"));
    pub const SLIME_DECAY_AND_DIFFUSE_CS: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/shader/slime.decay_and_diffuse.cso"
    ));
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Vec4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Vec2 {
    x: f32,
    y: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Agent {
    color: Vec4,
    position: Vec2,
    heading: f32,
}

impl Agent {
    pub fn morton_pos(&self) -> u32 {
        const B: [u32; 4] = [0x55555555, 0x33333333, 0x0F0F0F0F, 0x00FF00FF];
        const S: [u32; 4] = [1, 2, 4, 8];

        let mut x = self.position.x.floor() as u32;
        let mut y = self.position.y.floor() as u32;

        x = (x | (x << S[3])) & B[3];
        x = (x | (x << S[2])) & B[2];
        x = (x | (x << S[1])) & B[1];
        x = (x | (x << S[0])) & B[0];

        y = (y | (y << S[3])) & B[3];
        y = (y | (y << S[2])) & B[2];
        y = (y | (y << S[1])) & B[1];
        y = (y | (y << S[0])) & B[0];

        x | (y << 1)
    }
}

#[derive(Debug, Clone, Copy, StructOpt)]
struct Settings {
    #[structopt(default_value = "256", long)]
    width: u32,
    #[structopt(default_value = "256", long)]
    height: u32,
    #[structopt(default_value = "100", long)]
    num_agents: u32,
    #[structopt(default_value = "1", long)]
    steps_per_tick: u32,
    #[structopt(default_value = "0", long)]
    seed: u32,
    #[structopt(default_value = "1.0", long)]
    agent_speed: f32,
    #[structopt(default_value = "360.0", long)]
    agent_turn_rate_deg: f32,
    #[structopt(default_value = "30.0", long)]
    sensor_angle_deg: f32,
    #[structopt(default_value = "30.0", long)]
    sensor_offset: f32,
    #[structopt(default_value = "1", long)]
    sensor_size: u32,
    #[structopt(default_value = "1.0", long)]
    same_color_weight: f32,
    #[structopt(default_value = "-1.0", long)]
    different_color_weight: f32,
    #[structopt(default_value = "0.0", long)]
    eat_weight: f32,
    #[structopt(default_value = "1.0", long)]
    trail_weight: f32,
    #[structopt(default_value = "1.0", long)]
    exponential_decay_rate: f32,
    #[structopt(default_value = "0.0", long)]
    linear_decay_rate: f32,
    #[structopt(default_value = "1.0", long)]
    diffuse_rate: f32,
    #[structopt(default_value = "4.0", long)]
    density: f32,
}

#[derive(Debug, Clone, Copy)]
struct Constants {
    resolution: Vec2,            // 0
    num_agents: u32,             // 2
    steps_per_tick: u32,         // 3
    agent_speed: f32,            // 4
    agent_turn_rate_rad: f32,    // 5
    sensor_angle_rad: f32,       // 6
    sensor_offset: f32,          // 7
    sensor_size: u32,            // 8
    _pad0: u32,                  // 9
    _pad1: u32,                  // 10
    _pad2: u32,                  // 11
    agent_color: Vec4,           // 12
    same_color_weight: f32,      // 16
    different_color_weight: f32, // 17
    eat_weight: f32,             // 18
    trail_weight: f32,           // 19
    diffuse_rate: f32,           // 20
    exponential_decay_rate: f32, // 21
    linear_decay_rate: f32,      // 22
    time: f32,                   // 23
    delta_time: f32,             // 24
    _pad3: u32,                  // 25
    _pad4: u32,                  // 26
    _pad5: u32,                  // 27
}

impl Constants {
    pub fn new(
        settings: &Settings,
        initial_time: Instant,
        last_frame_time: Instant,
        current_time: Instant,
    ) -> Constants {
        Self {
            resolution: Vec2 {
                x: settings.width as f32,
                y: settings.height as f32,
            },
            num_agents: settings.num_agents,
            steps_per_tick: settings.steps_per_tick,
            agent_speed: settings.agent_speed,
            agent_turn_rate_rad: settings.agent_turn_rate_deg as f32 * PI / 180.0,
            sensor_angle_rad: settings.sensor_angle_deg as f32 * PI / 180.0,
            sensor_offset: settings.sensor_offset,
            sensor_size: settings.sensor_size,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            agent_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            }, // unused
            same_color_weight: settings.same_color_weight,
            different_color_weight: settings.different_color_weight,
            eat_weight: settings.eat_weight,
            trail_weight: settings.trail_weight,
            diffuse_rate: settings.diffuse_rate,
            exponential_decay_rate: settings.exponential_decay_rate,
            linear_decay_rate: settings.linear_decay_rate,
            time: current_time.duration_since(initial_time).as_secs_f32(),
            delta_time: current_time.duration_since(last_frame_time).as_secs_f32(),
            _pad3: 0,
            _pad4: 0,
            _pad5: 0,
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = s * v;
    let hp = h * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp.floor() as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };
    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
}

fn polar_to_rect(angle: f32, radius: f32) -> (f32, f32) {
    let (x, y) = angle.sin_cos();
    (x * radius, y * radius)
}

#[derive(Clone)]
struct Scene {
    device: Dx11Device,
    trails_texture: Dx11Texture2D,
    diffuse_texture: Dx11Texture2D,
    agents: Dx11RWStructuredBuffer<Agent>,
    advance_agents: Dx11ComputeShader,
    decay_and_diffuse: Dx11ComputeShader,
    settings: Settings,
    constants: Dx11ConstantBuffer<Constants>,
    initial_time: Instant,
    last_frame_time: Instant,
}

impl Scene {
    pub fn new(device: &Dx11Device, settings: Settings) -> Result<Self> {
        let trails_texture = Dx11Texture2D::new(
            device,
            settings.width,
            settings.height,
            DXGI_FORMAT_R16G16B16A16_FLOAT,
        )?;
        let diffuse_texture = Dx11Texture2D::new(
            device,
            settings.width,
            settings.height,
            DXGI_FORMAT_R16G16B16A16_FLOAT,
        )?;
        let mut agents = vec![];
        let mut rng = StdRng::seed_from_u64(settings.seed as u64);
        let radius = cmp::min(settings.width, settings.height) as f32 / settings.density;
        agents.resize_with(settings.num_agents as usize, || {
            let (px, py) = polar_to_rect(rng.gen::<f32>() * 2.0 * PI, rng.gen());
            let (r, g, b) = hsv_to_rgb(rng.gen(), 1.0, 1.0);
            Agent {
                color: Vec4 {
                    x: r * 12.0,
                    y: g * 12.0,
                    z: b * 12.0,
                    w: 1.0,
                },
                position: Vec2 {
                    x: settings.width as f32 / 2.0 + px * radius,
                    y: settings.height as f32 / 2.0 + py * radius,
                },
                heading: rng.gen::<f32>() * PI * 2.0,
            }
        });
        agents.sort_by(|a, b| a.morton_pos().cmp(&b.morton_pos()));
        let initial_time = Instant::now();
        let last_frame_time = initial_time;
        let constants = Dx11ConstantBuffer::new_with_data(
            device,
            &[Constants::new(
                &settings,
                initial_time,
                last_frame_time,
                last_frame_time,
            )],
        )?;
        Ok(Self {
            device: device.clone(),
            trails_texture,
            diffuse_texture,
            agents: Dx11RWStructuredBuffer::new_with_data(device, &agents)?,
            settings,
            initial_time,
            last_frame_time,
            constants,
            advance_agents: Dx11ComputeShader::new(device, shaders::SLIME_ADVANCE_AGENTS_CS)?,
            decay_and_diffuse: Dx11ComputeShader::new(device, shaders::SLIME_DECAY_AND_DIFFUSE_CS)?,
        })
    }

    pub fn render(&mut self) {
        let ctx = self.device.immediate_context();

        unsafe {
            let current_time = Instant::now();

            let constants = Constants::new(
                &self.settings,
                self.initial_time,
                self.last_frame_time,
                current_time,
            );
            self.constants.replace(&ctx, &[constants]);

            for _i in 0..self.settings.steps_per_tick {
                ctx.inner
                    .CSSetShader(self.advance_agents.inner.as_ptr(), ptr::null_mut(), 0);
                ctx.inner
                    .CSSetConstantBuffers(0, 1, [self.constants.inner.as_ptr()].as_ptr());
                ctx.inner.CSSetUnorderedAccessViews(
                    0,
                    3,
                    [
                        self.trails_texture.uav.as_ptr(),
                        self.diffuse_texture.uav.as_ptr(),
                        self.agents.uav.as_ptr(),
                    ]
                    .as_ptr(),
                    ptr::null(),
                );
                ctx.inner.Dispatch(self.settings.num_agents / 32 + 1, 1, 1);
                ctx.inner
                    .CSSetShader(self.decay_and_diffuse.inner.as_ptr(), ptr::null_mut(), 0);
                ctx.inner
                    .Dispatch(self.settings.width / 8 + 1, self.settings.height / 8 + 1, 1);
                std::mem::swap(&mut self.trails_texture, &mut self.diffuse_texture);
            }

            self.last_frame_time = current_time;
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    let settings = Settings::from_args();

    println!["{:?}", settings];
    let frame_count = 2;
    let mut event_loop = EventLoop::<()>::new_any_thread();
    let (width, height) = (settings.width, settings.height);
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(width, height))
        .with_fullscreen(Some(Fullscreen::Borderless(None)))
        .with_title("trails")
        .with_visible(false)
        .with_resizable(false)
        .build(&event_loop)?;
    let hwnd = window.hwnd();
    let device = Dx11Device::new()?;
    let swap_chain = Dx11SwapChain::new_with_hwnd(&device, hwnd, width, height, frame_count)?;
    let mut scene = Scene::new(&device, settings)?;
    let mut exited = false;
    window.set_visible(true);
    event_loop.run_return(move |event, _, control_flow| {
        if exited {
            *control_flow = ControlFlow::Exit;
            return;
        }
        *control_flow = ControlFlow::Poll;
        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    exited = true;
                }
                winit::event::WindowEvent::KeyboardInput { input, .. } => {
                    if input.state == ElementState::Pressed {
                        match input.virtual_keycode {
                            Some(VirtualKeyCode::Escape) => exited = true,
                            _ => (),
                        }
                    }
                }
                winit::event::WindowEvent::ModifiersChanged(_) => {
                    println!["modifierz"];
                }
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    println!["mousemove {:?}", position];
                }
                winit::event::WindowEvent::CursorEntered { .. } => {
                    println!["cursorenter"];
                }
                winit::event::WindowEvent::CursorLeft { .. } => {
                    println!["cursorleft"];
                }
                winit::event::WindowEvent::MouseWheel { delta, phase, .. } => {
                    println!["wheel {:?} {:?}", delta, phase];
                }
                winit::event::WindowEvent::MouseInput { state, button, .. } => {
                    println!["mouseinput {:?} {:?}", state, button];
                }
                _ => (),
            },
            winit::event::Event::MainEventsCleared => {
                let ctx = device.immediate_context();

                unsafe {
                    WaitForSingleObject(swap_chain.wait_handle, INFINITE);
                    scene.render();
                    ctx.inner.CopyResource(
                        swap_chain.back_buffer.as_ptr() as *mut _,
                        scene.trails_texture.inner.as_ptr() as *mut _,
                    );
                    swap_chain.inner.Present(1, 0);
                }
                //
            }
            _ => (),
        }
    });

    Ok(())
}
