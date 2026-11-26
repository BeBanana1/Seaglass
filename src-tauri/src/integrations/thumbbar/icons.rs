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

/// Barre des tâches en thème clair (`SystemUsesLightTheme`) ; sombre par défaut.
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

/// Les quatre icônes pour un thème.
pub(super) struct Icons {
    pub(super) light: bool,
    previous: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

impl Icons {
    pub(super) fn new(light: bool) -> windows::core::Result<Self> {
        // Sombre sur une barre claire, blanc sur une barre sombre.
        let rgb = if light { [0x1F; 3] } else { [0xFF; 3] };
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let mut icons = Self {
            light,
            previous: HICON::default(),
            play: HICON::default(),
            pause: HICON::default(),
            next: HICON::default(),
        };
        // Remplies une à une : en cas d'échec, `Drop` libère celles déjà créées.
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

/// Icône 32 bits (alpha) depuis les pixels de `glyphs::render`.
fn create_icon(glyph: Glyph, size: i32, rgb: [u8; 3]) -> windows::core::Result<HICON> {
    let pixels = glyphs::render(glyph, size as u32, rgb);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            // Négatif : lignes de haut en bas, comme `render`.
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
    // SAFETY : la section fait `size * size * 4` octets, la taille exacte de `pixels`.
    unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len()) };
    // Masque monochrome vide (lignes alignées sur 16 bits) : l'alpha fait foi.
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
    // L'icône garde ses propres copies des bitmaps.
    unsafe {
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
    }
    icon
}
