#[cfg(test)]
mod tests {
    use forge_server_tests::TestServer;

    #[tokio::test]
    async fn test_rs_patches() {
        let project = "fixtures/rust_tests1";
        let server = TestServer::new(project).await.unwrap();

        let prompt = "Refactor the following Rust code to use constructs like iterators and functional programming patterns. Avoid unwanted new lines, explicit loops and make the code concise while preserving its functionality. Do not rename variables.";
        let _ = server.chat(prompt).await;

        let patches = server.create_patches().await.unwrap();
        insta::assert_snapshot!(patches);
    }
}
