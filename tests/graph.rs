#[tokio::test]
async fn test_graphql_query() {
    println!("Testing");
    let result = trash_bot::trash_dates::get_tomorrows_trash()
        .await
        .expect("No data found.");
    println!("This is the test result: {:?}", result);
    assert!(true);
}
