use base64::{Engine, engine::general_purpose::STANDARD};
use notifyredirect_core::Result;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::*,
    },
    core::PCWSTR,
};
pub fn protect(text: &str) -> Result<String> {
    transform(text.as_bytes(), true).map(|b| STANDARD.encode(b))
}
pub fn unprotect(text: &str) -> Result<String> {
    let b = STANDARD
        .decode(text)
        .map_err(|_| "凭据格式无效".to_string())?;
    String::from_utf8(transform(&b, false)?).map_err(|_| "凭据解密无效".into())
}
fn transform(input: &[u8], seal: bool) -> Result<Vec<u8>> {
    let data = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let r = if seal {
            CryptProtectData(
                &data,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        } else {
            CryptUnprotectData(
                &data,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        r.map_err(|_| "Windows 凭据保护失败，请重新配置".to_string())?;
        let value = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData.cast())));
        Ok(value)
    }
}
