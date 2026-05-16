use cxx::UniquePtr;
use zim_sys::binding::ffi;

pub struct Uuid {
    ptr: UniquePtr<ffi::Uuid>,
}

impl Uuid {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Uuid>) -> Result<Uuid, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Uuid { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Uuid {
        self.ptr.as_ref().unwrap()
    }

    pub fn new_empty() -> Result<Uuid, ()> {
        Uuid::from_ptr(ffi::uuid_ctor())
    }

    /// This data should really only be 16 bytes long, but if it's longer it won't hurt, it will fail if shorter however
    pub fn new_from_bytes(uuid: &str) -> Result<Uuid, ()> {
        Uuid::from_ptr(ffi::uuid_ctor_str(uuid))
    }

    pub fn new_generate(value: &str) -> Result<Uuid, ()> {
        Uuid::from_ptr(ffi::uuid_generate(value))
    }
}

impl PartialEq for Uuid {
    fn eq(&self, other: &Self) -> bool {
        ffi::uuid_operator_eq(self.inner_ref(), other.inner_ref())
    }
}

impl Eq for Uuid {}

impl TryInto<String> for &Uuid {
    type Error = ();

    fn try_into(self) -> Result<String, Self::Error> {
        let res = ffi::uuid_std_string(self.inner_ref());
        match res.is_null() {
            true => Err(()),
            false => Ok(res.as_ref().unwrap().to_string()),
        }
    }
}
