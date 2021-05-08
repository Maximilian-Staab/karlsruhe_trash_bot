#[tokio::test]
async fn test_graphql_query() {
    println!("Testing");
    let result = telegram_bot::trash_dates::trash::perform_my_query()
        .await
        .expect("No data found.");
    println!("{:?}", result);
    assert!(true);
}
