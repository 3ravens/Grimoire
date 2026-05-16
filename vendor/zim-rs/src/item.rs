use cxx::UniquePtr;
use zim_sys::binding::ffi;

use crate::{blob::Blob, cxx::string_from_ptr};

pub struct Item {
    ptr: UniquePtr<ffi::Item>,
}

// [SAFETY]
// Item is readonly and the data is valid as long as we keep a reference to ffi::Item
// (which is validated by `UniquePtr` and rust borrow rules).
// libzim itself is threadsafe, so it is valid to call libzim method from different threads.
unsafe impl Send for Item {}
unsafe impl Sync for Item {}

impl Item {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Item>) -> Result<Item, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Item { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Item {
        self.ptr.as_ref().unwrap()
    }

    pub fn get_title(&self) -> String {
        let title = ffi::item_getTitle(self.inner_ref());
        title
            .as_ref()
            .expect("Item::get_title should be infallible")
            .to_string()
    }

    pub fn get_path(&self) -> String {
        let path = ffi::item_getPath(self.inner_ref());
        path.as_ref()
            .expect("Item::get_path should be infallible")
            .to_string()
    }

    pub fn get_mimetype(&self) -> Result<String, ()> {
        string_from_ptr(ffi::item_getMimetype(self.inner_ref()))
    }

    pub fn get_data(&self) -> Result<Blob, ()> {
        Blob::from_ptr(ffi::item_getData(self.inner_ref()))
    }

    pub fn get_data_offset(&self, offset: u64, size: u64) -> Result<Blob, ()> {
        Blob::from_ptr(ffi::item_getData_offset(self.inner_ref(), offset, size))
    }

    pub fn get_direct_access(&self) -> Result<Option<DirectAccess>, ()> {
        let direct_access = DirectAccess::from_ptr(ffi::item_getDirectAccess(self.inner_ref()))?;
        Ok(if direct_access.get_path().is_empty() {
            None
        } else {
            Some(direct_access)
        })
    }

    pub fn get_size(&self) -> u64 {
        ffi::item_getSize(self.inner_ref())
    }

    pub fn get_index(&self) -> u32 {
        ffi::item_getIndex(self.inner_ref())
    }
}

pub struct DirectAccess {
    ptr: UniquePtr<ffi::DirectAccess>,
}

impl DirectAccess {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::DirectAccess>) -> Result<DirectAccess, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(DirectAccess { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::DirectAccess {
        self.ptr.as_ref().unwrap()
    }

    pub fn get_path(&self) -> String {
        let path = ffi::directAccess_getPath(self.inner_ref());
        path.as_ref()
            .expect("DirectAccess::get_path should be infallible")
            .to_string()
    }

    pub fn get_offset(&self) -> u64 {
        ffi::directAccess_getOffset(self.inner_ref())
    }
}
