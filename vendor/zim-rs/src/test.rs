use crate::archive::*;
use crate::search::*;
use crate::suggestion::*;

pub static WIKT: &str = "/home/aka/Downloads/wiktionary_en_all_maxi_2022-09.zim";

#[test]
fn test() {
    let a = Archive::new(WIKT).expect("File should exist!");
    println!("Checksum: {}", a.get_checksum().unwrap());
}

#[test]
fn search() {
    let a = Archive::new(WIKT).expect("File should exist!");

    let mut sr = Searcher::new(&a).expect("Searcher failed to create");
    let q = Query::new("name").expect("Query failed to create");
    let s = sr.search(&q).expect("Search failed to create");
    println!("Estimated matches: {}", s.get_estimated_matches());

    let res = s.get_results(0, 10).expect("Search matches failed");
    let mut count = 0;
    for result in res {
        count += 1;
        match result {
            Ok(entry) => {
                println!("{}, [{}]", entry.get_title(), entry.get_index());
            }
            Err(_) => println!("Entry error!"),
        }
    }
    assert_eq!(count, 10);
}

#[test]
fn suggest() {
    let a = Archive::new(WIKT).expect("File should exist!");

    let mut sr = SuggestionSearcher::new(&a).expect("SuggestionSearcher failed to create");
    let s = sr
        .suggest("name")
        .expect("SuggestionSearch failed to create");
    println!("Estimated matches: {}", s.get_estimated_matches());

    let res = s
        .get_results(0, 10)
        .expect("SuggestionSearch matches failed");
    let mut count = 0;
    for result in res {
        count += 1;
        match result {
            Ok(item) => {
                println!("{}, [{}]", item.get_title(), item.get_path());
            }
            Err(_) => println!("SuggestionItem error!"),
        }
    }
    assert_eq!(count, 10);
}
