use cxx::UniquePtr;
use zim_sys::binding::ffi;

use crate::{archive::Archive, cxx::string_from_ptr};

pub struct SuggestionSearcher {
    ptr: UniquePtr<ffi::SuggestionSearcher>,
}

pub struct SuggestionSearch {
    ptr: UniquePtr<ffi::SuggestionSearch>,
}

pub struct SuggestionResultSet {
    ptr: UniquePtr<ffi::SuggestionResultSet>,
}

pub struct SuggestionIterator {
    cur: UniquePtr<ffi::SuggestionIterator>,
    end: UniquePtr<ffi::SuggestionIterator>,
    size: usize,
}

pub struct SuggestionItem {
    ptr: UniquePtr<ffi::SuggestionItem>,
}

impl SuggestionSearcher {
    pub(crate) fn from_ptr(
        ptr: UniquePtr<ffi::SuggestionSearcher>,
    ) -> Result<SuggestionSearcher, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(SuggestionSearcher { ptr }),
        }
    }

    pub fn new(archive: &Archive) -> Result<SuggestionSearcher, ()> {
        SuggestionSearcher::from_ptr(ffi::suggestionsearcher_ctor(archive.inner_ref()))
    }

    pub fn suggest(&mut self, query: &str) -> Result<SuggestionSearch, ()> {
        SuggestionSearch::from_ptr(ffi::suggestionsearcher_suggest(self.ptr.pin_mut(), query))
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        ffi::suggestionsearcher_setVerbose(self.ptr.pin_mut(), verbose);
    }
}

impl SuggestionSearch {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::SuggestionSearch>) -> Result<SuggestionSearch, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(SuggestionSearch { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::SuggestionSearch {
        self.ptr.as_ref().unwrap()
    }

    pub fn get_results(&self, start: i32, max_results: i32) -> Result<SuggestionResultSet, ()> {
        SuggestionResultSet::from_ptr(ffi::suggestionsearch_getResults(
            self.inner_ref(),
            start,
            max_results,
        ))
    }

    pub fn get_estimated_matches(&self) -> i32 {
        ffi::suggestionsearch_getEstimatedMatches(self.inner_ref())
    }
}

impl SuggestionResultSet {
    pub(crate) fn from_ptr(
        ptr: UniquePtr<ffi::SuggestionResultSet>,
    ) -> Result<SuggestionResultSet, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(SuggestionResultSet { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::SuggestionResultSet {
        self.ptr.as_ref().unwrap()
    }
}

impl IntoIterator for SuggestionResultSet {
    type Item = Result<SuggestionItem, ()>;

    type IntoIter = SuggestionIterator;

    fn into_iter(self) -> Self::IntoIter {
        match SuggestionIterator::from_searchresultset(self) {
            Ok(iterator) => iterator,
            Err(_) => SuggestionIterator::default(),
        }
    }
}

impl SuggestionIterator {
    pub(crate) fn from_searchresultset(set: SuggestionResultSet) -> Result<SuggestionIterator, ()> {
        let cur = ffi::suggestionresultset_begin(set.inner_ref());
        let end = ffi::suggestionresultset_end(set.inner_ref());
        let size = match ffi::suggestionresultset_size(set.inner_ref()) {
            n if n < 0 => 0,
            x => x as usize,
        };

        match (cur.is_null(), end.is_null()) {
            (false, false) => Ok(SuggestionIterator { cur, end, size }),
            _ => Err(()),
        }
    }

    fn cur_ref(&self) -> &ffi::SuggestionIterator {
        self.cur.as_ref().unwrap()
    }

    fn end_ref(&self) -> &ffi::SuggestionIterator {
        self.end.as_ref().unwrap()
    }
}

impl Default for SuggestionIterator {
    fn default() -> Self {
        Self {
            cur: UniquePtr::null(),
            end: UniquePtr::null(),
            size: 0,
        }
    }
}

impl Iterator for SuggestionIterator {
    type Item = Result<SuggestionItem, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let at_end =
            self.size == 0 || ffi::suggestioniterator_operator_eq(self.cur_ref(), self.end_ref());
        match at_end {
            true => None,
            false => {
                let item = SuggestionItem::from_ptr(ffi::suggestioniterator_operator_star(
                    self.cur.pin_mut(),
                ));
                ffi::suggestioniterator_operator_inc(self.cur.pin_mut());
                Some(item)
            }
        }
    }
}

impl SuggestionItem {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::SuggestionItem>) -> Result<SuggestionItem, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(SuggestionItem { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::SuggestionItem {
        self.ptr.as_ref().unwrap()
    }

    pub fn get_title(&self) -> String {
        string_from_ptr(ffi::suggestionitem_getTitle(self.inner_ref()))
            .expect("SuggestionItem::get_title should be infallible")
    }

    pub fn get_path(&self) -> String {
        string_from_ptr(ffi::suggestionitem_getPath(self.inner_ref()))
            .expect("SuggestionItem::get_path should be infallible")
    }

    pub fn get_snippet(&self) -> String {
        string_from_ptr(ffi::suggestionitem_getSnippet(self.inner_ref()))
            .expect("SuggestionItem::get_snippet should be infallible")
    }

    pub fn has_snippet(&self) -> bool {
        ffi::suggestionitem_hasSnippet(self.inner_ref())
    }
}
