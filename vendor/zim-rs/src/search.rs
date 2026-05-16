use cxx::UniquePtr;
use zim_sys::binding::ffi;

use crate::{archive::Archive, entry::Entry};

pub struct Searcher {
    ptr: UniquePtr<ffi::Searcher>,
}

pub struct Query {
    ptr: UniquePtr<ffi::Query>,
}

pub struct Search {
    ptr: UniquePtr<ffi::Search>,
}

pub struct SearchResultSet {
    ptr: UniquePtr<ffi::SearchResultSet>,
}

pub struct SearchIterator {
    cur: UniquePtr<ffi::SearchIterator>,
    end: UniquePtr<ffi::SearchIterator>,
    size: usize,
}

impl Searcher {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Searcher>) -> Result<Searcher, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Searcher { ptr }),
        }
    }

    pub fn new(archive: &Archive) -> Result<Searcher, ()> {
        Searcher::from_ptr(ffi::searcher_ctor(archive.inner_ref()))
    }

    pub fn add_archive(&mut self, archive: &Archive) {
        ffi::searcher_addArchive(self.ptr.pin_mut(), archive.inner_ref())
    }

    pub fn search(&mut self, query: &Query) -> Result<Search, ()> {
        Search::from_ptr(ffi::searcher_search(self.ptr.pin_mut(), query.inner_ref()))
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        ffi::searcher_setVerbose(self.ptr.pin_mut(), verbose);
    }
}

impl Query {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Query>) -> Result<Query, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Query { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Query {
        self.ptr.as_ref().unwrap()
    }

    pub fn new(query: &str) -> Result<Query, ()> {
        Query::from_ptr(ffi::query_ctor(query))
    }

    pub fn set_query(&mut self, query: &str) {
        ffi::query_setQuery(self.ptr.pin_mut(), query)
    }

    pub fn set_georange(&mut self, latitude: f32, longitude: f32, distance: f32) {
        ffi::query_setGeorange(self.ptr.pin_mut(), latitude, longitude, distance)
    }
}

impl Search {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Search>) -> Result<Search, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Search { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Search {
        self.ptr.as_ref().unwrap()
    }

    pub fn get_results(&self, start: i32, max_results: i32) -> Result<SearchResultSet, ()> {
        SearchResultSet::from_ptr(ffi::search_getResults(self.inner_ref(), start, max_results))
    }

    pub fn get_estimated_matches(&self) -> i32 {
        ffi::search_getEstimatedMatches(self.inner_ref())
    }
}

impl SearchResultSet {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::SearchResultSet>) -> Result<SearchResultSet, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(SearchResultSet { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::SearchResultSet {
        self.ptr.as_ref().unwrap()
    }
}

impl IntoIterator for SearchResultSet {
    type Item = Result<Entry, ()>;

    type IntoIter = SearchIterator;

    fn into_iter(self) -> Self::IntoIter {
        match SearchIterator::from_searchresultset(self) {
            Ok(iterator) => iterator,
            Err(_) => SearchIterator::default(),
        }
    }
}

impl SearchIterator {
    pub(crate) fn from_searchresultset(set: SearchResultSet) -> Result<SearchIterator, ()> {
        let cur = ffi::searchresultset_begin(set.inner_ref());
        let end = ffi::searchresultset_end(set.inner_ref());
        let size = match ffi::searchresultset_size(set.inner_ref()) {
            n if n < 0 => 0,
            x => x as usize,
        };

        match (cur.is_null(), end.is_null()) {
            (false, false) => Ok(SearchIterator { cur, end, size }),
            _ => Err(()),
        }
    }

    fn cur_ref(&self) -> &ffi::SearchIterator {
        self.cur.as_ref().unwrap()
    }

    fn end_ref(&self) -> &ffi::SearchIterator {
        self.end.as_ref().unwrap()
    }
}

impl Default for SearchIterator {
    fn default() -> Self {
        Self {
            cur: UniquePtr::null(),
            end: UniquePtr::null(),
            size: 0,
        }
    }
}

impl Iterator for SearchIterator {
    type Item = Result<Entry, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let at_end =
            self.size == 0 || ffi::searchiterator_operator_eq(self.cur_ref(), self.end_ref());
        match at_end {
            true => None,
            false => {
                let item = Entry::from_ptr(ffi::searchiterator_operator_star(self.cur_ref()));
                ffi::searchiterator_operator_inc(self.cur.pin_mut());
                Some(item)
            }
        }
    }
}
