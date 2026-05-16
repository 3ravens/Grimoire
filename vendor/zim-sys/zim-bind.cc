#include "zim-sys/zim-bind.h"
#include "zim-sys/src/binding.rs.h"
using namespace zim;

#include "rust/cxx.h"
using rust::Str;

#include <memory>
using std::make_unique;
using std::unique_ptr;

#include <string>
using std::string;

// FILE: archive.h

unique_ptr<Archive> archive_ctor_file(rust::Str path)
{
    try
    {
        return make_unique<Archive>(string(path));
    }
    catch (...)
    {
        return NULL;
    }
}

const string &archive_getFilename(const Archive &archive)
{
    return archive.getFilename();
}

size_type archive_getFilesize(const Archive &archive)
{
    return archive.getFilesize();
}

entry_index_type archive_getAllEntryCount(const Archive &archive)
{
    return archive.getAllEntryCount();
}

entry_index_type archive_getEntryCount(const Archive &archive)
{
    return archive.getEntryCount();
}

entry_index_type archive_getArticleCount(const Archive &archive)
{
    return archive.getArticleCount();
}

unique_ptr<Uuid> archive_getUuid(const Archive &archive)
{
    try
    {
        return make_unique<Uuid>(archive.getUuid());
    }
    catch (...)
    {
        return NULL;
    }
}

unique_ptr<string> archive_getMetadata(const Archive &archive, rust::Str name)
{
    try
    {
        return make_unique<string>(archive.getMetadata(string(name)));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Item> archive_getMetadataItem(const Archive &archive, rust::Str name)
{
    try
    {
        return make_unique<Item>(archive.getMetadataItem(string(name)));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<vector<string>> archive_getMetadataKeys(const Archive &archive)
{
    try
    {
        return make_unique<vector<string>>(archive.getMetadataKeys());
    }
    catch (...)
    {
        return NULL;
    }
}

// Item archive_getIllustrationItem(const Archive &archive, unsigned int size = 48);
// vector<unsigned int> archive_getIllustrationSizes(const Archive &archive); // std::set probably has no bridge to rust

unique_ptr<Entry> archive_getEntryByPath_idx(const Archive &archive, entry_index_type idx)
{
    try
    {
        return make_unique<Entry>(archive.getEntryByPath(idx));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getEntryByPath_str(const Archive &archive, rust::Str path)
{
    try
    {
        return make_unique<Entry>(archive.getEntryByPath(string(path)));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getEntryByTitle_idx(const Archive &archive, entry_index_type idx)
{
    try
    {
        return make_unique<Entry>(archive.getEntryByTitle(idx));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getEntryByTitle_str(const Archive &archive, rust::Str title)
{
    try
    {
        return make_unique<Entry>(archive.getEntryByTitle(string(title)));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getEntryByClusterOrder(const Archive &archive, entry_index_type idx)
{
    try
    {
        return make_unique<Entry>(archive.getEntryByClusterOrder(idx));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getMainEntry(const Archive &archive)
{
    try
    {
        return make_unique<Entry>(archive.getMainEntry());
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> archive_getRandomEntry(const Archive &archive)
{
    try
    {
        return make_unique<Entry>(archive.getRandomEntry());
    }
    catch (...)
    {
        return NULL;
    }
}

bool archive_hasEntryByPath(const Archive &archive, rust::Str path)
{
    return archive.hasEntryByPath(string(path));
}
bool archive_hasEntryByTitle(const Archive &archive, rust::Str title)
{
    return archive.hasEntryByTitle(string(title));
}
bool archive_hasMainEntry(const Archive &archive)
{
    return archive.hasMainEntry();
}
// bool archive_hasIllustration(const Archive &archive, unsigned int size = 48);
bool archive_hasFulltextIndex(const Archive &archive)
{
    return archive.hasFulltextIndex();
}
bool archive_hasTitleIndex(const Archive &archive)
{
    return archive.hasTitleIndex();
}
// EntryRange functions
bool archive_hasChecksum(const Archive &archive)
{
    return archive.hasChecksum();
}
unique_ptr<string> archive_getChecksum(const Archive &archive)
{
    return make_unique<string>(archive.getChecksum());
}
bool archive_check(const Archive &archive)
{
    return archive.check();
}
// bool archive_checkIntegrity(const Archive &archive, IntegrityCheck checkType);
bool archive_isMultiPart(const Archive &archive)
{
    return archive.isMultiPart();
}
bool archive_hasNewNamespaceScheme(const Archive &archive)
{
    return archive.hasNewNamespaceScheme();
}

unique_ptr<EntryRangeEfficient> archive_iterEfficient(const Archive& archive) {
    return make_unique<EntryRangeEfficient>(archive.iterEfficient());
}

unique_ptr<IterEfficient> entryrangeefficient_begin(const EntryRangeEfficient& range) {
    return make_unique<IterEfficient>(range.inner.begin());
}
unique_ptr<IterEfficient> entryrangeefficient_end(const EntryRangeEfficient& range){
    return make_unique<IterEfficient>(range.inner.end());
}

bool iterefficient_eq(const IterEfficient& iter, const IterEfficient& other) {
    return iter.inner.operator==(other.inner);
}

unique_ptr<Entry> iterefficient_star(const IterEfficient& iter) {
    try
    {
        return make_unique<Entry>(iter.inner.operator*());
    }
    catch (...)
    {
        return NULL;
    }
}
void iterefficient_inc(IterEfficient& iter) {
    iter.inner.operator++();
}


// FILE: blob.h

unique_ptr<Blob> blob_ctor()
{
    return make_unique<Blob>();
}

const char *blob_data(const Blob &blob)
{
    return blob.data();
}

size_type blob_size(const Blob &blob)
{
    return blob.size();
}

// FILE: entry.h

bool entry_isRedirect(const Entry &entry)
{
    return entry.isRedirect();
}
unique_ptr<string> entry_getTitle(const Entry &entry)
{
    return make_unique<string>(entry.getTitle());
}
unique_ptr<string> entry_getPath(const Entry &entry)
{
    return make_unique<string>(entry.getPath());
}
unique_ptr<Item> entry_getItem(const Entry &entry, bool follow)
{
    try
    {
        return make_unique<Item>(entry.getItem(follow));
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Item> entry_getRedirect(const Entry &entry)
{
    try
    {
        return make_unique<Item>(entry.getRedirect());
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Entry> entry_getRedirectEntry(const Entry &entry)
{
    try
    {
        return make_unique<Entry>(entry.getRedirectEntry());
    }
    catch (...)
    {
        return NULL;
    }
}
/*
entry_index_type entry_getRedirectEntryIndex(const Entry &entry)
{
    try
    {
        return entry.getRedirectEntryIndex();
    }
    catch (...)
    {
        // TODO: fix this with appropriate error handling
        return 0;
    }
}
*/
entry_index_type entry_getIndex(const Entry &entry)
{
    return entry.getIndex();
}

// FILE: item.h

unique_ptr<string> item_getTitle(const Item &item)
{
    return make_unique<string>(item.getTitle());
}
unique_ptr<string> item_getPath(const Item &item)
{
    return make_unique<string>(item.getPath());
}
unique_ptr<string> item_getMimetype(const Item &item)
{
    try
    {
        // can throw in FileImpl::getMimeType
        return make_unique<string>(item.getMimetype());
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Blob> item_getData(const Item &item)
{
    try
    {
        return make_unique<Blob>(item.getData());
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Blob> item_getData_offset(const Item &item, offset_type offset, size_type size)
{
    try
    {
        return make_unique<Blob>(item.getData(offset, size));
    }
    catch (...)
    {
        return NULL;
    }
}
size_type item_getSize(const Item &item)
{
    return item.getSize();
}
unique_ptr<DirectAccess> item_getDirectAccess(const Item &item)
{
    try {
        return make_unique<DirectAccess>(item.getDirectAccessInformation());
    } catch (...)
    {
        return NULL;
    }
}
entry_index_type item_getIndex(const Item &item)
{
    return item.getIndex();
}


unique_ptr<string> directAccess_getPath(const DirectAccess & dAccess) {
    return make_unique<string>(dAccess.inner.filename);
}
offset_type directAccess_getOffset(const DirectAccess & dAccess) {
    return dAccess.inner.offset;
}


// FILE: suggestion.h

unique_ptr<SuggestionSearcher> suggestionsearcher_ctor(const Archive &archive)
{
    return make_unique<SuggestionSearcher>(archive);
}
unique_ptr<SuggestionSearch> suggestionsearcher_suggest(SuggestionSearcher &suggestionsearcher, rust::Str query)
{
    try
    {
        return make_unique<SuggestionSearch>(suggestionsearcher.suggest(string(query)));
    }
    catch (...)
    {
        return NULL;
    }
}
void suggestionsearcher_setVerbose(SuggestionSearcher &suggestionsearcher, bool verbose)
{
    suggestionsearcher.setVerbose(verbose);
}

unique_ptr<SuggestionResultSet> suggestionsearch_getResults(const SuggestionSearch &suggestionsearch, int start, int maxResults)
{
    return make_unique<SuggestionResultSet>(suggestionsearch.getResults(start, maxResults));
}
int suggestionsearch_getEstimatedMatches(const SuggestionSearch &suggestionsearch)
{
    return suggestionsearch.getEstimatedMatches();
}

unique_ptr<SuggestionIterator> suggestionresultset_begin(const SuggestionResultSet &suggestionresultset)
{
    return make_unique<SuggestionIterator>(suggestionresultset.begin());
}
unique_ptr<SuggestionIterator> suggestionresultset_end(const SuggestionResultSet &suggestionresultset)
{
    return make_unique<SuggestionIterator>(suggestionresultset.end());
}
int suggestionresultset_size(const SuggestionResultSet &suggestionresultset)
{
    return suggestionresultset.size();
}

// FILE: suggestion_iterator.h

bool suggestioniterator_operator_eq(const SuggestionIterator &suggestioniterator, const SuggestionIterator &o)
{
    return suggestioniterator.operator==(o);
}
bool suggestioniterator_operator_neq(const SuggestionIterator &suggestioniterator, const SuggestionIterator &o)
{
    return suggestioniterator.operator!=(o);
}
void suggestioniterator_operator_inc(SuggestionIterator &suggestioniterator)
{
    suggestioniterator.operator++();
}
unique_ptr<SuggestionItem> suggestioniterator_operator_star(SuggestionIterator &suggestioniterator)
{
    try
    {
        return make_unique<SuggestionItem>(suggestioniterator.operator*());
    }
    catch (...)
    {
        return NULL;
    }
}

unique_ptr<Entry> suggestioniterator_getEntry(const SuggestionIterator &suggestioniterator)
{
    try
    {
        return make_unique<Entry>(suggestioniterator.getEntry());
    }
    catch (...)
    {
        return NULL;
    }
}

unique_ptr<string> suggestionitem_getTitle(const SuggestionItem &suggestionitem)
{
    return make_unique<string>(suggestionitem.getTitle());
}
unique_ptr<string> suggestionitem_getPath(const SuggestionItem &suggestionitem)
{
    return make_unique<string>(suggestionitem.getPath());
}
unique_ptr<string> suggestionitem_getSnippet(const SuggestionItem &suggestionitem)
{
    return make_unique<string>(suggestionitem.getSnippet());
}
bool suggestionitem_hasSnippet(const SuggestionItem &suggestionitem)
{
    return suggestionitem.hasSnippet();
}

// FILE: uuid.h

unique_ptr<Uuid> uuid_ctor()
{
    try
    {
        return make_unique<Uuid>();
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Uuid> uuid_ctor_str(rust::Str uuid)
{
    if (uuid.size() != 16)
    {
        return NULL;
    }
    try
    {
        string s = string(uuid);
        const char *cs = s.c_str();
        return make_unique<Uuid>(cs);
    }
    catch (...)
    {
        return NULL;
    }
}
unique_ptr<Uuid> uuid_generate(rust::Str value)
{
    try
    {
        return make_unique<Uuid>(Uuid::generate(string(value)));
    }
    catch (...)
    {
        return NULL;
    }
}
bool uuid_operator_eq(const Uuid &uuid, const Uuid &o)
{
    return uuid.operator==(o);
}
unique_ptr<string> uuid_std_string(const Uuid &uuid)
{
    try
    {
        string s = static_cast<string>(uuid);
        return make_unique<string>(s);
    }
    catch (...)
    {
        return NULL;
    }
}
