//! `fixtures/slugs.json` holds pack id slugs both Blockwork and the
//! community site's copy of `slug` (`src/packs.ts` there) must produce.

#[test]
fn slugs_match_the_shared_fixture() {
    let cases: Vec<(String, String)> =
        serde_json::from_str(include_str!("fixtures/slugs.json")).unwrap();
    for (text, expected) in cases {
        assert_eq!(
            blockwork_model::block_packs::slug(&text),
            expected,
            "{text:?}"
        );
    }
}
