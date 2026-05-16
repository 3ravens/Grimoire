#pragma once
#include "rust/cxx.h"

#include <memory>
using std::unique_ptr;

#include <string>
using std::string;

#include <vector>
using std::vector;

#include <zim/archive.h>
#include <zim/blob.h>
#include <zim/entry.h>
#include <zim/error.h>
#include <zim/item.h>
#include <zim/suggestion.h>
#include <zim/uuid.h>
#include <zim/version.h>
using namespace zim;

class EntryRangeEfficient {
    public:
        EntryRangeEfficient(Archive::EntryRange<EntryOrder::efficientOrder>&& inner) : inner(inner) {};
        Archive::EntryRange<EntryOrder::efficientOrder> inner;
};

class IterEfficient {
    public:
        IterEfficient(Archive::iterator<EntryOrder::efficientOrder>&& inner) : inner(inner) {};
        Archive::iterator<EntryOrder::efficientOrder> inner;
};

class DirectAccess {
    public:
        DirectAccess(zim::ItemDataDirectAccessInfo&& inner) : inner(inner) {};
        zim::ItemDataDirectAccessInfo inner;
};

// FILE: archive.h
unique_ptr<Archive> archive_ctor_file(rust::Str path);
// missing ctor_fd, ctor_fd_offset
const string &archive_getFilename(const Archive &archive);
size_type archive_getFilesize(const Archive &archive);
entry_index_type archive_getAllEntryCount(const Archive &archive);
entry_index_type archive_getEntryCount(const Archive &archive);
entry_index_type archive_getArticleCount(const Archive &archive);
unique_ptr<Uuid> archive_getUuid(const Archive &archive);
unique_ptr<string> archive_getMetadata(const Archive &archive, rust::Str name);
unique_ptr<Item> archive_getMetadataItem(const Archive &archive, rust::Str name);
unique_ptr<vector<string>> archive_getMetadataKeys(const Archive &archive);

// Item archive_getIllustrationItem(const Archive &archive, unsigned int size = 48);
// vector<unsigned int> archive_getIllustrationSizes(const Archive &archive); // std::set probably has no bridge to rust
unique_ptr<Entry> archive_getEntryByPath_idx(const Archive &archive, entry_index_type idx);
unique_ptr<Entry> archive_getEntryByPath_str(const Archive &archive, rust::Str path);
unique_ptr<Entry> archive_getEntryByTitle_idx(const Archive &archive, entry_index_type idx);
unique_ptr<Entry> archive_getEntryByTitle_str(const Archive &archive, rust::Str title);
unique_ptr<Entry> archive_getEntryByClusterOrder(const Archive &archive, entry_index_type idx);
unique_ptr<Entry> archive_getMainEntry(const Archive &archive);
unique_ptr<Entry> archive_getRandomEntry(const Archive &archive);

bool archive_hasEntryByPath(const Archive &archive, rust::Str path);
bool archive_hasEntryByTitle(const Archive &archive, rust::Str title);
bool archive_hasMainEntry(const Archive &archive);
// bool archive_hasIllustration(const Archive &archive, unsigned int size = 48);
bool archive_hasFulltextIndex(const Archive &archive);
bool archive_hasTitleIndex(const Archive &archive);
// EntryRange functions
bool archive_hasChecksum(const Archive &archive);
unique_ptr<string> archive_getChecksum(const Archive &archive);
bool archive_check(const Archive &archive);
// bool archive_checkIntegrity(const Archive &archive, IntegrityCheck checkType);
bool archive_isMultiPart(const Archive &archive);
bool archive_hasNewNamespaceScheme(const Archive &archive);

unique_ptr<EntryRangeEfficient> archive_iterEfficient(const Archive& archive);

unique_ptr<IterEfficient> entryrangeefficient_begin(const EntryRangeEfficient& range);
unique_ptr<IterEfficient> entryrangeefficient_end(const EntryRangeEfficient& range);

bool iterefficient_eq(const IterEfficient& iter, const IterEfficient& other);
unique_ptr<Entry> iterefficient_star(const IterEfficient& iter);
void iterefficient_inc(IterEfficient& iter);


// FILE: blob.h
unique_ptr<Blob> blob_ctor();
const char *blob_data(const Blob &blob);
size_type blob_size(const Blob &blob);

// FILE: entry.h
bool entry_isRedirect(const Entry &entry);
unique_ptr<string> entry_getTitle(const Entry &entry);
unique_ptr<string> entry_getPath(const Entry &entry);
unique_ptr<Item> entry_getItem(const Entry &entry, bool follow);
unique_ptr<Item> entry_getRedirect(const Entry &entry);
unique_ptr<Entry> entry_getRedirectEntry(const Entry &entry);
// entry_index_type entry_getRedirectEntryIndex(const Entry &entry);
entry_index_type entry_getIndex(const Entry &entry);

// FILE: item.h
unique_ptr<string> item_getTitle(const Item &item);
unique_ptr<string> item_getPath(const Item &item);
unique_ptr<string> item_getMimetype(const Item &item);
unique_ptr<Blob> item_getData(const Item &item);
unique_ptr<Blob> item_getData_offset(const Item &item, offset_type offset, size_type size);
size_type item_getSize(const Item &item);
unique_ptr<DirectAccess> item_getDirectAccess(const Item &item);
entry_index_type item_getIndex(const Item &item);

// Wrapper DirectAccess
unique_ptr<string> directAccess_getPath(const DirectAccess & dAccess);
offset_type directAccess_getOffset(const DirectAccess & dAccess);

// FILE: suggestion.h
unique_ptr<SuggestionSearcher> suggestionsearcher_ctor(const Archive &archive);
unique_ptr<SuggestionSearch> suggestionsearcher_suggest(SuggestionSearcher &suggestionsearcher, rust::Str query);
void suggestionsearcher_setVerbose(SuggestionSearcher &suggestionsearcher, bool verbose);

unique_ptr<SuggestionResultSet> suggestionsearch_getResults(const SuggestionSearch &suggestionsearch, int start, int maxResults);
int suggestionsearch_getEstimatedMatches(const SuggestionSearch &suggestionsearch);

unique_ptr<SuggestionIterator> suggestionresultset_begin(const SuggestionResultSet &suggestionresultset);
unique_ptr<SuggestionIterator> suggestionresultset_end(const SuggestionResultSet &suggestionresultset);
int suggestionresultset_size(const SuggestionResultSet &suggestionresultset);

// FILE: suggestion_iterator.h
bool suggestioniterator_operator_eq(const SuggestionIterator &suggestioniterator, const SuggestionIterator &o);
bool suggestioniterator_operator_neq(const SuggestionIterator &suggestioniterator, const SuggestionIterator &o);
void suggestioniterator_operator_inc(SuggestionIterator &suggestioniterator);
unique_ptr<SuggestionItem> suggestioniterator_operator_star(SuggestionIterator &suggestioniterator);
unique_ptr<Entry> suggestioniterator_getEntry(const SuggestionIterator &suggestioniterator);

unique_ptr<string> suggestionitem_getTitle(const SuggestionItem &suggestionitem);
unique_ptr<string> suggestionitem_getPath(const SuggestionItem &suggestionitem);
unique_ptr<string> suggestionitem_getSnippet(const SuggestionItem &suggestionitem);
bool suggestionitem_hasSnippet(const SuggestionItem &suggestionitem);

// FILE: uuid.h
unique_ptr<Uuid> uuid_ctor();
unique_ptr<Uuid> uuid_ctor_str(rust::Str uuid); // uuid must be 16 characters long and valid
unique_ptr<Uuid> uuid_generate(rust::Str value);
bool uuid_operator_eq(const Uuid &uuid, const Uuid &o);
unique_ptr<string> uuid_std_string(const Uuid &uuid);
