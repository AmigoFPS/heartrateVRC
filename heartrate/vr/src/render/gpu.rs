#[derive(Debug)]
pub enum GpuError {
    #[cfg_attr(windows, allow(dead_code))]
    Unsupported,
    #[cfg(windows)]
    Api {
        context: &'static str,
        source: windows::core::Error,
    },
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::Unsupported => write!(f, "Direct3D 11 is only available on Windows"),
            #[cfg(windows)]
            GpuError::Api { context, source } => write!(f, "{context} failed: {source}"),
        }
    }
}

impl std::error::Error for GpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(windows)]
            GpuError::Api { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(windows)]
mod imp {
    use log::{debug, info};
    use windows::core::Interface;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
        D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_WRITE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD, D3D11_SDK_VERSION,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_DYNAMIC,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1};

    use super::GpuError;
    use crate::sys;

    pub struct GpuTexture {

        context: ID3D11DeviceContext,
        texture: ID3D11Texture2D,
        _device: ID3D11Device,
        width: u32,
        height: u32,
    }

    unsafe impl Send for GpuTexture {}

    impl GpuTexture {
        pub fn new(adapter_index: Option<u32>, width: u32, height: u32) -> Result<Self, GpuError> {
            let adapter = adapter_index.and_then(|i| match dxgi_adapter(i) {
                Ok(a) => Some(a),
                Err(e) => {
                    debug!("could not open DXGI adapter {i} ({e}); using the default");
                    None
                }
            });

            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;

            let driver_type = if adapter.is_some() {
                D3D_DRIVER_TYPE_UNKNOWN
            } else {
                D3D_DRIVER_TYPE_HARDWARE
            };

            unsafe {
                D3D11CreateDevice(
                    adapter.as_ref(),
                    driver_type,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )
            }
            .map_err(|source| GpuError::Api {
                context: "D3D11CreateDevice",
                source,
            })?;

            let device = device.ok_or(GpuError::Api {
                context: "D3D11CreateDevice returned no device",
                source: windows::core::Error::empty(),
            })?;
            let context = context.ok_or(GpuError::Api {
                context: "D3D11CreateDevice returned no context",
                source: windows::core::Error::empty(),
            })?;

            let texture = create_texture(&device, width, height)?;

            info!("Direct3D 11 overlay texture ready ({width}x{height}, adapter {adapter_index:?})");

            Ok(Self {
                context,
                texture,
                _device: device,
                width,
                height,
            })
        }

        pub fn sibling(&self, width: u32, height: u32) -> Result<Self, GpuError> {
            let texture = create_texture(&self._device, width, height)?;
            debug!("second Direct3D 11 overlay texture ready ({width}x{height})");

            Ok(Self {
                context: self.context.clone(),
                texture,
                _device: self._device.clone(),
                width,
                height,
            })
        }

        pub fn size(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        pub fn update(&self, pixels: &[u8]) -> Result<(), GpuError> {
            debug_assert_eq!(pixels.len(), (self.width as usize) * (self.height as usize) * 4);

            let resource: ID3D11Resource = self.texture.cast().map_err(|source| GpuError::Api {
                context: "ID3D11Texture2D::cast",
                source,
            })?;

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();

            unsafe {
                self.context
                    .Map(&resource, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
            }
            .map_err(|source| GpuError::Api {
                context: "ID3D11DeviceContext::Map",
                source,
            })?;

            let src_pitch = self.width as usize * 4;
            let dst_pitch = mapped.RowPitch as usize;

            unsafe {
                let base = mapped.pData as *mut u8;
                for row in 0..self.height as usize {
                    let src = &pixels[row * src_pitch..row * src_pitch + src_pitch];
                    std::ptr::copy_nonoverlapping(
                        src.as_ptr(),
                        base.add(row * dst_pitch),
                        src_pitch,
                    );
                }
                self.context.Unmap(&resource, 0);

                self.context.Flush();
            }

            Ok(())
        }

        pub fn as_openvr_texture(&self) -> sys::Texture_t {
            sys::Texture_t {
                handle: self.texture.as_raw(),
                eType: sys::ETextureType_DirectX,
                eColorSpace: sys::EColorSpace_Gamma,
            }
        }
    }

    fn create_texture(
        device: &ID3D11Device,
        width: u32,
        height: u32,
    ) -> Result<ID3D11Texture2D, GpuError> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,

            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;

        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(|source| {
            GpuError::Api {
                context: "CreateTexture2D",
                source,
            }
        })?;
        texture.ok_or(GpuError::Api {
            context: "CreateTexture2D returned no texture",
            source: windows::core::Error::empty(),
        })
    }

    fn dxgi_adapter(index: u32) -> Result<IDXGIAdapter, GpuError> {

        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.map_err(|source| GpuError::Api {
                context: "CreateDXGIFactory1",
                source,
            })?;

        unsafe { factory.EnumAdapters(index) }.map_err(|source| GpuError::Api {
            context: "IDXGIFactory1::EnumAdapters",
            source,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::GpuError;
    use crate::sys;

    pub struct GpuTexture;

    impl GpuTexture {
        pub fn new(_adapter: Option<u32>, _width: u32, _height: u32) -> Result<Self, GpuError> {
            Err(GpuError::Unsupported)
        }
        pub fn sibling(&self, _width: u32, _height: u32) -> Result<Self, GpuError> {
            Err(GpuError::Unsupported)
        }
        pub fn size(&self) -> (u32, u32) {
            (0, 0)
        }
        pub fn update(&self, _pixels: &[u8]) -> Result<(), GpuError> {
            Err(GpuError::Unsupported)
        }
        pub fn as_openvr_texture(&self) -> sys::Texture_t {
            sys::Texture_t {
                handle: std::ptr::null_mut(),
                eType: sys::ETextureType_Invalid,
                eColorSpace: sys::EColorSpace_Auto,
            }
        }
    }
}

pub use imp::GpuTexture;