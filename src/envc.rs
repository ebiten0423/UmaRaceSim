use std::collections::HashMap;
use std::sync::OnceLock;
static ENV:OnceLock<HashMap<String,String>>=OnceLock::new();
fn map()->&'static HashMap<String,String> {ENV.get_or_init(||std::env::vars().collect())}
pub fn var(k:&str)->Result<String,std::env::VarError> {map().get(k).cloned().ok_or(std::env::VarError::NotPresent)}
pub fn var_os(k:&str)->Option<std::ffi::OsString> {map().get(k).map(|v|v.into())}
