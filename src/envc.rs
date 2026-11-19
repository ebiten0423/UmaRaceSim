use std::collections::HashMap;
use std::sync::OnceLock;
static JP:OnceLock<Vec<(String,String)>>=OnceLock::new();
static ALL:OnceLock<HashMap<String,String>>=OnceLock::new();
fn jp()->&'static [(String,String)] {JP.get_or_init(||std::env::vars().filter(|(k,_)|k.starts_with("JPRACE_")).collect())}
fn get(k:&str)->Option<&'static String> {
    if k.starts_with("JPRACE_") {jp().iter().find(|(a,_)|a==k).map(|(_,v)|v)}
    else {ALL.get_or_init(||std::env::vars().collect()).get(k)}
}
pub fn var(k:&str)->Result<String,std::env::VarError> {get(k).cloned().ok_or(std::env::VarError::NotPresent)}
pub fn var_os(k:&str)->Option<std::ffi::OsString> {get(k).map(|v|v.into())}
