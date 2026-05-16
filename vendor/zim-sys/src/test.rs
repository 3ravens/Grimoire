use std::{pin::Pin, ptr::null};

use crate::binding::ffi::{suggestionitem_getTitle, suggestioniterator_operator_star};

use super::binding::ffi;

pub static wikt: &str = "/home/aka/Downloads/wiktionary_en_all_maxi_2022-09.zim";

/// Open a non-empty archive and test archive functions which should work
#[test]
fn archive_open_everything() {
    println!("Archive Test:");
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let size = ffi::archive_getFilesize(archive);
    println!("  Size: {}", size);
    assert!(size != 0);

    let file = ffi::archive_getFilename(archive);
    println!("  File: {}", file.to_string());

    let all_entry_count = ffi::archive_getAllEntryCount(archive);
    println!("  All Entry Count: {}", all_entry_count);

    let entry_count = ffi::archive_getEntryCount(archive);
    println!("  Entry Count: {}", entry_count);

    let article_count = ffi::archive_getArticleCount(archive);
    println!("  Article Count: {}", article_count);

    let metadata_keys = ffi::archive_getMetadataKeys(archive);
    assert!(!metadata_keys.is_null());
    print!("    Metadata Keys: ");
    for key in metadata_keys.as_ref().unwrap().iter() {
        print!("{} ", key);
    }
    println!("");
}

/// Try to open archive that doesn't exist
#[test]
fn archive_open_failure() {
    let archive = ffi::archive_ctor_file("////broken..////path///");
    assert!(archive.is_null());
}

// Try some invalid metadata queries
#[test]
fn archive_metadata_failure() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let metadata = ffi::archive_getMetadata(archive, "ThisShouldFail!!!");
    assert!(metadata.is_null());

    let metadata2 = ffi::archive_getMetadataItem(archive, "ThisAlsoShouldFail!!!");
    assert!(metadata2.is_null());
}

// Try some invalid entry lookups
#[test]
fn archive_get_entry_failure() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let entry = ffi::archive_getEntryByPath_str(archive, "ABC/DEF/brokenbrokenborkenSHOULDNTEXIST");
    assert!(entry.is_null());

    let entry2 = ffi::archive_getEntryByTitle_str(archive, "brokenbrokenborkenSHOULDNTEXIST");
    assert!(entry2.is_null());
}

// Test the uuid, assume the archive uuid is non-zero
#[test]
fn archive_test_uuid() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let uuid = ffi::archive_getUuid(archive);
    assert!(!uuid.is_null());
    let uuid = uuid.as_ref().unwrap();

    let zero_uuid = ffi::uuid_ctor();
    assert!(!zero_uuid.is_null());
    let zero_uuid = zero_uuid.as_ref().unwrap();

    // compare the uuids, the archive uuid shouldn't be zero
    assert!(!ffi::uuid_operator_eq(uuid, zero_uuid));
}

// Trivial blob test, create and empty blob and check it's data ptr
#[test]
fn blob_trivial() {
    let blob = ffi::blob_ctor();
    assert!(!blob.is_null());
    let blob = blob.as_ref().unwrap();

    unsafe {
        let data = ffi::blob_data(blob);
        assert!(data == null());
    }

    let size = ffi::blob_size(blob);
    assert!(size == 0);
}

// Entry test using the main page
#[test]
fn entry_mainpage() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    println!("Main page: ");
    let page = ffi::archive_getMainEntry(archive);
    assert!(!page.is_null());
    let page = page.as_ref().unwrap();

    let is_redirect = ffi::entry_isRedirect(page);
    assert!(!is_redirect);

    let title = ffi::entry_getTitle(page);
    assert!(!title.is_null());
    println!("  Title: {}", title.as_ref().unwrap());
}

// Try to open the main page item and get some information about it
#[test]
fn item_mainpage() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let page = ffi::archive_getMainEntry(archive);
    assert!(!page.is_null());
    let page = page.as_ref().unwrap();

    let item = ffi::entry_getItem(page, false);
    assert!(!item.is_null());
    let item = item.as_ref().unwrap();

    println!("Main page item: ");
    // finally can do item tests here
    let title = ffi::item_getTitle(item);
    assert!(!title.is_null());
    println!("  Title: {}", title.as_ref().unwrap());

    let path = ffi::item_getPath(item);
    assert!(!path.is_null());
    println!("  Path: {}", path.as_ref().unwrap());

    let mimetype = ffi::item_getMimetype(item);
    assert!(!mimetype.is_null());
    println!("  Mimetype: {}", mimetype.as_ref().unwrap());

    let size = ffi::item_getSize(item);
    println!("  Size: {}", size);

    let index = ffi::item_getIndex(item);
    println!("  Index: {}", index);
}

// Try to search for 'name' in wiktionary zim
#[test]
fn search_test() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let mut searcher = ffi::searcher_ctor(archive);
    assert!(!searcher.is_null());
    let mut searcher = searcher.as_mut().unwrap();

    let query = ffi::query_ctor("name");
    assert!(!query.is_null());
    let query = query.as_ref().unwrap();

    let search = ffi::searcher_search(searcher, query);
    assert!(!search.is_null());
    let search = search.as_ref().unwrap();

    let results = ffi::search_getResults(search, 0, 1);
    assert!(!results.is_null());
    let results = results.as_ref().unwrap();

    let size = ffi::searchresultset_size(results);
    assert!(size == 1);
}

// Try to search suggestions for 'name' in wiktionary zim
#[test]
fn suggestion_test() {
    let archive = ffi::archive_ctor_file(wikt);
    assert!(!archive.is_null());
    let archive = archive.as_ref().unwrap();

    let mut searcher = ffi::suggestionsearcher_ctor(archive);
    assert!(!searcher.is_null());
    let mut searcher = searcher.as_mut().unwrap();

    let search = ffi::suggestionsearcher_suggest(searcher, "name");
    assert!(!search.is_null());
    let search = search.as_ref().unwrap();

    let results = ffi::suggestionsearch_getResults(search, 0, 1);
    assert!(!results.is_null());
    let results = results.as_ref().unwrap();

    let size = ffi::suggestionresultset_size(results);
    assert!(size == 1);

    let mut first = ffi::suggestionresultset_begin(&results);
    assert!(!first.is_null());

    let item = suggestioniterator_operator_star(first.pin_mut());
    assert!(!item.is_null());
    let item = item.as_ref().unwrap();

    let title = suggestionitem_getTitle(item);
    assert!(!title.is_null());

    println!("Found: '{}'", title.to_string());
}

// Test making a trivial uuid and reading it back
#[test]
fn uuid_test_zeros() {
    let uuid = ffi::uuid_ctor();
    assert!(!uuid.is_null());
    let uuid = uuid.as_ref().unwrap();

    let uuid_str = ffi::uuid_std_string(uuid);
    assert!(!uuid_str.is_null());
    let uuid_str = uuid_str.as_ref().unwrap();

    assert_eq!(uuid_str.to_string(), "00000000-0000-0000-0000-000000000000");
}

// Test making a uuid from data, reading it back and comparing it against another
#[test]
fn uuid_test_from_data_and_compare() {
    let uuid = ffi::uuid_ctor_str("1111111111111111");
    assert!(!uuid.is_null());
    let uuid = uuid.as_ref().unwrap();

    let uuid_str = ffi::uuid_std_string(uuid);
    assert!(!uuid_str.is_null());
    let uuid_str = uuid_str.as_ref().unwrap();

    assert_eq!(uuid_str.to_string(), "31313131-3131-3131-3131-313131313131");

    let uuid_2 = ffi::uuid_ctor();
    assert!(!uuid_2.is_null());
    let uuid_2 = uuid_2.as_ref().unwrap();

    // uuid should be equal to itself
    assert!(ffi::uuid_operator_eq(uuid, uuid));
    assert!(ffi::uuid_operator_eq(uuid_2, uuid_2));
    // but not equal to each other
    assert!(!ffi::uuid_operator_eq(uuid, uuid_2));
}
