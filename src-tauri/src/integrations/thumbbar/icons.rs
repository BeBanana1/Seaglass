//! Icons of the thumbnail buttons as Win32 resources: one set per taskbar theme, made
//! from the pixels of `glyphs::render`.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};
use windows::core::w;

use super::glyphs::{self, Glyph};

/// Taskbar in the light theme (`SystemUsesLightTheme`); dark by default.
pub(super) fn taskbar_is_light() -> bool {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut size),
        )
    };
    status.is_ok() && value == 1
}

/// The four icons for one theme.
pub(super) struct Icons {
    pub(super) light: bool,
    previous: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

impl Icons {
    pub(super) fn new(light: bool) -> windows::core::Result<Self> {
        // Dark on a light taskbar, white on a dark one.
        let rgb = if light { [0x1F; 3] } else { [0xFF; 3] };
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let mut icons = Self {
            light,
            previous: HICON::default(),
            play: HICON::default(),
            pause: HICON::default(),
            next: HICON::default(),
        };
        // Filled one by one: on failure, `Drop` frees those already created.
        icons.previous = create_icon(Glyph::Previous, size, rgb)?;
        icons.play = create_icon(Glyph::Play, size, rgb)?;
        icons.pause = create_icon(Glyph::Pause, size, rgb)?;
        icons.next = create_icon(Glyph::Next, size, rgb)?;
        Ok(icons)
    }

    pub(super) fn get(&self, glyph: Glyph) -> HICON {
        match glyph {
            Glyph::Previous => self.previous,
            Glyph::Play => self.play,
            Glyph::Pause => self.pause,
            Glyph::Next => self.next,
        }
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.previous, self.play, self.pause, self.next] {
            if !icon.is_invalid() {
                let _ = unsafe { DestroyIcon(icon) };
            }
        }
    }
}

/// 32-bit icon (with alpha) from the pixels of `glyphs::render`.
fn create_icon(glyph: Glyph, size: i32, rgb: [u8; 3]) -> windows::core::Result<HICON> {
    let pixels = glyphs::render(glyph, size as u32, rgb);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            // Negative: rows top to bottom, like `render`.
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let color = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }?;
    if bits.is_null() {
        let _ = unsafe { DeleteObject(color.into()) };
        return Err(windows::core::Error::from_thread());
    }
    // SAFETY: the section is `size * size * 4` bytes, the exact size of `pixels`.
    unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len()) };
    // Empty monochrome mask (rows aligned on 16 bits): the alpha channel is what counts.
    let mask_bits = vec![0u8; (size as usize).div_ceil(16) * 2 * size as usize];
    let mask = unsafe { CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr().cast())) };
    let icon = unsafe {
        CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        })
    };
    // The icon keeps its own copies of the bitmaps.
    unsafe {
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
    }
    icon
}
