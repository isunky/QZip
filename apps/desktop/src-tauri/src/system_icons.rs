use std::collections::{HashMap, HashSet};

pub(super) fn normalize_system_icon_extension(value: &str) -> Option<String> {
    let extension = value.trim().trim_start_matches('.').to_ascii_lowercase();
    (!extension.is_empty()
        && extension.len() <= 32
        && extension.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '+')
        }))
    .then_some(extension)
}

#[cfg(target_os = "windows")]
fn system_icon_data_url(extension: &str) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use windows_sys::Win32::{
        Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW},
    };

    let path = format!("qzip.{extension}");
    let wide_path = path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut file_info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            wide_path.as_ptr(),
            FILE_ATTRIBUTE_NORMAL,
            &mut file_info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_USEFILEATTRIBUTES,
        )
    };
    if result == 0 || file_info.hIcon.is_null() {
        return None;
    }

    let png = unsafe { encode_windows_icon_as_png(file_info.hIcon) };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(file_info.hIcon);
    }
    png.map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(target_os = "windows")]
unsafe fn encode_windows_icon_as_png(
    icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<Vec<u8>> {
    use image::ImageEncoder as _;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, SelectObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx};

    const ICON_SIZE: usize = 32;
    let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return None;
    }

    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bitmap_info.bmiHeader.biWidth = ICON_SIZE as i32;
    bitmap_info.bmiHeader.biHeight = -(ICON_SIZE as i32);
    bitmap_info.bmiHeader.biPlanes = 1;
    bitmap_info.bmiHeader.biBitCount = 32;
    bitmap_info.bmiHeader.biCompression = BI_RGB;
    let mut bits = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(
            dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if bitmap.is_null() || bits.is_null() {
        unsafe { DeleteDC(dc) };
        return None;
    }

    let previous = unsafe { SelectObject(dc, bitmap) };
    let byte_count = ICON_SIZE * ICON_SIZE * 4;
    unsafe { std::ptr::write_bytes(bits, 0, byte_count) };
    let drawn = unsafe {
        DrawIconEx(
            dc,
            0,
            0,
            icon,
            ICON_SIZE as i32,
            ICON_SIZE as i32,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        )
    } != 0;
    let bgra = unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), byte_count) }.to_vec();
    if !previous.is_null() {
        unsafe { SelectObject(dc, previous) };
    }
    unsafe {
        DeleteObject(bitmap);
        DeleteDC(dc);
    }
    if !drawn {
        return None;
    }

    let has_alpha = bgra.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mut rgba = Vec::with_capacity(byte_count);
    for pixel in bgra.chunks_exact(4) {
        let alpha = if has_alpha {
            pixel[3]
        } else if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
            u8::MAX
        } else {
            0
        };
        let expand = |channel: u8| {
            if has_alpha && alpha > 0 && alpha < u8::MAX {
                ((u16::from(channel) * 255 / u16::from(alpha)).min(255)) as u8
            } else {
                channel
            }
        };
        rgba.extend_from_slice(&[expand(pixel[2]), expand(pixel[1]), expand(pixel[0]), alpha]);
    }

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &rgba,
            ICON_SIZE as u32,
            ICON_SIZE as u32,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(png)
}

#[cfg(not(target_os = "windows"))]
fn system_icon_data_url(_extension: &str) -> Option<String> {
    None
}

#[tauri::command]
pub(super) async fn get_system_file_icons(extensions: Vec<String>) -> HashMap<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut seen = HashSet::new();
        let mut icons = HashMap::new();
        for extension in extensions
            .into_iter()
            .filter_map(|value| normalize_system_icon_extension(&value))
        {
            if !seen.insert(extension.clone()) {
                continue;
            }
            if seen.len() > 128 {
                break;
            }
            if let Some(icon) = system_icon_data_url(&extension) {
                icons.insert(extension, icon);
            }
        }
        icons
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{normalize_system_icon_extension, system_icon_data_url};

    #[test]
    fn normalizes_safe_file_extensions() {
        assert_eq!(
            normalize_system_icon_extension(" .DOCX "),
            Some("docx".into())
        );
        assert_eq!(
            normalize_system_icon_extension("tar-gz"),
            Some("tar-gz".into())
        );
    }

    #[test]
    fn rejects_paths_and_unbounded_extensions() {
        assert_eq!(normalize_system_icon_extension("../exe"), None);
        assert_eq!(normalize_system_icon_extension(&"x".repeat(33)), None);
        assert_eq!(normalize_system_icon_extension(""), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn renders_a_shell_file_icon_as_png_data() {
        let icon = system_icon_data_url("txt").expect("Windows supplies a text-file icon");
        assert!(icon.starts_with("data:image/png;base64,iVBORw0KGgo"));
    }
}
