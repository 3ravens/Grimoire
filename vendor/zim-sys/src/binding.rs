// entry_index_type = u32
// cluster_index_type = u32
// blob_index_type = u32
// size_type = u64
// offset_type = u64

#[cxx::bridge]
pub mod ffi {
    extern "C++" {
        pub type EntryRangeEfficient;
        pub type IterEfficient;
        pub type DirectAccess;
    }

    #[namespace = "zim"]
    extern "C++" {
        pub type Archive;
        pub type Blob;
        pub type Entry;
        pub type Item;


        pub type SuggestionSearcher;
        pub type SuggestionSearch;
        pub type SuggestionResultSet;
        pub type SuggestionIterator;
        pub type SuggestionItem;

        pub type Uuid;
    }

    unsafe extern "C++" {
        include!("zim-sys/zim-bind.h");

        // FILE: archive.h
        pub fn archive_ctor_file(path: &str) -> UniquePtr<Archive>;
        pub fn archive_getFilename(archive: &Archive) -> &CxxString;
        pub fn archive_getFilesize(archive: &Archive) -> u64;
        pub fn archive_getAllEntryCount(archive: &Archive) -> u32;
        pub fn archive_getEntryCount(archive: &Archive) -> u32;
        pub fn archive_getArticleCount(archive: &Archive) -> u32;
        pub fn archive_getUuid(archive: &Archive) -> UniquePtr<Uuid>;
        pub fn archive_getMetadata(archive: &Archive, name: &str) -> UniquePtr<CxxString>;
        pub fn archive_getMetadataItem(archive: &Archive, name: &str) -> UniquePtr<Item>;
        pub fn archive_getMetadataKeys(archive: &Archive) -> UniquePtr<CxxVector<CxxString>>;
        pub fn archive_getEntryByPath_idx(archive: &Archive, idx: u32) -> UniquePtr<Entry>;
        pub fn archive_getEntryByPath_str(archive: &Archive, path: &str) -> UniquePtr<Entry>;
        pub fn archive_getEntryByTitle_idx(archive: &Archive, idx: u32) -> UniquePtr<Entry>;
        pub fn archive_getEntryByTitle_str(archive: &Archive, title: &str) -> UniquePtr<Entry>;
        pub fn archive_getMainEntry(archive: &Archive) -> UniquePtr<Entry>;
        pub fn archive_getRandomEntry(archive: &Archive) -> UniquePtr<Entry>;
        pub fn archive_hasEntryByPath(archive: &Archive, path: &str) -> bool;
        pub fn archive_hasEntryByTitle(archive: &Archive, title: &str) -> bool;
        pub fn archive_hasMainEntry(archive: &Archive) -> bool;
        pub fn archive_hasFulltextIndex(archive: &Archive) -> bool;
        pub fn archive_hasTitleIndex(archive: &Archive) -> bool;
        pub fn archive_hasChecksum(archive: &Archive) -> bool;
        pub fn archive_getChecksum(archive: &Archive) -> UniquePtr<CxxString>;
        pub fn archive_check(archive: &Archive) -> bool;
        pub fn archive_isMultiPart(archive: &Archive) -> bool;
        pub fn archive_hasNewNamespaceScheme(archive: &Archive) -> bool;
        pub fn archive_iterEfficient(archive: &Archive) -> UniquePtr<EntryRangeEfficient>;

        pub fn entryrangeefficient_begin(range: &EntryRangeEfficient) -> UniquePtr<IterEfficient>;
        pub fn entryrangeefficient_end(range: &EntryRangeEfficient) -> UniquePtr<IterEfficient>;

        pub fn iterefficient_eq(iter: &IterEfficient, other: &IterEfficient) -> bool;
        pub fn iterefficient_star(iter: &IterEfficient) -> UniquePtr<Entry>;
        pub fn iterefficient_inc(iter: Pin<&mut IterEfficient>);


        // FILE: blob.h
        pub fn blob_ctor() -> UniquePtr<Blob>;
        pub fn blob_data(blob: &Blob) -> *const c_char;
        pub fn blob_size(blob: &Blob) -> u64;

        // FILE: entry.h
        pub fn entry_isRedirect(entry: &Entry) -> bool;
        pub fn entry_getTitle(entry: &Entry) -> UniquePtr<CxxString>;
        pub fn entry_getPath(entry: &Entry) -> UniquePtr<CxxString>;
        pub fn entry_getItem(entry: &Entry, follow: bool) -> UniquePtr<Item>;
        pub fn entry_getRedirect(entry: &Entry) -> UniquePtr<Item>;
        pub fn entry_getRedirectEntry(entry: &Entry) -> UniquePtr<Entry>;
        //pub fn entry_getRedirectEntryIndex(entry: &Entry) -> u32;
        pub fn entry_getIndex(entry: &Entry) -> u32;

        // FILE: item.h
        pub fn item_getTitle(item: &Item) -> UniquePtr<CxxString>;
        pub fn item_getPath(item: &Item) -> UniquePtr<CxxString>;
        pub fn item_getMimetype(item: &Item) -> UniquePtr<CxxString>;
        pub fn item_getData(item: &Item) -> UniquePtr<Blob>;
        pub fn item_getData_offset(item: &Item, offset: u64, size: u64) -> UniquePtr<Blob>;
        pub fn item_getDirectAccess(item: &Item) -> UniquePtr<DirectAccess>;
        pub fn item_getSize(item: &Item) -> u64;
        pub fn item_getIndex(item: &Item) -> u32;

        // Wrapper DirectAccess
        pub fn directAccess_getPath(dAccess: &DirectAccess) -> UniquePtr<CxxString>;
        pub fn directAccess_getOffset(dAccess: &DirectAccess) -> u64;

        // FILE: suggestion.h
        pub fn suggestionsearcher_ctor(archive: &Archive) -> UniquePtr<SuggestionSearcher>;
        pub fn suggestionsearcher_suggest(
            suggestionsearcher: Pin<&mut SuggestionSearcher>,
            query: &str,
        ) -> UniquePtr<SuggestionSearch>;
        pub fn suggestionsearcher_setVerbose(
            suggestionsearcher: Pin<&mut SuggestionSearcher>,
            verbose: bool,
        );

        pub fn suggestionsearch_getResults(
            suggestionsearch: &SuggestionSearch,
            start: i32,
            maxResults: i32,
        ) -> UniquePtr<SuggestionResultSet>;
        pub fn suggestionsearch_getEstimatedMatches(suggestionsearch: &SuggestionSearch) -> i32;

        pub fn suggestionresultset_begin(
            suggestionresultset: &SuggestionResultSet,
        ) -> UniquePtr<SuggestionIterator>;
        pub fn suggestionresultset_end(
            suggestionresultset: &SuggestionResultSet,
        ) -> UniquePtr<SuggestionIterator>;
        pub fn suggestionresultset_size(suggestionresultset: &SuggestionResultSet) -> i32;

        // FILE: suggestion_iterator.h
        pub fn suggestioniterator_operator_eq(
            suggestioniterator: &SuggestionIterator,
            o: &SuggestionIterator,
        ) -> bool;
        pub fn suggestioniterator_operator_neq(
            suggestioniterator: &SuggestionIterator,
            o: &SuggestionIterator,
        ) -> bool;
        pub fn suggestioniterator_operator_inc(suggestioniterator: Pin<&mut SuggestionIterator>);
        pub fn suggestioniterator_operator_star(
            suggestioniterator: Pin<&mut SuggestionIterator>,
        ) -> UniquePtr<SuggestionItem>;
        pub fn suggestioniterator_getEntry(
            suggestioniterator: &SuggestionIterator,
        ) -> UniquePtr<Entry>;

        pub fn suggestionitem_getTitle(suggestionitem: &SuggestionItem) -> UniquePtr<CxxString>;
        pub fn suggestionitem_getPath(suggestionitem: &SuggestionItem) -> UniquePtr<CxxString>;
        pub fn suggestionitem_getSnippet(suggestionitem: &SuggestionItem) -> UniquePtr<CxxString>;
        pub fn suggestionitem_hasSnippet(suggestionitem: &SuggestionItem) -> bool;

        // FILE: uuid.h
        pub fn uuid_ctor() -> UniquePtr<Uuid>;
        pub fn uuid_ctor_str(uuid: &str) -> UniquePtr<Uuid>;
        pub fn uuid_generate(value: &str) -> UniquePtr<Uuid>;
        pub fn uuid_operator_eq(uuid: &Uuid, o: &Uuid) -> bool;
        pub fn uuid_std_string(uuid: &Uuid) -> UniquePtr<CxxString>;
    }
}
