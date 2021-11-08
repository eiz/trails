use core::{ffi::c_void, fmt, mem, ptr};
use eiz::{
    com::{com_new, ComError, ComPtr},
    nvenc::sys::{
        GUID, NVENCAPI_MAJOR_VERSION, NVENCAPI_MINOR_VERSION, NVENCAPI_VERSION, NVENCSTATUS,
        NV_ENCODE_API_FUNCTION_LIST, NV_ENCODE_API_FUNCTION_LIST_VER, NV_ENC_BUFFER_FORMAT_ABGR10,
        NV_ENC_CODEC_HEVC_GUID, NV_ENC_CONFIG_VER, NV_ENC_CREATE_BITSTREAM_BUFFER,
        NV_ENC_CREATE_BITSTREAM_BUFFER_VER, NV_ENC_DEVICE_TYPE_DIRECTX,
        NV_ENC_ERR_DEVICE_NOT_EXIST, NV_ENC_ERR_ENCODER_BUSY, NV_ENC_ERR_ENCODER_NOT_INITIALIZED,
        NV_ENC_ERR_EVENT_NOT_REGISTERD, NV_ENC_ERR_GENERIC, NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY,
        NV_ENC_ERR_INVALID_CALL, NV_ENC_ERR_INVALID_DEVICE, NV_ENC_ERR_INVALID_ENCODERDEVICE,
        NV_ENC_ERR_INVALID_EVENT, NV_ENC_ERR_INVALID_PARAM, NV_ENC_ERR_INVALID_PTR,
        NV_ENC_ERR_INVALID_VERSION, NV_ENC_ERR_LOCK_BUSY, NV_ENC_ERR_MAP_FAILED,
        NV_ENC_ERR_NEED_MORE_INPUT, NV_ENC_ERR_NOT_ENOUGH_BUFFER, NV_ENC_ERR_NO_ENCODE_DEVICE,
        NV_ENC_ERR_OUT_OF_MEMORY, NV_ENC_ERR_RESOURCE_NOT_MAPPED,
        NV_ENC_ERR_RESOURCE_NOT_REGISTERED, NV_ENC_ERR_RESOURCE_REGISTER_FAILED,
        NV_ENC_ERR_UNIMPLEMENTED, NV_ENC_ERR_UNSUPPORTED_DEVICE, NV_ENC_ERR_UNSUPPORTED_PARAM,
        NV_ENC_HEVC_PROFILE_MAIN10_GUID, NV_ENC_INITIALIZE_PARAMS, NV_ENC_INITIALIZE_PARAMS_VER,
        NV_ENC_INPUT_IMAGE, NV_ENC_INPUT_PTR, NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX,
        NV_ENC_LOCK_BITSTREAM, NV_ENC_LOCK_BITSTREAM_VER, NV_ENC_MAP_INPUT_RESOURCE,
        NV_ENC_MAP_INPUT_RESOURCE_VER, NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS,
        NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER, NV_ENC_OUTPUT_PTR, NV_ENC_PIC_PARAMS,
        NV_ENC_PIC_PARAMS_VER, NV_ENC_PIC_STRUCT_FRAME, NV_ENC_PRESET_CONFIG,
        NV_ENC_PRESET_CONFIG_VER, NV_ENC_PRESET_HQ_GUID, NV_ENC_PRESET_LOW_LATENCY_HQ_GUID,
        NV_ENC_REGISTERED_PTR, NV_ENC_REGISTER_RESOURCE, NV_ENC_REGISTER_RESOURCE_VER,
        PNVENCODEAPICREATEINSTANCE, PNVENCODEAPIGETMAXSUPPORTEDVERSION,
        _NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CBR,
    },
};
use lazy_static::lazy_static;
use winapi::shared::dxgiformat::DXGI_FORMAT_R10G10B10A2_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::d3d11::{
    ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT,
};
use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};

trait ExtractOption {
    type Item;
}

impl<T> ExtractOption for Option<T> {
    type Item = T;
}

unsafe fn get_proc_addr<T: ExtractOption>(lib: HINSTANCE, symbol: &[u8]) -> Option<T::Item> {
    debug_assert!(symbol[symbol.len() - 1] == 0);
    let symbol = GetProcAddress(lib, symbol.as_ptr() as *const _);

    if symbol.is_null() {
        None
    } else {
        Some(mem::transmute_copy(&symbol))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EncoderError {
    NotSupported,
    VersionTooOld,
    MissingFunction,

    NoEncodeDevice,
    UnsupportedDevice,
    InvalidEncoderDevice,
    InvalidDevice,
    DeviceDoesNotExist,
    InvalidPointer,
    InvalidEvent,
    InvalidParam,
    InvalidCall,
    OutOfMemory,
    EncoderNotInitialized,
    UnsupportedParam,
    LockBusy,
    NotEnoughBuffer,
    InvalidVersion,
    MapFailed,
    NeedMoreInput,
    EncoderBusy,
    EventNotRegistered,
    Generic,
    IncompatibleClientKey,
    Unimplemented,
    ResourceRegisterFailed,
    ResourceNotRegistered,
    ResourceNotMapped,
    UnknownError(i32),
    Com(ComError),
}

impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl From<NVENCSTATUS> for EncoderError {
    fn from(val: NVENCSTATUS) -> EncoderError {
        match val {
            NV_ENC_ERR_NO_ENCODE_DEVICE => Self::NoEncodeDevice,
            NV_ENC_ERR_UNSUPPORTED_DEVICE => Self::UnsupportedDevice,
            NV_ENC_ERR_INVALID_ENCODERDEVICE => Self::InvalidEncoderDevice,
            NV_ENC_ERR_INVALID_DEVICE => Self::InvalidDevice,
            NV_ENC_ERR_DEVICE_NOT_EXIST => Self::DeviceDoesNotExist,
            NV_ENC_ERR_INVALID_PTR => Self::InvalidPointer,
            NV_ENC_ERR_INVALID_EVENT => Self::InvalidEvent,
            NV_ENC_ERR_INVALID_PARAM => Self::InvalidParam,
            NV_ENC_ERR_INVALID_CALL => Self::InvalidCall,
            NV_ENC_ERR_OUT_OF_MEMORY => Self::OutOfMemory,
            NV_ENC_ERR_ENCODER_NOT_INITIALIZED => Self::EncoderNotInitialized,
            NV_ENC_ERR_UNSUPPORTED_PARAM => Self::UnsupportedParam,
            NV_ENC_ERR_LOCK_BUSY => Self::LockBusy,
            NV_ENC_ERR_NOT_ENOUGH_BUFFER => Self::NotEnoughBuffer,
            NV_ENC_ERR_INVALID_VERSION => Self::InvalidVersion,
            NV_ENC_ERR_MAP_FAILED => Self::MapFailed,
            NV_ENC_ERR_NEED_MORE_INPUT => Self::NeedMoreInput,
            NV_ENC_ERR_ENCODER_BUSY => Self::EncoderBusy,
            NV_ENC_ERR_EVENT_NOT_REGISTERD => Self::EventNotRegistered,
            NV_ENC_ERR_GENERIC => Self::Generic,
            NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY => Self::IncompatibleClientKey,
            NV_ENC_ERR_UNIMPLEMENTED => Self::Unimplemented,
            NV_ENC_ERR_RESOURCE_REGISTER_FAILED => Self::ResourceRegisterFailed,
            NV_ENC_ERR_RESOURCE_NOT_REGISTERED => Self::ResourceNotRegistered,
            NV_ENC_ERR_RESOURCE_NOT_MAPPED => Self::ResourceNotMapped,
            x => Self::UnknownError(x),
        }
    }
}

impl From<ComError> for EncoderError {
    fn from(val: ComError) -> Self {
        Self::Com(val)
    }
}

struct NvidiaEncoderApi {
    api: NV_ENCODE_API_FUNCTION_LIST,
}

impl NvidiaEncoderApi {
    pub unsafe fn open_encode_session_ex(
        &self,
        params: &mut NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS,
        encoder: *mut *mut c_void,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncOpenEncodeSessionEx
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(params, encoder))
    }

    pub unsafe fn get_encode_preset_config(
        &self,
        encoder: *mut c_void,
        encode_guid: GUID,
        preset_guid: GUID,
        preset_config: *mut NV_ENC_PRESET_CONFIG,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncGetEncodePresetConfig
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, encode_guid, preset_guid, preset_config))
    }

    pub unsafe fn initialize_encoder(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_INITIALIZE_PARAMS,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncInitializeEncoder
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn register_resource(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_REGISTER_RESOURCE,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncRegisterResource
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn create_bitstream_buffer(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_CREATE_BITSTREAM_BUFFER,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncCreateBitstreamBuffer
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn map_input_resource(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_MAP_INPUT_RESOURCE,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncMapInputResource
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn encode_picture(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_PIC_PARAMS,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncEncodePicture
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn lock_bitstream(
        &self,
        encoder: *mut c_void,
        params: &mut NV_ENC_LOCK_BITSTREAM,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncLockBitstream
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, params))
    }

    pub unsafe fn unlock_bitstream(
        &self,
        encoder: *mut c_void,
        ptr: NV_ENC_OUTPUT_PTR,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncUnlockBitstream
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, ptr))
    }

    pub unsafe fn unmap_input_resource(
        &self,
        encoder: *mut c_void,
        ptr: NV_ENC_INPUT_PTR,
    ) -> Result<(), EncoderError> {
        let f = self
            .api
            .nvEncUnmapInputResource
            .ok_or(EncoderError::MissingFunction)?;

        invoke_nvenc(|| (f)(encoder, ptr))
    }
}

unsafe impl Sync for NvidiaEncoderApi {}
unsafe impl Send for NvidiaEncoderApi {}

fn invoke_nvenc<F>(f: F) -> Result<(), EncoderError>
where
    F: FnOnce() -> NVENCSTATUS,
{
    let status = (f)();

    if status == 0 {
        Ok(())
    } else {
        Err(EncoderError::from(status))
    }
}

unsafe fn init_nvenc_api() -> Result<NvidiaEncoderApi, EncoderError> {
    let lib = LoadLibraryA(b"nvEncodeAPI64\0".as_ptr() as *const _);

    if lib.is_null() {
        return Err(EncoderError::NotSupported);
    }

    let get_max_supported_version = get_proc_addr::<PNVENCODEAPIGETMAXSUPPORTEDVERSION>(
        lib,
        b"NvEncodeAPIGetMaxSupportedVersion\0",
    )
    .ok_or(EncoderError::MissingFunction)?;
    let create_instance =
        get_proc_addr::<PNVENCODEAPICREATEINSTANCE>(lib, b"NvEncodeAPICreateInstance\0")
            .ok_or(EncoderError::MissingFunction)?;
    let api_version = (NVENCAPI_MAJOR_VERSION << 4) | NVENCAPI_MINOR_VERSION;
    let mut max_version: u32 = 0;
    let mut api: NV_ENCODE_API_FUNCTION_LIST = mem::zeroed();

    api.version = NV_ENCODE_API_FUNCTION_LIST_VER;
    invoke_nvenc(|| (get_max_supported_version)(&mut max_version))?;

    if max_version < api_version {
        return Err(EncoderError::VersionTooOld);
    }

    invoke_nvenc(|| (create_instance)(&mut api))?;
    Ok(NvidiaEncoderApi { api })
}

lazy_static! {
    static ref NVENC_API: Result<NvidiaEncoderApi, EncoderError> = unsafe { init_nvenc_api() };
}

pub struct NvidiaH265Encoder {
    api: &'static NvidiaEncoderApi,
    width: u32,
    height: u32,
    encoder: *mut c_void,
    input_texture: ComPtr<ID3D11Texture2D>,
    input_registered: NV_ENC_REGISTERED_PTR,
    bitstream_buf: NV_ENC_OUTPUT_PTR,
}

impl NvidiaH265Encoder {
    pub fn new(
        device: ComPtr<ID3D11Device>,
        width: u32,
        height: u32,
    ) -> Result<Self, EncoderError> {
        unsafe {
            let api = NVENC_API.as_ref().map_err(|e| *e)?;
            let mut encoder = ptr::null_mut();
            let mut open_params: NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS = mem::zeroed();

            open_params.version = NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
            open_params.deviceType = NV_ENC_DEVICE_TYPE_DIRECTX;
            open_params.device = device.as_ptr() as *mut _;
            open_params.apiVersion = NVENCAPI_VERSION;
            api.open_encode_session_ex(&mut open_params, &mut encoder)?;

            let mut preset_config: NV_ENC_PRESET_CONFIG = mem::zeroed();

            preset_config.version = NV_ENC_PRESET_CONFIG_VER;
            preset_config.presetCfg.version = NV_ENC_CONFIG_VER;

            api.get_encode_preset_config(
                encoder,
                NV_ENC_CODEC_HEVC_GUID,
                NV_ENC_PRESET_LOW_LATENCY_HQ_GUID,
                &mut preset_config,
            )?;

            preset_config.presetCfg.profileGUID = NV_ENC_HEVC_PROFILE_MAIN10_GUID;
            preset_config
                .presetCfg
                .encodeCodecConfig
                .hevcConfig
                .set_pixelBitDepthMinus8(2);

            preset_config.presetCfg.rcParams.rateControlMode =
                _NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CBR;
            preset_config.presetCfg.rcParams.averageBitRate = 100 * 1000 * 1000;

            let vui = &mut preset_config
                .presetCfg
                .encodeCodecConfig
                .hevcConfig
                .hevcVUIParameters;

            vui.videoSignalTypePresentFlag = 1;
            vui.videoFormat = 5;
            vui.videoFullRangeFlag = 1;
            vui.colourDescriptionPresentFlag = 1;
            vui.colourPrimaries = 9; // rec 2020
            vui.transferCharacteristics = 16; // pq
            vui.colourMatrix = 9; // rec 2020

            let mut init_params: NV_ENC_INITIALIZE_PARAMS = mem::zeroed();

            init_params.version = NV_ENC_INITIALIZE_PARAMS_VER;
            init_params.encodeGUID = NV_ENC_CODEC_HEVC_GUID;
            init_params.presetGUID = NV_ENC_PRESET_HQ_GUID;
            init_params.encodeWidth = width;
            init_params.encodeHeight = height;
            init_params.frameRateNum = 60;
            init_params.frameRateDen = 1;
            init_params.enablePTD = 1;
            init_params.encodeConfig = &mut preset_config.presetCfg;
            api.initialize_encoder(encoder, &mut init_params)?;

            let texture_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R10G10B10A2_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };

            let texture = com_new(|x| device.CreateTexture2D(&texture_desc, ptr::null(), x))?;

            let mut register_resource_params: NV_ENC_REGISTER_RESOURCE = mem::zeroed();

            register_resource_params.version = NV_ENC_REGISTER_RESOURCE_VER;
            register_resource_params.resourceType = NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX;
            register_resource_params.width = width;
            register_resource_params.height = height;
            register_resource_params.resourceToRegister = texture.as_ptr() as *mut _;
            register_resource_params.bufferFormat = NV_ENC_BUFFER_FORMAT_ABGR10;
            register_resource_params.bufferUsage = NV_ENC_INPUT_IMAGE;
            api.register_resource(encoder, &mut register_resource_params)?;

            let mut create_bitstream_buffer: NV_ENC_CREATE_BITSTREAM_BUFFER = mem::zeroed();

            create_bitstream_buffer.version = NV_ENC_CREATE_BITSTREAM_BUFFER_VER;
            api.create_bitstream_buffer(encoder, &mut create_bitstream_buffer)?;

            Ok(Self {
                api,
                width,
                height,
                encoder,
                input_texture: texture,
                input_registered: register_resource_params.registeredResource,
                bitstream_buf: create_bitstream_buffer.bitstreamBuffer,
            })
        }
    }

    pub fn encode(&self) -> Result<EncodedFrame, EncoderError> {
        unsafe {
            let mut map_input_resource: NV_ENC_MAP_INPUT_RESOURCE = mem::zeroed();

            map_input_resource.version = NV_ENC_MAP_INPUT_RESOURCE_VER;
            map_input_resource.registeredResource = self.input_registered;
            self.api
                .map_input_resource(self.encoder, &mut map_input_resource)?;

            let mut pic_params: NV_ENC_PIC_PARAMS = mem::zeroed();

            pic_params.version = NV_ENC_PIC_PARAMS_VER;
            pic_params.inputWidth = self.width;
            pic_params.inputHeight = self.height;
            pic_params.inputBuffer = map_input_resource.mappedResource;
            pic_params.outputBitstream = self.bitstream_buf;
            pic_params.bufferFmt = NV_ENC_BUFFER_FORMAT_ABGR10;
            pic_params.pictureStruct = NV_ENC_PIC_STRUCT_FRAME;
            self.api.encode_picture(self.encoder, &mut pic_params)?;
            self.api
                .unmap_input_resource(self.encoder, map_input_resource.mappedResource)?;

            let mut lock_bitstream: NV_ENC_LOCK_BITSTREAM = mem::zeroed();

            lock_bitstream.version = NV_ENC_LOCK_BITSTREAM_VER;
            lock_bitstream.outputBitstream = self.bitstream_buf;
            self.api.lock_bitstream(self.encoder, &mut lock_bitstream)?;

            Ok(EncodedFrame {
                owner: self,
                data: std::slice::from_raw_parts(
                    lock_bitstream.bitstreamBufferPtr as *mut u8,
                    lock_bitstream.bitstreamSizeInBytes as usize,
                ),
            })
        }
    }

    pub fn texture(&self) -> &ComPtr<ID3D11Texture2D> {
        &self.input_texture
    }
}

pub struct EncodedFrame<'a> {
    owner: &'a NvidiaH265Encoder,
    data: &'a [u8],
}

impl<'a> EncodedFrame<'a> {
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> Drop for EncodedFrame<'a> {
    fn drop(&mut self) {
        unsafe {
            self.owner
                .api
                .unlock_bitstream(self.owner.encoder, self.owner.bitstream_buf)
                .unwrap();
        }
    }
}
