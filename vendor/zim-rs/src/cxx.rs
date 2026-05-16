use cxx::{CxxString, CxxVector, UniquePtr};

pub(crate) fn string_from_ptr(ptr: UniquePtr<CxxString>) -> Result<String, ()> {
    match ptr.is_null() {
        true => Err(()),
        false => Ok(ptr.as_ref().unwrap().to_string()),
    }
}

pub(crate) fn vec_string_from_ptr(ptr: UniquePtr<CxxVector<CxxString>>) -> Result<Vec<String>, ()> {
    match ptr.is_null() {
        true => Err(()),
        false => Ok(ptr
            .as_ref()
            .unwrap()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()),
    }
}
