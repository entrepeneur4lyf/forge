#[cfg(test)]
mod tests {
    use forge_env::Environment;
    use forge_provider::ModelId;
    use forge_server::{ChatRequest, ChatResponse, Server};
    use regex::Regex;
    use tokio_stream::StreamExt;

    macro_rules! assert {
        ($file_path:expr) => {
            for file_path in $file_path.iter() {
                let snap_name = snap_name(file_path);
                dbg!(&snap_name);
                let file_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), file_path);
                let a = read(format!("{}", file_path));
                let re = Regex::new(r"\.(\w+)$").unwrap();
                let b = read(re.replace(&file_path, "_updated.$1"));

                let patch = diffy::create_patch(&a, &b);

                let mut patch_output = vec![];
                diffy::PatchFormatter::new()
                    .write_patch_into(&patch, &mut patch_output)
                    .expect("Failed to format patch");

                insta::assert_snapshot!(snap_name, String::from_utf8(patch_output).unwrap());
            }
            delete_updated_files(&$file_path);
        };
    }

    fn snap_name<T: AsRef<str>>(path: T) -> String {
        let path = path.as_ref();

        path.replace("/", "_").replace(".", "_")
    }

    async fn server() -> Server {
        let env = Environment::from_env().await.unwrap();
        let api_key = std::env::var("FORGE_KEY").expect("FORGE_KEY must be set");
        Server::new(env, api_key)
    }

    fn read<T: AsRef<str>>(path: T) -> String {
        std::fs::read_to_string(path.as_ref())
            .unwrap_or_else(|_| panic!("Failed to read file {}", path.as_ref()))
    }

    fn delete_updated_files<T: AsRef<str>>(file_path: &[T]) {
        for file in file_path {
            let file = format!(
                "{}/tests/{}",
                env!("CARGO_MANIFEST_DIR"),
                file.as_ref().replace(".md", "_updated.md")
            );
            let _ = std::fs::remove_file(file);
        }
    }

    async fn chat<T: AsRef<str>>(base: T, file_paths: &[T]) -> Vec<ChatResponse> {
        delete_updated_files(file_paths);
        let server = server().await;
        let req = ChatRequest::default()
            .message(format!("{} in the file(s) located at {} .Do not change the input file directly, create another file with changes within the same dir as the original file and name it <file_name>_updated.ext", base.as_ref(), file_paths.iter().map(|f| f.as_ref()).collect::<Vec<&str>>().join(", ")))
            .model(ModelId::new("anthropic/claude-3.5-haiku"));

        server
            .chat(req)
            .await
            .unwrap()
            .collect::<Vec<ChatResponse>>()
            .await
    }

    #[tokio::test]
    async fn test_rs_patches() {
        let file_paths = vec!["tests/fixtures/non_idiomatic.rs"];
        let _resp = chat("Refactor the following Rust code to use constructs like iterators and functional programming patterns. Avoid explicit loops and make the code concise while preserving its functionality. Do not rename variables.", &file_paths).await;
        assert!(file_paths);
    }
}
