use anyhow::Result;
use eiz::com::{com_new, com_new_void, ComError, ComPtr};
use std::{ffi::c_void, marker::PhantomData, ptr};
use winapi::{
    shared::{
        dxgi::{DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_DISCARD},
        dxgi1_2::{
            IDXGIFactory2, IDXGISwapChain1, DXGI_ALPHA_MODE_IGNORE, DXGI_SCALING_STRETCH,
            DXGI_SWAP_CHAIN_DESC1,
        },
        dxgi1_3::CreateDXGIFactory2,
        dxgi1_4::IDXGISwapChain3,
        dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_R16G16B16A16_FLOAT},
        dxgitype::{DXGI_SAMPLE_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT},
        minwindef::UINT,
    },
    um::{
        d3d11::{
            D3D11CreateDevice, ID3D11Buffer, ID3D11ComputeShader, ID3D11Device,
            ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Resource, ID3D11Texture2D,
            ID3D11UnorderedAccessView, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET,
            D3D11_BIND_UNORDERED_ACCESS, D3D11_BUFFER_DESC, D3D11_RESOURCE_MISC_BUFFER_STRUCTURED,
            D3D11_SDK_VERSION, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        },
        d3dcommon::D3D_DRIVER_TYPE_HARDWARE,
        winnt::HANDLE,
    },
    Interface,
};

#[derive(Clone)]
pub struct Dx11Device {
    pub inner: ComPtr<ID3D11Device>,
}

impl Dx11Device {
    pub fn new() -> Result<Self> {
        let inner = com_new(|x: *mut *mut ID3D11Device| unsafe {
            D3D11CreateDevice(
                ptr::null_mut(),
                D3D_DRIVER_TYPE_HARDWARE,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                0,
                D3D11_SDK_VERSION,
                x,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        })?;
        Ok(Self { inner })
    }

    pub fn immediate_context(&self) -> Dx11Context {
        Dx11Context {
            inner: com_new_void(|x| unsafe { self.inner.GetImmediateContext(x) }).unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct Dx11Context {
    pub inner: ComPtr<ID3D11DeviceContext>,
}

impl Dx11Context {
    //
}

pub struct Dx11SwapChain {
    pub inner: ComPtr<IDXGISwapChain3>,
    pub back_buffer: ComPtr<ID3D11Resource>,
    pub wait_handle: HANDLE,
}

impl Dx11SwapChain {
    pub fn new_with_hwnd(
        device: &Dx11Device,
        hwnd: *mut c_void,
        width: u32,
        height: u32,
        frame_count: u32,
    ) -> Result<Self, ComError> {
        let dxgi_factory = com_new(|x: *mut *mut IDXGIFactory2| unsafe {
            CreateDXGIFactory2(0, &IDXGIFactory2::uuidof(), x as *mut _)
        })?;
        let inner = com_new(|x: *mut *mut IDXGISwapChain1| unsafe {
            dxgi_factory.CreateSwapChainForHwnd(
                device.inner.as_ptr() as *mut _,
                hwnd as *mut _,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: width,
                    Height: height,
                    Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                    Stereo: 0,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: frame_count,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                    Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                },
                ptr::null(),
                ptr::null_mut(),
                x,
            )
        })?;
        let inner = inner.query_interface::<IDXGISwapChain3>()?;
        let back_buffer = com_new(|x: *mut *mut ID3D11Resource| unsafe {
            inner.GetBuffer(0, &ID3D11Resource::uuidof(), x as *mut *mut _)
        })?;
        let wait_handle = unsafe { inner.GetFrameLatencyWaitableObject() };
        Ok(Self {
            inner,
            back_buffer,
            wait_handle,
        })
    }
}

#[derive(Clone)]
pub struct Dx11Texture2D {
    pub inner: ComPtr<ID3D11Texture2D>,
    pub rtv: ComPtr<ID3D11RenderTargetView>,
    pub uav: ComPtr<ID3D11UnorderedAccessView>,
}

impl Dx11Texture2D {
    pub fn new(device: &Dx11Device, width: u32, height: u32, format: DXGI_FORMAT) -> Result<Self> {
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_UNORDERED_ACCESS | D3D11_BIND_RENDER_TARGET,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let inner =
            com_new(|x| unsafe { device.inner.CreateTexture2D(&texture_desc, ptr::null(), x) })?;
        let rtv = com_new(|x| unsafe {
            device
                .inner
                .CreateRenderTargetView(inner.as_ptr() as *mut _, ptr::null(), x)
        })?;
        let uav = com_new(|x| unsafe {
            device
                .inner
                .CreateUnorderedAccessView(inner.as_ptr() as *mut _, ptr::null(), x)
        })?;
        let immediate = device.immediate_context();
        unsafe {
            immediate
                .inner
                .ClearRenderTargetView(rtv.as_ptr(), &[0.0, 0.0, 0.0, 1.0]);
        }
        Ok(Self { inner, rtv, uav })
    }
}

#[derive(Clone)]
pub struct Dx11RWStructuredBuffer<T: Copy> {
    pub inner: ComPtr<ID3D11Buffer>,
    pub uav: ComPtr<ID3D11UnorderedAccessView>,
    _phantom: PhantomData<T>,
}

impl<T: Copy> Dx11RWStructuredBuffer<T> {
    pub fn new_with_data(device: &Dx11Device, data: &[T]) -> Result<Self> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: (data.len() * std::mem::size_of::<T>()) as UINT,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_UNORDERED_ACCESS,
            CPUAccessFlags: 0,
            MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED,
            StructureByteStride: std::mem::size_of::<T>() as UINT,
        };
        let inner = com_new(|x| unsafe {
            device.inner.CreateBuffer(
                &desc,
                &D3D11_SUBRESOURCE_DATA {
                    pSysMem: data.as_ptr() as *const _,
                    SysMemPitch: 0,
                    SysMemSlicePitch: 0,
                },
                x,
            )
        })?;
        let uav = com_new(|x| unsafe {
            device
                .inner
                .CreateUnorderedAccessView(inner.as_ptr() as *mut _, ptr::null(), x)
        })?;
        Ok(Self {
            inner,
            uav,
            _phantom: PhantomData,
        })
    }
}

#[derive(Clone)]
pub struct Dx11ConstantBuffer<T: Copy> {
    pub inner: ComPtr<ID3D11Buffer>,
    _phantom: PhantomData<T>,
}

impl<T: Copy> Dx11ConstantBuffer<T> {
    pub fn new_with_data(device: &Dx11Device, data: &[T]) -> Result<Self> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: (data.len() * std::mem::size_of::<T>()) as UINT,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: std::mem::size_of::<T>() as UINT,
        };
        let inner = com_new(|x| unsafe {
            device.inner.CreateBuffer(
                &desc,
                &D3D11_SUBRESOURCE_DATA {
                    pSysMem: data.as_ptr() as *const _,
                    SysMemPitch: 0,
                    SysMemSlicePitch: 0,
                },
                x,
            )
        })?;
        Ok(Self {
            inner,
            _phantom: PhantomData,
        })
    }

    pub fn replace(&self, ctx: &Dx11Context, data: &[T]) {
        unsafe {
            let stride = (data.len() * std::mem::size_of::<T>()) as UINT;
            ctx.inner.UpdateSubresource(
                self.inner.as_ptr() as *mut _,
                0,
                ptr::null(),
                data.as_ptr() as *const _,
                stride,
                stride,
            )
        }
    }
}

#[derive(Clone)]
pub struct Dx11ComputeShader {
    pub inner: ComPtr<ID3D11ComputeShader>,
}

impl Dx11ComputeShader {
    pub fn new(device: &Dx11Device, bytecode: &[u8]) -> Result<Self> {
        let inner = com_new(|x| unsafe {
            device.inner.CreateComputeShader(
                bytecode.as_ptr() as *const _,
                bytecode.len(),
                ptr::null_mut(),
                x,
            )
        })?;
        Ok(Self { inner })
    }
}
